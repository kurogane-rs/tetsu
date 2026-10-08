//! macOS Chromium sandbox for helper processes.
//!
//! A helper enters its seatbelt sandbox through `libcef_sandbox.dylib` before
//! it loads the CEF framework.

use libloading::Library;
use std::{
    ffi::{c_char, c_int, c_void},
    fmt, io,
    path::PathBuf,
    ptr::NonNull,
};

use crate::MainArgs;

/// Why a helper could not enter the sandbox.
#[derive(Debug)]
pub enum SandboxError {
    /// The path of the running executable is unknown.
    CurrentExe(io::Error),
    /// The sandbox library at `path` could not be loaded.
    Load {
        path: PathBuf,
        source: libloading::Error,
    },
    /// The sandbox library lacks the function `name`.
    Symbol {
        name: &'static str,
        source: libloading::Error,
    },
    /// `cef_sandbox_initialize` returned no sandbox context.
    Initialize,
    /// The helper tried to enter the sandbox already.
    AlreadyAttempted,
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentExe(_) => f.write_str("cannot locate the running executable"),
            Self::Load { path, .. } => {
                write!(f, "cannot load the sandbox library {}", path.display())
            }
            Self::Symbol { name, .. } => write!(f, "the sandbox library has no {name}"),
            Self::Initialize => f.write_str("cef_sandbox_initialize returned no sandbox context"),
            Self::AlreadyAttempted => f.write_str("the helper tried to enter the sandbox already"),
        }
    }
}

impl std::error::Error for SandboxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentExe(source) => Some(source),
            Self::Load { source, .. } | Self::Symbol { source, .. } => Some(source),
            Self::Initialize | Self::AlreadyAttempted => None,
        }
    }
}

type InitializeFn = unsafe extern "C" fn(argc: c_int, argv: *mut *mut c_char) -> *mut c_void;
type DestroyFn = unsafe extern "C" fn(context: *mut c_void);

/// Seatbelt sandbox of a helper process, left when dropped.
pub struct Sandbox {
    initialize: InitializeFn,
    destroy: DestroyFn,
    attempted: bool,
    context: Option<NonNull<c_void>>,
    // Keeps both functions' code mapped
    _lib: Library,
}

// SAFETY: the context is an opaque handle that only `cef_sandbox_destroy`
// takes, from `Drop`; the functions are plain C entry points
unsafe impl Send for Sandbox {}

// SAFETY: a shared reference reaches neither the context nor the functions
unsafe impl Sync for Sandbox {}

impl Sandbox {
    /// Sandbox library, relative to the helper executable's directory.
    const LIBCEF_SANDBOX_PATH: &str =
        "../../../Chromium Embedded Framework.framework/Libraries/libcef_sandbox.dylib";

    /// Loads the sandbox library of the application bundle the helper belongs
    /// to.
    pub fn new() -> Result<Self, SandboxError> {
        let exe = std::env::current_exe().map_err(SandboxError::CurrentExe)?;

        // Beside the executable's own path, so a symlinked helper finds its bundle
        let path = exe.with_file_name(Self::LIBCEF_SANDBOX_PATH);

        // SAFETY: the sandbox library's initializers need no prior state
        let lib =
            unsafe { Library::new(&path) }.map_err(|source| SandboxError::Load { path, source })?;

        // SAFETY: the types match `cef_sandbox_mac.h`
        let (initialize, destroy) = unsafe {
            (
                symbol::<InitializeFn>(&lib, "cef_sandbox_initialize")?,
                symbol::<DestroyFn>(&lib, "cef_sandbox_destroy")?,
            )
        };

        Ok(Self {
            initialize,
            destroy,
            attempted: false,
            context: None,
            _lib: lib,
        })
    }

    /// Enters the sandbox, before the helper loads the CEF framework.
    ///
    /// A failed attempt is never repeated, since a partly initialized seatbelt
    /// cannot be rolled back.
    pub fn initialize(&mut self, args: &MainArgs) -> Result<(), SandboxError> {
        if self.attempted {
            return Err(SandboxError::AlreadyAttempted);
        }
        self.attempted = true;

        // SAFETY: `args` holds the process's `argc` and `argv`
        let context = unsafe { (self.initialize)(args.argc, args.argv) };
        self.context = Some(NonNull::new(context).ok_or(SandboxError::Initialize)?);

        Ok(())
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Some(context) = self.context {
            // SAFETY: `context` came from `cef_sandbox_initialize` and is destroyed once
            unsafe { (self.destroy)(context.as_ptr()) };
        }
    }
}

/// Resolves a function the sandbox library exports.
///
/// # Safety
///
/// `T` must be the function's type.
unsafe fn symbol<T: Copy>(lib: &Library, name: &'static str) -> Result<T, SandboxError> {
    unsafe { lib.get::<T>(name.as_bytes()) }
        .map(|symbol| *symbol)
        .map_err(|source| SandboxError::Symbol { name, source })
}
