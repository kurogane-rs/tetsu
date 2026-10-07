//! The framework of a macOS application bundle.
//!
//! Finds the Chromium Embedded Framework a bundled executable runs with and
//! loads it through [`tetsu_sys::load_libcef`].

use crate::sys;

pub struct LibraryLoader {
    path: std::path::PathBuf,
}

impl LibraryLoader {
    /// The framework of the bundle the executable at `path` belongs to; a
    /// helper sits three levels deeper than the main executable.
    pub fn new(path: &std::path::Path, helper: bool) -> Self {
        let resolver = if helper { "../../.." } else { "../Frameworks" };
        let path = path
            // path is the current_exe path, read the parent to support symlinks
            .parent()
            .unwrap()
            .join(resolver)
            .join(sys::LIBCEF_FILE)
            .canonicalize()
            .unwrap();

        Self { path }
    }

    /// Loads the framework; false when it cannot be loaded.
    pub fn load(&self) -> bool {
        // SAFETY: the framework is loaded before any other call into CEF
        unsafe { sys::load_libcef(&self.path) }.is_ok()
    }
}
