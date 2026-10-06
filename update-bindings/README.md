# update-bindings

Download the prebuilt [Chromium Embedded Framework](https://github.com/chromiumembedded/cef)
archive on any supported platform and run `bindgen` on the C API for the `tetsu-sys` crate,
then regenerate the safe bindings in the `tetsu` crate.

You can find the latest version of the prebuilt CEF archives on the [Chromium Embedded Framework
(CEF) Automated Builds](https://cef-builds.spotifycdn.com/index.html).
