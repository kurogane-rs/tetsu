//! Build-script helpers for applications on tetsu.
//!
//! What CEF documents an application's build must give its executables, for
//! a build script to call.

use std::{env, fs, path::PathBuf};

/// Application manifest CEF's own Windows executables carry.
///
/// It declares every Windows version from Vista to 11, Common Controls 6 and
/// the `asInvoker` execution level. Without the Windows 10 entry Windows
/// tells the process, and Chromium in it (`base::win::OSInfo` reads
/// `GetVersionEx`), that it runs on Windows 8: version 6.2 build 9200 where
/// the system is 10.0 build 26200, as measured. CEF's `bootstrap.exe`
/// embeds it already.
pub const WINDOWS_MANIFEST: &str = include_str!("windows.manifest");

/// Embeds [`WINDOWS_MANIFEST`] in the package's Windows executables, from a
/// build script.
///
/// Does nothing unless the target is Windows with the MSVC toolchain. The
/// linker embeds the manifest, so a resource script must not add one of its
/// own.
///
/// # Panics
///
/// Panics outside a build script, where Cargo sets no `OUT_DIR`, and when
/// the manifest cannot be written there.
pub fn embed_windows_manifest() {
    let out_dir = env::var_os("OUT_DIR")
        .expect("embed_windows_manifest runs in a build script, which Cargo gives an OUT_DIR");

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let toolchain = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" || toolchain != "msvc" {
        return;
    }

    let path = PathBuf::from(out_dir).join("windows.manifest");
    fs::write(&path, WINDOWS_MANIFEST)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));

    // For every target of the package: an argument for one kind alone is
    // refused in a package without that kind
    println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo::rustc-link-arg=/MANIFESTUAC:NO");
    println!("cargo::rustc-link-arg=/MANIFESTINPUT:{}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_declares_windows_10_and_one_trust_info() {
        assert!(WINDOWS_MANIFEST
            .contains(r#"<supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>"#));
        assert_eq!(WINDOWS_MANIFEST.matches("<trustInfo").count(), 1);
    }
}
