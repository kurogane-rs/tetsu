# tetsu

Use CEF in Rust. tetsu is Kurogane's fork of [cef-rs](https://github.com/tauri-apps/cef-rs): crates `tetsu`, `tetsu-sys` and `tetsu-download`.

## Supported Targets

| Target | Linux | macOS | Windows |
| ------ | ----- | ----- | ------- |
| x86_64 | ✅    | ✅    | ✅      |
| ARM64  | ✅    | ✅    | ✅      |

## Usage

### Install CEF

Applications load libcef when they start (see [Loading libcef](#loading-libcef)), so building needs no CEF. At run time `tetsu_sys::find_cef_dir` looks in the macOS application bundle the executable belongs to, beside the executable, in the directory `CEF_PATH` names, then in the shared installation `tetsu/cef/<version>/cef_<os>_<arch>` under the local data directory (`~/.local/share`, `%LOCALAPPDATA%`, `~/Library/Application Support`). Install the CEF version the crate was generated for there, once for every project:

```sh
cargo run -p export-cef-dir
```

Repeat this step each time you upgrade to a `tetsu` crate of a new CEF version.

### Use a CEF of your own

Export a distribution elsewhere and point `CEF_PATH` at it when starting the application. A set `CEF_PATH` that names no directory is an error; the lookup never skips it for another CEF.

#### Linux or macOS

```sh
cargo run -p export-cef-dir -- --force $HOME/.local/share/cef
export CEF_PATH="$HOME/.local/share/cef"
```

#### Windows (using PowerShell)

```pwsh
cargo run -p export-cef-dir -- --force $env:USERPROFILE/.local/share/cef
$env:CEF_PATH="$env:USERPROFILE/.local/share/cef"
```

#### Nix (on Linux)

With the `nixpkgs` channel configured, `export-cef-dir --nix` takes the distribution from the `cef-binary` package:

```sh
cargo run -p export-cef-dir -- --nix --force $HOME/.local/share/cef
export CEF_PATH="$HOME/.local/share/cef"
```

libcef needs no library search path; the application loads it from the directory it finds.

### Run the `cefsimple` Example

`bundle-cef-app` bundles tetsu's examples so they run. It is a development tool of this repository; applications ship with their own bundler, such as Kurogane's. This command works on each platform:
```sh
cargo run -p bundle-cef-app -- cefsimple -o target/bundle
```

`-F` (`--features`) enables features of the example's package, such as `-F linux-x11` for cefsimple's X11 code on Linux.

You can configure the name of the macOS helper in the `Cargo.toml` file, as well as a resource directory that will be copied into the bundle in a platform-appropriate location:
```toml
[package.metadata.cef.bundle]
helper_name = "cefsimple_helper"
resources_path = "resources"
```

#### Linux

There's an extra `--release` flag to build a much smaller bundle on Linux:
```sh
cargo run -p bundle-cef-app -- cefsimple -o target/bundle --release
./target/bundle/cefsimple
```

#### macOS

The macOS utility creates an application bundle directory at the target location, you can run it with the `open` command:
```sh
cargo run -p bundle-cef-app -- cefsimple -o target/bundle
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
  -F, --features <FEATURES>
  -h, --help                         Print help
```

The identifier defaults to `rs.kurogane.tetsu.<NAME>`.

#### Windows (using PowerShell)

The Windows utility supports the `--release` flag, but it makes much less difference in the binary size than on Linux. It also does not copy the resources directory to the bundle, because the preferred mechanism on Windows is to link binary resources directly into the executable.

The utility writes an executable manifest. A package that builds a cdylib, as cefsimple does, runs in CEF's sandbox: the utility builds the library instead of the executable and copies it beside CEF's `bootstrap.exe`, renamed after the application, so you can run it from there directly:
```pwsh
cargo run -p bundle-cef-app -- cefsimple -o ./target/bundle
./target/bundle/cefsimple.exe
```

`cargo run -p cefsimple` starts the executable itself, without the sandbox.

### Loading libcef

The bindings link no libcef. Before any other call into CEF the application finds its CEF and loads it:

```rust
let cef = tetsu::sys::find_cef_dir()?;
unsafe { tetsu::sys::load_libcef(&cef.libcef()) }?;
```

Loading fixes the process's CEF API version to the bindings' and refuses a libcef that is not the CEF build the bindings were generated from. A CEF function called before it panics, naming `load_libcef`. Building needs no CEF distribution on any platform, so nothing is compiled from C++ either.

Outside a bundle, CEF finds its resources (the `.pak` files, `icudtl.dat`, the V8 snapshot and `locales/`) where the application names them (`resources_dir_path`, `locales_dir_path`), in the directory `find_cef_dir` returned.

### The Windows application manifest

CEF's Windows executables carry an application manifest; without it Windows tells Chromium it runs on Windows 8. `tetsu-build` embeds the same one from a build script:

```rust
fn main() {
    tetsu_build::embed_windows_manifest();
}
```

It does nothing on other targets. `tetsu_build::WINDOWS_MANIFEST` is the manifest itself.

### Features of `tetsu`

- `resources` (default): the names of Chromium's command and resource IDs, for `tetsu_sys::cef_id_for_command_id_name` and its siblings.
- `wrapper`: Rust ports of CEF's C++ wrapper, such as its message router and resource manager.
- `accelerated_osr`: imports the textures of accelerated off-screen rendering into `wgpu`.

### Cross-compiling to Windows

The `tetsu-sys` crate can be cross-compiled to `x86_64-pc-windows-msvc` from Linux with [cargo-xwin](https://github.com/rust-cross/cargo-xwin), which downloads the Windows SDK and links with `lld-link`. Install `clang`, `lld` and `llvm` from your package manager, then:

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin
cargo xwin build --target x86_64-pc-windows-msvc
```

Nothing is compiled from C++ and nothing of CEF is linked, so a cross-compiled build needs no CEF distribution. `cargo run -p export-cef-dir -- --target x86_64-pc-windows-msvc <dir>` exports the Windows one to run it with.

## Following CEF and cef-rs

tetsu takes cef-rs as a reference and never merges it. [UPSTREAM.md](UPSTREAM.md) says how cef-rs is reviewed and how tetsu moves to a new CEF.

## Contributing

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.
