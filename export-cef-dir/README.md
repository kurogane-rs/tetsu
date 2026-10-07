# export-cef-dir

Export files from the prebuilt [Chromium Embedded Framework](https://github.com/chromiumembedded/cef)
archive on any supported platform, laid out as `tetsu_sys::find_cef_dir` and `tetsu_sys::load_libcef`
expect them.

Without an output directory, the distribution goes into tetsu's shared installation,
`tetsu/cef/<version>/cef_<os>_<arch>` under the local data directory, where an application
started without `CEF_PATH` finds it:

```sh
cargo run -p export-cef-dir
```

With one, it goes there instead; point `CEF_PATH` at that directory when starting the application:

```sh
cargo run -p export-cef-dir -- --force ~/.local/share/cef
export CEF_PATH=~/.local/share/cef
```

libcef needs no library search path; the application loads it from the directory it finds.
