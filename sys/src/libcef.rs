//! libcef, loaded when the application starts.
//!
//! The bindings link nothing at build time. Each function
//! libcef exports is resolved by name when [`load_libcef`] opens the library,
//! so building needs no CEF distribution and the application decides which
//! CEF it runs. The library must be the CEF build the bindings were generated
//! from, which loading checks.

use std::{
    ffi::CStr,
    fmt,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, PoisonError},
};

use crate::LibcefFunctions;

/// The library's file name in a CEF distribution's root.
#[cfg(target_os = "windows")]
pub const LIBCEF_FILE: &str = "libcef.dll";
/// The library's file name in a CEF distribution's root.
#[cfg(target_os = "linux")]
pub const LIBCEF_FILE: &str = "libcef.so";
/// The library's path in a CEF distribution's root, or in an application
/// bundle's `Contents/Frameworks`.
#[cfg(target_os = "macos")]
pub const LIBCEF_FILE: &str = "Chromium Embedded Framework.framework/Chromium Embedded Framework";

/// libcef, opened once and never closed (CEF cannot be unloaded)
pub struct Library(libloading::Library);

impl Library {
    /// Resolves an exported symbol as a value of `T`.
    ///
    /// # Safety
    ///
    /// `T` must be the symbol's type.
    pub(crate) unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, LoadError> {
        unsafe { self.0.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|source| LoadError::Symbol {
                name: name.to_string_lossy().into_owned(),
                source,
            })
    }
}

/// Why libcef could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// The library at `path` could not be opened.
    Open {
        path: PathBuf,
        source: libloading::Error,
    },
    /// The library lacks a function the bindings call, so it is not the CEF
    /// they were generated for.
    Symbol {
        name: String,
        source: libloading::Error,
    },
    /// The library at `path` is another CEF build, commit `found` rather
    /// than `expected`, the `CEF_VERSION` the bindings were generated from.
    VersionMismatch {
        path: PathBuf,
        found: String,
        expected: String,
    },
    /// The process already runs another libcef.
    AlreadyLoaded { loaded: PathBuf, requested: PathBuf },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, .. } => write!(f, "cannot load {}", path.display()),
            Self::Symbol { name, .. } => write!(
                f,
                "libcef has no {name}; it is not CEF {}.{}.{}",
                crate::CEF_VERSION_MAJOR,
                crate::CEF_VERSION_MINOR,
                crate::CEF_VERSION_PATCH
            ),
            Self::VersionMismatch {
                path,
                found,
                expected,
            } => write!(
                f,
                "{} is CEF commit {found}, not CEF {expected} which these bindings were generated for",
                path.display()
            ),
            Self::AlreadyLoaded { loaded, requested } => write!(
                f,
                "cannot load {}: this process already runs {}",
                requested.display(),
                loaded.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. } | Self::Symbol { source, .. } => Some(source),
            Self::VersionMismatch { .. } | Self::AlreadyLoaded { .. } => None,
        }
    }
}

struct Loaded {
    path: PathBuf,
    functions: LibcefFunctions,
    // Keeps the functions' code mapped; never dropped, as the static holds it
    _library: Library,
}

static LOADED: OnceLock<Loaded> = OnceLock::new();
static LOADING: Mutex<()> = Mutex::new(());

/// Loads libcef from `path` (`libcef.dll`, `libcef.so`, the framework's binary
/// on macOS, see [`crate::find_cef_dir`]) and resolves the functions it
/// exports.
///
/// Comes before any other call into CEF. Fixes the process's CEF API version
/// to the bindings' (`CEF_API_VERSION_LAST`), as the first `cef_api_hash`
/// call does, and refuses a library that is not the CEF build the bindings
/// were generated from. Loading the same path again does nothing; another
/// path once one is loaded is an error, since a process runs one CEF.
///
/// # Safety
///
/// Opening the library runs its initializers.
pub unsafe fn load_libcef(path: &Path) -> Result<(), LoadError> {
    let _loading = LOADING.lock().unwrap_or_else(PoisonError::into_inner);

    if let Some(loaded) = LOADED.get() {
        if loaded.path == path {
            return Ok(());
        }
        return Err(LoadError::AlreadyLoaded {
            loaded: loaded.path.clone(),
            requested: path.to_owned(),
        });
    }

    let library = unsafe { open(path) }.map_err(|source| LoadError::Open {
        path: path.to_owned(),
        source,
    })?;
    let functions = LibcefFunctions::resolve(&library)?;
    if let Err(found) = unsafe { check_build(&functions) } {
        // Its initializers ran, so it stays mapped rather than being unloaded
        std::mem::forget(library);
        return Err(LoadError::VersionMismatch {
            path: path.to_owned(),
            found,
            expected: bindings_version().to_owned(),
        });
    }
    let _ = LOADED.set(Loaded {
        path: path.to_owned(),
        functions,
        _library: library,
    });

    Ok(())
}

