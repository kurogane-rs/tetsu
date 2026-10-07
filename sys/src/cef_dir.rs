//! CEF's directory, found when the application starts.
//!
//! An executable inside a macOS application bundle runs the framework the
//! bundle carries and no other. Any other executable runs a CEF beside it,
//! else the one `CEF_PATH` names, else the shared installation of the CEF
//! these bindings were generated for, which `tetsu_download::install` fills.

use std::{
    env::consts::{ARCH, OS},
    fmt, io,
    path::{Path, PathBuf},
};

use crate::LIBCEF_FILE;

/// A CEF directory and where it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefDir {
    /// The directory holding [`LIBCEF_FILE`] and CEF's resources, or a
    /// bundle's `Contents/Frameworks`.
    pub path: PathBuf,
    /// Where it was found.
    pub found_in: FoundIn,
}

impl CefDir {
    /// The library [`crate::load_libcef`] loads from this directory.
    pub fn libcef(&self) -> PathBuf {
        self.path.join(LIBCEF_FILE)
    }
}

/// Where [`find_cef_dir`] found CEF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundIn {
    /// The macOS application bundle the executable belongs to, whether or
    /// not its framework is in place.
    AppBundle,
    /// The executable's own directory.
    BesideExecutable,
    /// The directory `CEF_PATH` names.
    CefPath,
    /// The shared installation, [`cef_install_dir`].
    Installed,
}

impl fmt::Display for FoundIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AppBundle => "the application bundle",
            Self::BesideExecutable => "beside the executable",
            Self::CefPath => "CEF_PATH",
            Self::Installed => "the shared installation",
        })
    }
}

/// Why no CEF directory was found.
#[derive(Debug)]
pub enum FindError {
    /// `CEF_PATH` names no directory; no other CEF runs in its place.
    CefPathMissing(PathBuf),
    /// Nothing beside the executable, no `CEF_PATH` and no installation at
    /// `installed`, which is `None` for a user without a local data
    /// directory.
    NotInstalled { installed: Option<PathBuf> },
    /// The path of the running executable is unknown.
    CurrentExe(io::Error),
}

impl fmt::Display for FindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CefPathMissing(path) => {
                write!(
                    f,
                    "CEF_PATH names {}, which is not a directory",
                    path.display()
                )
            }
            Self::NotInstalled {
                installed: Some(path),
            } => write!(
                f,
                "no CEF beside the executable or in CEF_PATH, and CEF {} is not installed at {}",
                cef_version(),
                path.display()
            ),
            Self::NotInstalled { installed: None } => write!(
                f,
                "no CEF beside the executable or in CEF_PATH, and no local data directory to \
                 install CEF {} into",
                cef_version()
            ),
            Self::CurrentExe(_) => f.write_str("cannot locate the running executable"),
        }
    }
}

impl std::error::Error for FindError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentExe(source) => Some(source),
            Self::CefPathMissing(_) | Self::NotInstalled { .. } => None,
        }
    }
}

/// Finds the CEF the running application loads.
///
/// Looks in the macOS application bundle the executable belongs to, then
/// beside the executable, then in the directory `CEF_PATH` names (a set
/// `CEF_PATH` naming no directory is an error, never skipped), then in the
/// shared installation of the CEF these bindings were generated for
/// ([`cef_install_dir`]).
pub fn find_cef_dir() -> Result<CefDir, FindError> {
    let exe = std::env::current_exe().map_err(FindError::CurrentExe)?;
    let cef_path = std::env::var_os("CEF_PATH")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);

    find(&exe, cef_path, cef_install_dir())
}

fn find(
    exe: &Path,
    cef_path: Option<PathBuf>,
    installed: Option<PathBuf>,
) -> Result<CefDir, FindError> {
    let found = |path: PathBuf, found_in| Ok(CefDir { path, found_in });

    // Not checked for presence, as loading names what is missing
    if cfg!(target_os = "macos") {
        if let Some(frameworks) = bundle_frameworks(exe) {
            return found(frameworks, FoundIn::AppBundle);
        }
    }

    if let Some(dir) = exe.parent() {
        if dir.join(LIBCEF_FILE).exists() {
            return found(dir.to_path_buf(), FoundIn::BesideExecutable);
        }
    }

    if let Some(path) = cef_path {
        if !path.is_dir() {
            return Err(FindError::CefPathMissing(path));
        }
        return found(path, FoundIn::CefPath);
    }

    match installed {
        Some(path) if path.is_dir() => found(path, FoundIn::Installed),
        installed => Err(FindError::NotInstalled { installed }),
    }
}

