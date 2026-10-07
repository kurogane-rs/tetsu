#![doc = include_str!("../README.md")]

#[allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    clippy::all
)]
mod bindings;
pub use bindings::*;

pub mod libcef;
pub use libcef::{libcef_path, load_libcef, LoadError, LIBCEF_FILE};

#[cfg(target_os = "windows")]
impl Default for HWND {
    fn default() -> Self {
        Self(std::ptr::null_mut())
    }
}

#[cfg(target_os = "windows")]
impl Default for HINSTANCE {
    fn default() -> Self {
        Self(std::ptr::null_mut())
    }
}

use std::{
    env::{
        self,
        consts::{ARCH, OS},
    },
    fs,
    path::PathBuf,
};

pub fn get_cef_dir() -> Option<PathBuf> {
    let cef_path_env = env::var("FLATPAK")
        .map(|_| String::from("/usr/lib"))
        .or_else(|_| env::var("CEF_PATH"));

    match cef_path_env {
        Ok(cef_path) => {
            // Allow overriding the CEF path with environment variables.
            let configured_path = PathBuf::from(cef_path);
            let cef_dir = format!("cef_{OS}_{ARCH}");
            let package_version = env!("CARGO_PKG_VERSION");
            let cef_version = package_version
                .split_once('+')
                .map(|(_, version)| version)
                .unwrap_or(package_version);

            [
                configured_path.join(cef_version).join(&cef_dir),
                configured_path,
            ]
            .into_iter()
            .find_map(|path| fs::exists(&path).ok()?.then(|| path.canonicalize().ok()))
            .flatten()
        }
        Err(_) => {
            let cef_dir = cef_install_dir()?;
            fs::exists(&cef_dir).ok()?.then_some(cef_dir)
        }
    }
}

/// The user's shared installation of the CEF these bindings were generated
/// for, `tetsu/cef/<version>/cef_<os>_<arch>` under the local data directory
/// as `tetsu_download::install` writes it. An application started without a
/// CEF of its own finds one there; the directory may not exist.
pub fn cef_install_dir() -> Option<PathBuf> {
    let package_version = env!("CARGO_PKG_VERSION");
    let cef_version = package_version
        .split_once('+')
        .map(|(_, version)| version)
        .unwrap_or(package_version);

    dirs::data_local_dir().map(|dir| {
        dir.join("tetsu")
            .join("cef")
            .join(cef_version)
            .join(format!("cef_{OS}_{ARCH}"))
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cef_dir() {
        let _ = get_cef_dir().expect("CEF not found");
    }

    #[test]
    fn test_init() {
        use std::ptr::*;

        unsafe {
            load_libcef(&get_cef_dir().expect("CEF not found").join(LIBCEF_FILE))
                .expect("cannot load libcef");

            assert_eq!(cef_initialize(null(), null(), null_mut(), null_mut()), 0);
        };
    }
}
