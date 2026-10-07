//! Links what tetsu's own code needs; nothing of CEF.
//!
//! The application loads libcef when it starts (`tetsu_sys::load_libcef`),
//! so a build reads no CEF distribution.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    // tetsu's macOS application code uses AppKit
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo::rustc-link-lib=framework=AppKit");
    }
}