/// The `Contents/Frameworks` of the application bundle `exe` belongs to; a
/// helper bundle sits in its application's.
fn bundle_frameworks(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;

    if macos.file_name()? != "MacOS"
        || contents.file_name()? != "Contents"
        || app.extension()? != "app"
    {
        return None;
    }

    let parent = app.parent()?;
    if parent.file_name()? == "Frameworks" && parent.parent()?.file_name()? == "Contents" {
        return Some(parent.to_path_buf());
    }

    Some(contents.join("Frameworks"))
}

/// The CEF version these bindings were generated for.
fn cef_version() -> &'static str {
    let package_version = env!("CARGO_PKG_VERSION");
    package_version
        .split_once('+')
        .map_or(package_version, |(_, version)| version)
}

/// The shared installation of the CEF these bindings were generated for,
/// `tetsu/cef/<version>/cef_<os>_<arch>` under the local data directory as
/// `tetsu_download::install` writes it; the directory may not exist.
pub fn cef_install_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| {
        dir.join("tetsu")
            .join("cef")
            .join(cef_version())
            .join(format!("cef_{OS}_{ARCH}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A fresh directory under the system's temporary directory.
    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tetsu-cef-dir-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn with_libcef(dir: &Path) -> PathBuf {
        let libcef = dir.join(LIBCEF_FILE);
        fs::create_dir_all(libcef.parent().unwrap()).unwrap();
        fs::write(&libcef, "").unwrap();
        dir.to_path_buf()
    }

    #[test]
    fn a_cef_beside_the_executable_comes_before_cef_path() {
        let dir = tmp("beside");
        let exe = with_libcef(&dir.join("bin")).join("app");
        let cef_path = with_libcef(&dir.join("cef"));

        let found = find(&exe, Some(cef_path), None).unwrap();

        assert_eq!(found.found_in, FoundIn::BesideExecutable);
        assert_eq!(found.path, dir.join("bin"));
    }

    #[test]
    fn cef_path_comes_before_the_installation() {
        let dir = tmp("cef-path");
        let cef_path = with_libcef(&dir.join("cef"));
        let installed = with_libcef(&dir.join("installed"));

        let found = find(&dir.join("app"), Some(cef_path.clone()), Some(installed)).unwrap();

        assert_eq!(
            found,
            CefDir {
                path: cef_path,
                found_in: FoundIn::CefPath
            }
        );
    }

    #[test]
    fn a_missing_cef_path_is_an_error_even_with_an_installation() {
        let dir = tmp("missing");
        let installed = with_libcef(&dir.join("installed"));

        let error = find(&dir.join("app"), Some(dir.join("nowhere")), Some(installed)).unwrap_err();

        assert!(matches!(error, FindError::CefPathMissing(path) if path == dir.join("nowhere")));
    }

    #[test]
    fn the_installation_is_the_last_place_looked() {
        let dir = tmp("installed");
        let installed = with_libcef(&dir.join("installed"));

        let found = find(&dir.join("app"), None, Some(installed.clone())).unwrap();
        assert_eq!(
            found,
            CefDir {
                path: installed,
                found_in: FoundIn::Installed
            }
        );

        let error = find(&dir.join("app"), None, Some(dir.join("absent"))).unwrap_err();
        assert!(matches!(
            error,
            FindError::NotInstalled { installed: Some(_) }
        ));
    }

    #[test]
    fn a_bundle_and_its_helpers_share_the_application_s_frameworks() {
        let app = Path::new("/Applications/My App.app/Contents");
        assert_eq!(
            bundle_frameworks(&app.join("MacOS/My App")),
            Some(app.join("Frameworks"))
        );
        assert_eq!(
            bundle_frameworks(
                &app.join("Frameworks/My App Helper.app/Contents/MacOS/My App Helper")
            ),
            Some(app.join("Frameworks"))
        );
        assert_eq!(bundle_frameworks(Path::new("/usr/local/bin/app")), None);
    }

    #[test]
    fn the_installation_is_named_for_the_bindings_version() {
        if let Some(dir) = cef_install_dir() {
            assert!(dir.ends_with(Path::new(cef_version()).join(format!("cef_{OS}_{ARCH}"))));
        }
    }
}
