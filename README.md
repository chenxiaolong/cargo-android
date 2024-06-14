# cargo-android

cargo-android is a bare-bones, no dependency wrapper around `cargo` that sets up the necessary environment variables to cross-compile for Android.

## Usage

1. Install cargo-android.

    ```bash
    cargo install --git https://github.com/chenxiaolong/cargo-android
    ```

2. Install the Android Rust target. This requires Rust to be installed via rustup because most other distributions do not package the Android targets.

    ```bash
    rustup target add aarch64-linux-android
    ```

3. Set the `ANDROID_NDK_ROOT` environment variable to the path of the Android NDK toolchain.

4. Set the `ANDROID_API` environment variable to the desired API level (eg. Android 14 is API 34). If left unset, the latest API level supported by the NDK toolchain will be used. Executables compiled for old Android versions can run on newer Android versions, but the reverse is not always true.

5. Run `cargo` commands by prefixing the subcommand with `android`. For example:

    ```bash
    cargo build --target aarch64-linux-android --release
    ```

    would become:

    ```bash
    cargo android build --target aarch64-linux-android --release
    ```

    If `--target` is not specified or if the target is not an Android target, then the wrapper just runs cargo as-is, without setting any environment variables.

## Contributing

Bug fixes are welcome! However, I'm unlikely to accept changes for supporting additional features or configuration that I don't need for my personal projects.

## License

cargo-android is licensed under GPLv3. Please see [`LICENSE`](./LICENSE) for the full license text.
