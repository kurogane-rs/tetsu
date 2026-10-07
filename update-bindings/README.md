# update-bindings

Download the prebuilt [Chromium Embedded Framework](https://github.com/chromiumembedded/cef)
archive on any supported platform and run `bindgen` on the C API for the `tetsu-sys` crate,
then regenerate the safe bindings in the `tetsu` crate.

For Linux and Windows it then rewrites the `tetsu-sys` bindings (`src/loader.rs`): each function
libcef exports becomes a function of the same name and signature that calls through a table the
application fills when it loads libcef (`tetsu_sys::load_libcef`). Run without `--bindgen`, it
rewrites bindings bindgen or upstream left as `extern "C"` declarations and leaves rewritten ones
as they are; when upstream changes a bindings file, take upstream's file and run it again.

You can find the latest version of the prebuilt CEF archives on the [Chromium Embedded Framework
(CEF) Automated Builds](https://cef-builds.spotifycdn.com/index.html).