/// Fixes the API version and compares the library's commit with the one in
/// `CEF_VERSION`, returning the library's commit when they differ.
///
/// # Safety
///
/// `functions` are a loaded libcef's.
unsafe fn check_build(functions: &LibcefFunctions) -> Result<(), String> {
    // Only the first call's version counts, for the whole process
    unsafe { (functions.cef_api_hash)(crate::CEF_API_VERSION_LAST, 0) };

    // Entry 2 is the library's CEF_COMMIT_HASH
    let found = unsafe { (functions.cef_api_hash)(crate::CEF_API_VERSION_LAST, 2) };
    let found = if found.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(found) }
            .to_string_lossy()
            .into_owned()
    };

    let expected = bindings_commit();
    if !expected.is_empty() && found.starts_with(expected) {
        Ok(())
    } else {
        Err(found)
    }
}

/// `CEF_VERSION` of the bindings, such as
/// `154.0.33+ga03e714+chromium-154.0.8037.94`.
fn bindings_version() -> &'static str {
    CStr::from_bytes_with_nul(crate::CEF_VERSION)
        .ok()
        .and_then(|version| version.to_str().ok())
        .unwrap_or_default()
}

/// The abbreviated commit in `CEF_VERSION`, `a03e714` in
/// `154.0.33+ga03e714+chromium-154.0.8037.94`.
fn bindings_commit() -> &'static str {
    bindings_version()
        .split('+')
        .nth(1)
        .and_then(|commit| commit.strip_prefix('g'))
        .unwrap_or_default()
}

/// The libcef this process loaded, once [`load_libcef`] has.
pub fn libcef_path() -> Option<&'static Path> {
    LOADED.get().map(|loaded| loaded.path.as_path())
}

#[inline]
pub(crate) fn functions() -> &'static LibcefFunctions {
    match LOADED.get() {
        Some(loaded) => &loaded.functions,
        None => not_loaded(),
    }
}

#[cold]
fn not_loaded() -> ! {
    panic!("libcef is not loaded; call tetsu_sys::load_libcef before any other CEF function")
}

#[cfg(target_os = "windows")]
unsafe fn open(path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::windows;

    // libcef's own DLLs (chrome_elf.dll and the rest) load from its directory
    unsafe { windows::Library::load_with_flags(path, windows::LOAD_WITH_ALTERED_SEARCH_PATH) }
        .map(|library| Library(library.into()))
}

#[cfg(target_os = "linux")]
unsafe fn open(path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::unix;

    unsafe { unix::Library::open(Some(path), unix::RTLD_NOW | unix::RTLD_LOCAL) }
        .map(|library| Library(library.into()))
}

#[cfg(target_os = "macos")]
unsafe fn open(path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::unix;

    // Lazily bound, as CEF's own framework loader opens it
    unsafe { unix::Library::open(Some(path), unix::RTLD_LAZY | unix::RTLD_LOCAL) }
        .map(|library| Library(library.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crate_version_names_the_bindings_cef_version() {
        let build = env!("CARGO_PKG_VERSION")
            .split_once('+')
            .map(|(_, build)| build);
        let version = format!(
            "{}.{}.{}",
            crate::CEF_VERSION_MAJOR,
            crate::CEF_VERSION_MINOR,
            crate::CEF_VERSION_PATCH
        );

        assert_eq!(build, Some(version.as_str()));
        assert!(bindings_version().starts_with(&format!("{version}+")));
    }

    #[test]
    fn the_bindings_name_their_commit() {
        let commit = bindings_commit();

        assert!(
            commit.len() >= 7,
            "{:?} names no commit",
            bindings_version()
        );
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
