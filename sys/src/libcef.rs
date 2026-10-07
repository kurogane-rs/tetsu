//! libcef, loaded when the application starts.
//!
//! On Linux and Windows the bindings link nothing at build time. Each function
//! libcef exports is resolved by name when [`load_libcef`] opens the library,
//! so building needs no CEF distribution and the application decides which
//! CEF it runs.

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
            Self::AlreadyLoaded { .. } => None,
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

/// Loads libcef from `path` (`libcef.dll`, `libcef.so`) and resolves the
/// functions it exports.
///
/// Comes before any other call into CEF. Loading the same path again does
/// nothing; another path once one is loaded is an error, since a process runs
/// one CEF.
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
    let _ = LOADED.set(Loaded {
        path: path.to_owned(),
        functions,
        _library: library,
    });

    Ok(())
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
