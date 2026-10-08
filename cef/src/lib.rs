#![doc = include_str!("../README.md")]

pub mod args;
pub mod rc;
pub mod string;
pub mod window_info;

#[cfg(feature = "wrapper")]
pub mod wrapper;

#[cfg(target_os = "macos")]
pub mod application_mac;

#[cfg(target_os = "macos")]
pub mod sandbox;

#[cfg(feature = "accelerated_osr")]
pub mod osr_texture_import;

#[cfg(feature = "resources")]
pub mod resources;

#[rustfmt::skip]
mod bindings;
pub use bindings::*;

pub use rc::Rc as _;

pub use tetsu_sys as sys;

#[cfg(all(
    not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
    feature = "accelerated_osr"
))]
compile_error!("accelerated_osr not supported on this platform");

pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

#[cfg(test)]
fn test_init_cef() {
    use std::{ptr, sync::Once};

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let cef = sys::find_cef_dir().expect("CEF not found");
        unsafe { sys::load_libcef(&cef.libcef()) }.expect("cannot load libcef");

        assert_eq!(initialize(None, None, None, ptr::null_mut()), 0);
    });
}
