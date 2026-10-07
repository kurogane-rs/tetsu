# cef-rs

Use CEF in Rust.

## Supported Targets

| Target | Linux | macOS | Windows |
| ------ | ----- | ----- | ------- |
| x86_64 | ✅    | ✅    | ✅      |
| ARM64  | ✅    | ✅    | ✅      |

## Usage

### Nix (on Linux)

If you are using Nix on Linux with the `nixpkgs` channel configured, you can take advantage of the `cef-binary` package to share the CEF binaries across your system. Just set the `NIX_CEF_BINARY` environment variable before running any of these `cargo` commands.

```sh
export NIX_CEF_BINARY=1
```

You can still run `export-cef-dir` and set the `CEF_PATH` environment variable if you prefer, e.g., for build output caching, but it is not necessary. If you combine `NIX_CEF_BINARY` with `export-cef-dir`, even `export-cef-dir` will use Nix instead of directly downloading the CEF binaries.

### Install Shared CEF Binaries

This step is optional. Linux and Windows builds need no CEF at all (see [Loading libcef](#loading-libcef)). Builds for macOS and the runtime copy below otherwise install the CEF they need once for every project of the user, under `tetsu/cef/<version>/cef_<os>_<arch>` in the local data directory (`~/.local/share`, `%LOCALAPPDATA%`, `~/Library/Application Support`); `tetsu_sys::cef_install_dir()` names that directory at run time, so an application can load its CEF from there. Only a user without a data directory, or `NIX_CEF_BINARY`, gets a copy under the build script's `OUT_DIR`. Export a distribution of your own as below and point `CEF_PATH` at it to build against that instead; repeat this step each time you upgrade to a new version of the `tetsu` crate.

A set `CEF_PATH` is used as it is: when it does not exist or holds no distribution of the CEF version the crate needs, the build fails and names the path. It never downloads into it.

#### Linux or macOS:

```sh
cargo run -p export-cef-dir -- --force $HOME/.local/share/cef
```

#### Windows (using PowerShell)

```pwsh
cargo run -p export-cef-dir -- --force $env:USERPROFILE/.local/share/cef
```

### Set Environment Variables

#### Linux

```sh
export CEF_PATH="$HOME/.local/share/cef"
```

#### macOS

```sh
export CEF_PATH="$HOME/.local/share/cef"
export DYLD_FALLBACK_LIBRARY_PATH="$DYLD_FALLBACK_LIBRARY_PATH:$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
```

#### Windows (using PowerShell)

```pwsh
$env:CEF_PATH="$env:USERPROFILE/.local/share/cef"
```

On Linux and Windows libcef needs no library search path; the application loads it from a path it names.

### Run the `cefsimple` Example

This command should work with each platform:
```sh
cargo run --bin bundle-cef-app -- cefsimple -o target/bundle
```

You can configure the name of the macOS helper in the `Cargo.toml` file, as well as a resource directory that will be copied into the bundle in a platform-appropriate location:
```toml
[package.metadata.cef.bundle]
helper_name = "cefsimple_helper"
resources_path = "resources"
```

#### Linux

There's an extra `--release` flag to build a much smaller bundle on Linux:
```sh
cargo run --bin bundle-cef-app -- cefsimple -o target/bundle --release
./target/bundle/cefsimple.exe
```

#### macOS

The macOS utility creates an application bundle directory at the target location, you can run it with the `open` command:
```sh
cargo run --bin bundle-cef-app -- cefsimple -o target/bundle
open target/bundle/cefsimple.app
```

On macOS, the `bundle-cef-app` utility also supports several additional bundle options, most of which default to the name of the application (e.g. `cefsimple`):
```
Usage: bundle-cef-app [OPTIONS] <NAME>

Arguments:
  <NAME>

Options:
  -o, --output <OUTPUT>
  -i, --identifier <IDENTIFIER>
  -d, --display-name <DISPLAY_NAME>
  -r, --region <REGION>              [default: English]
  -v, --version <VERSION>            [default: 1.0.0]
  -h, --help                         Print help
```

#### Windows (using PowerShell)

The Windows utility supports the `--release` flag, but it makes much less difference in the binary size than on Linux. It also does not copy the resources directory to the bundle, because the preferred mechanism on Windows is to link binary resources directly into the executable.

However, the utility will emit an executable manifest file, and if the `sandbox` feature is enabled, it will build the DLL (cdylib) target instead of the executable (bin) target, and copy that with a renamed `bootstrap.exe` file to the bundle directory, so you can run it from there directly:
```pwsh
cargo run --bin bundle-cef-app -- cefsimple -o ./target/bundle
./target/bundle/cefsimple.exe
```

### Loading libcef

On Linux and Windows the bindings link no libcef. Each function libcef exports is resolved when the application calls `tetsu::sys::load_libcef` with the path to `libcef.dll` or `libcef.so` (`tetsu::sys::LIBCEF_FILE` names the file), which comes before any other call into CEF; one called before panics, naming `load_libcef`. The application decides which CEF it runs, and building needs no CEF distribution.

### Runtime files next to the binary

On Linux and Windows a binary run from `target/` finds CEF's runtime (the libraries, `.pak` files, `icudtl.dat`, the V8 snapshot and `locales/`) when it sits next to it. Set `TETSU_STAGE_RUNTIME=1` and the `tetsu-sys` build script copies it there. This repository sets it for its own builds (`.cargo/config.toml`), so the examples load libcef from beside their executable and run with `cargo run`; an application that names CEF's paths itself (`resources_dir_path`, `locales_dir_path`) leaves it unset and skips the copy of up to 1.5 GB per profile.

### Cross-compiling to Windows

The `tetsu-sys` crate can be cross-compiled to `x86_64-pc-windows-msvc` from Linux with [cargo-xwin](https://github.com/rust-cross/cargo-xwin), which downloads the Windows SDK and links with `lld-link`. Install `clang`, `lld` and `llvm` from your package manager, then:

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin
cargo xwin build --target x86_64-pc-windows-msvc
```

Nothing is compiled from C++ and nothing of CEF is linked, so a cross-compiled build needs no CEF distribution. The runtime copy (`TETSU_STAGE_RUNTIME=1`) needs the Windows one: a set `CEF_PATH` must hold it (`cargo run -p export-cef-dir -- --target x86_64-pc-windows-msvc <dir>` writes one); without `CEF_PATH` the build installs it into the shared directory.

## Contributing

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.
