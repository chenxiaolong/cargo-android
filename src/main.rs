// SPDX-FileCopyrightText: 2024 Andrew Gunnerson
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    path::PathBuf,
    process::{self, Command, ExitStatus},
};

#[cfg(target_os = "linux")]
const NDK_OS: &str = "linux";
#[cfg(target_os = "macos")]
const NDK_OS: &str = "darwin";
#[cfg(target_os = "windows")]
const NDK_OS: &str = "windows";

#[cfg(not(target_os = "windows"))]
const CLANG_SUFFIX: &str = "";
#[cfg(target_os = "windows")]
const CLANG_SUFFIX: &str = ".cmd";

#[cfg(not(target_os = "windows"))]
const EXE_SUFFIX: &str = "";
#[cfg(target_os = "windows")]
const EXE_SUFFIX: &str = ".exe";

fn get_android_env(target: &str) -> Result<HashMap<String, OsString>, String> {
    let ndk_dir = env::var_os("ANDROID_NDK_ROOT")
        .map(PathBuf::from)
        .ok_or("ANDROID_NDK_ROOT must be set when building for Android")?;

    let upper_target = target.to_ascii_uppercase().replace('-', "_");
    let ndk_target = match target {
        "armv7-linux-androideabi" | "thumbv7neon-linux-androideabi" => "armv7a-linux-androideabi",
        t => t,
    };

    let mut toolchain_dir = ndk_dir.clone();
    toolchain_dir.push("toolchains");
    toolchain_dir.push("llvm");
    toolchain_dir.push("prebuilt");
    toolchain_dir.push(format!("{NDK_OS}-x86_64"));

    if !toolchain_dir.exists() {
        return Err(format!("Toolchain directory not found: {toolchain_dir:?}"));
    }

    let sysroot_dir = toolchain_dir.join("sysroot");

    let api = if let Some(v) = env::var_os("ANDROID_API") {
        v.to_str()
            .and_then(|s| s.parse::<u8>().ok())
            .ok_or_else(|| format!("Invalid ANDROID_API: {v:?}"))?
    } else {
        let mut lib_dir = sysroot_dir.clone();
        lib_dir.push("usr");
        lib_dir.push("lib");
        lib_dir.push(target);

        lib_dir
            .read_dir()
            .map_err(|e| format!("{lib_dir:?}: {e}"))?
            .filter_map(|r| {
                r.ok()
                    .and_then(|e| e.file_name().to_str().and_then(|n| n.parse::<u8>().ok()))
            })
            .max()
            .ok_or_else(|| format!("Failed to get API list from: {lib_dir:?}"))?
    };

    let mut ar = toolchain_dir.clone();
    ar.push("bin");
    ar.push(format!("llvm-ar{EXE_SUFFIX}"));

    let mut clang = toolchain_dir.clone();
    clang.push("bin");
    clang.push(format!("{ndk_target}{api}-clang{CLANG_SUFFIX}"));

    let mut vars = HashMap::new();
    vars.insert(format!("AR_{target}"), ar.into_os_string());
    vars.insert(format!("CC_{target}"), clang.as_os_str().to_owned());
    vars.insert(format!("BINDGEN_EXTRA_CLANG_ARGS_{target}"), {
        let mut v = OsString::from("--sysroot=");
        v.push(sysroot_dir);
        v
    });
    vars.insert(
        format!("CARGO_TARGET_{upper_target}_LINKER"),
        clang.into_os_string(),
    );

    // Work around https://github.com/rust-lang/rust/issues/109717.
    if target == "x86_64-linux-android" {
        let mut clang_dir = toolchain_dir.clone();
        clang_dir.push("lib");
        clang_dir.push("clang");

        let clang_version = clang_dir
            .read_dir()
            .and_then(|mut d| d.next().transpose())
            .map_err(|e| format!("Failed to list directory: {clang_dir:?}: {e}"))?
            .ok_or_else(|| format!("Missing clang version: {clang_dir:?}"))?
            .file_name();

        let mut clang_rt_dir = clang_dir.clone();
        clang_rt_dir.push(clang_version);
        clang_rt_dir.push("lib");
        clang_rt_dir.push("linux");

        let clang_rt_dir = clang_rt_dir
            .into_os_string()
            .into_string()
            .map_err(|p| format!("Invalid UTF-8: {p:?}"))?;

        let mut rustflags = vec![];

        // Global flags completely override CARGO_TARGET_<target>_RUSTFLAGS, so
        // we have to append to the global flags instead of using target flags.
        // Cargo only supports UTF-8 for these variables, so we don't worry
        // about OsString here.
        if let Ok(flags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
            rustflags.extend(flags.split('\x1f').map(str::to_string));
        } else if let Ok(flags) = env::var("RUSTFLAGS") {
            rustflags.extend(
                flags
                    .split(' ')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            );
        }

        rustflags.push("-L".into());
        rustflags.push(clang_rt_dir);
        rustflags.push("-l".into());
        rustflags.push("static=clang_rt.builtins-x86_64-android".into());

        vars.insert(
            format!("CARGO_ENCODED_RUSTFLAGS"),
            rustflags.join("\x1f").into(),
        );
    }

    Ok(vars)
}

fn main_wrapper() -> Result<ExitStatus, String> {
    let mut target = None;
    let mut next_is_target = false;

    for arg in env::args_os().skip(2) {
        let Some(arg_str) = arg.to_str() else {
            let arg_str_lossy = arg.to_string_lossy();
            if next_is_target || arg_str_lossy.starts_with("--target=") {
                return Err(format!("Invalid UTF-8: {arg:?}"));
            } else {
                continue;
            }
        };

        if next_is_target {
            target = Some(arg_str.to_owned());
            break;
        } else if arg_str == "--target" {
            next_is_target = true;
        } else if let Some(value) = arg_str.strip_prefix("--target=") {
            target = Some(value.to_owned());
            break;
        }
    }

    let cargo = env::var_os("CARGO").ok_or("CARGO must be set")?;

    let mut command = Command::new(cargo);
    command.args(env::args_os().skip(2));

    if let Some(t) = &target {
        if t.contains("android") {
            command.envs(get_android_env(t)?);
        }
    }

    let mut child = command.spawn().map_err(|e| format!("{command:?}: {e}"))?;
    let status = child.wait().map_err(|e| format!("{command:?}: {e}"))?;

    Ok(status)
}

fn get_exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::process::ExitStatusExt;

        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }

    255
}

fn main() {
    let code = match main_wrapper() {
        Ok(status) => get_exit_code(status),
        Err(e) => {
            eprintln!("{e}");
            255
        }
    };

    process::exit(code);
}
