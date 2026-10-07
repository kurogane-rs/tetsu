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

mod cef_dir;
pub use cef_dir::{cef_install_dir, find_cef_dir, CefDir, FindError, FoundIn};

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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_init() {
        use std::ptr::*;

        let cef = find_cef_dir().expect("CEF not found");
        unsafe {
            load_libcef(&cef.libcef()).expect("cannot load libcef");

            assert_eq!(cef_initialize(null(), null(), null_mut(), null_mut()), 0);
        };
    }
}
