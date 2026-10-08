#![doc = include_str!("../README.md")]

use bzip2::bufread::BzDecoder;
use fs_err::File;
use serde::{Deserialize, Serialize};
use sha1_smol::Sha1;
use std::{
    collections::HashMap,
    env,
    fmt::{self, Display},
    io::{self, BufReader, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

#[macro_use]
extern crate thiserror;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unsupported target triplet: {0}")]
    UnsupportedTarget(String),
    #[error("HTTP request error: {0}")]
    Request(#[from] ureq::Error),
    #[error("Version not found: {0}")]
    VersionNotFound(String),
    #[error("Missing Content-Length header")]
    MissingContentLength,
    #[error("Opaque Content-Length header: {0}")]
    OpaqueContentLength(#[from] ureq::http::header::ToStrError),
    #[error("Invalid Content-Length header: {0}")]
    InvalidContentLength(String),
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unexpected file size: downloaded {downloaded} expected {expected}")]
    UnexpectedFileSize { downloaded: u64, expected: u64 },
    #[error("Bad SHA1 file hash: {0}")]
    CorruptedFile(String),
    #[error("Invalid archive file path: {0}")]
    InvalidArchiveFile(String),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{} has no archive.json naming its CEF build", .dir.display())]
    NoArchiveJson { dir: PathBuf },
    #[error("The archive.json in {} names {name}, which is not a CEF archive", .dir.display())]
    UnknownArchive { dir: PathBuf, name: String },
    #[error("{} is CEF {found}, not CEF {expected}", .dir.display())]
    VersionMismatch {
        dir: PathBuf,
        found: String,
        expected: String,
    },
    #[error("{} is CEF for {found}, not for {expected}", .dir.display())]
    PlatformMismatch {
        dir: PathBuf,
        found: String,
        expected: String,
    },
    #[error("This user has no local data directory to install CEF into")]
    NoDataDir,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Rust target CEF publishes builds for.
struct Target {
    triple: &'static str,
    /// CEF's platform name, such as `windows64`.
    platform: &'static str,
    os: &'static str,
    arch: &'static str,
}

const TARGETS: &[Target] = &[
    Target {
        triple: "aarch64-apple-darwin",
        platform: "macosarm64",
        os: "macos",
        arch: "aarch64",
    },
    Target {
        triple: "x86_64-apple-darwin",
        platform: "macosx64",
        os: "macos",
        arch: "x86_64",
    },
    Target {
        triple: "x86_64-pc-windows-msvc",
        platform: "windows64",
        os: "windows",
        arch: "x86_64",
    },
    Target {
        triple: "aarch64-pc-windows-msvc",
        platform: "windowsarm64",
        os: "windows",
        arch: "aarch64",
    },
    Target {
        triple: "i686-pc-windows-msvc",
        platform: "windows32",
        os: "windows",
        arch: "x86",
    },
    Target {
        triple: "x86_64-unknown-linux-gnu",
        platform: "linux64",
        os: "linux",
        arch: "x86_64",
    },
    Target {
        triple: "aarch64-unknown-linux-gnu",
        platform: "linuxarm64",
        os: "linux",
        arch: "aarch64",
    },
    Target {
        triple: "arm-unknown-linux-gnueabi",
        platform: "linuxarm",
        os: "linux",
        arch: "arm",
    },
];

fn find_target(triple: &str) -> Result<&'static Target> {
    TARGETS
        .iter()
        .find(|target| target.triple == triple)
        .ok_or_else(|| Error::UnsupportedTarget(triple.to_string()))
}

/// Returns the Rust targets CEF publishes builds for.
pub fn supported_targets() -> impl Iterator<Item = &'static str> {
    TARGETS.iter().map(|target| target.triple)
}

/// Returns CEF's platform name for a Rust target, such as `windows64` for
/// `x86_64-pc-windows-msvc`.
pub fn cef_platform_name(target: &str) -> Result<&'static str> {
    find_target(target).map(|target| target.platform)
}

/// Returns the CEF version a crate version's build metadata names, `154.0.33`
/// in `154.4.0+154.0.33`; a version without build metadata names itself.
pub fn default_version(version: &str) -> String {
    version
        .split_once('+')
        .map_or(version, |(_, cef_version)| cef_version)
        .to_string()
}

/// CEF build an installation's `archive.json` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    /// CEF version, such as `154.0.33`.
    pub cef_version: String,
    /// Chromium version, such as `154.0.8037.94`.
    pub chromium_version: String,
    /// CEF platform name, such as `windows64`.
    pub platform: String,
    /// Archive file name.
    pub name: String,
}

impl Archive {
    /// Parses a CEF archive name, such as
    /// `cef_binary_154.0.33+ga03e714+chromium-154.0.8037.94_windows64_minimal.tar.bz2`.
    fn parse(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(".tar.bz2").unwrap_or(name);
        let (cef_version, rest) = stem.strip_prefix("cef_binary_")?.split_once('+')?;
        let (_commit, rest) = rest.split_once("+chromium-")?;
        let (chromium_version, rest) = rest.split_once('_')?;
        let (platform, distribution) = rest.split_once('_')?;

        let parts = [cef_version, chromium_version, platform, distribution];
        if parts.iter().any(|part| part.is_empty()) {
            return None;
        }

        Some(Self {
            cef_version: cef_version.to_string(),
            chromium_version: chromium_version.to_string(),
            platform: platform.to_string(),
            name: name.to_string(),
        })
    }
}

/// Reads the CEF build an installation's `archive.json` names; `None` when the
/// directory has no `archive.json`.
pub fn read_archive_json(dir: &Path) -> Result<Option<Archive>> {
    let file = match File::open(archive_json_path(dir)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let file: CefFile = serde_json::from_reader(BufReader::new(file))?;

    Archive::parse(&file.name)
        .map(Some)
        .ok_or_else(|| Error::UnknownArchive {
            dir: dir.to_path_buf(),
            name: file.name,
        })
}

/// Checks that an installation's `archive.json` names exactly CEF
/// `cef_version` (such as `154.0.33`) for `target`.
///
/// libcef is loaded at run time and must be the build the bindings were
/// generated from, so an older or newer archive is refused.
pub fn check_archive_json(dir: &Path, cef_version: &str, target: &str) -> Result<Archive> {
    let platform = cef_platform_name(target)?;
    let archive = read_archive_json(dir)?.ok_or_else(|| Error::NoArchiveJson {
        dir: dir.to_path_buf(),
    })?;

    if archive.cef_version != cef_version {
        return Err(Error::VersionMismatch {
            dir: dir.to_path_buf(),
            found: archive.cef_version,
            expected: cef_version.to_string(),
        });
    }

    if archive.platform != platform {
        return Err(Error::PlatformMismatch {
            dir: dir.to_path_buf(),
            found: archive.platform,
            expected: platform.to_string(),
        });
    }

    Ok(archive)
}

fn archive_json_path<P>(location: P) -> PathBuf
where
    P: AsRef<Path>,
{
    let location = location.as_ref().join("archive.json");
    std::path::absolute(&location).unwrap_or(location)
}

pub const DEFAULT_CDN_URL: &str = "https://cef-builds.spotifycdn.com";

pub fn default_download_url() -> String {
    env::var("CEF_DOWNLOAD_URL").unwrap_or(DEFAULT_CDN_URL.to_owned())
}

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
}

impl FromStr for Channel {
    type Err = String;

    fn from_str(channel: &str) -> std::result::Result<Self, Self::Err> {
        match channel {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            other => Err(format!("unknown channel {other}, expected stable or beta")),
        }
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Stable => write!(f, "stable"),
            Channel::Beta => write!(f, "beta"),
        }
    }
}

/// CEF's build index, each platform's builds by CEF's platform name.
#[derive(Deserialize, Serialize, Default)]
#[serde(transparent)]
pub struct CefIndex {
    pub platforms: HashMap<String, CefPlatform>,
}

impl CefIndex {
    pub fn download(url: &str) -> Result<Self> {
        Ok(ureq::get(&format!("{url}/index.json"))
            .call()?
            .into_body()
            .read_json()?)
    }

    pub fn platform(&self, target: &str) -> Result<&CefPlatform> {
        self.platforms
            .get(cef_platform_name(target)?)
            .ok_or_else(|| Error::UnsupportedTarget(target.to_string()))
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct CefPlatform {
    pub versions: Vec<CefVersion>,
}

impl CefPlatform {
    /// Returns the newest build of `channel`.
    pub fn latest(&self, channel: &Channel) -> Result<&CefVersion> {
        self.versions
            .iter()
            .filter(|version| version.channel == *channel)
            .filter_map(|version| Some((numeric_version(&version.cef_version)?, version)))
            .max_by(|(a, _), (b, _)| a.cmp(b))
            .map(|(_, version)| version)
            .ok_or_else(|| Error::VersionNotFound(format!("the newest {channel} build")))
    }

    pub fn version(&self, cef_version: &str) -> Result<&CefVersion> {
        let version_prefix = format!("{cef_version}+");
        self.versions
            .iter()
            .find(|v| v.cef_version.starts_with(&version_prefix))
            .ok_or_else(|| Error::VersionNotFound(cef_version.to_string()))
    }
}

/// Returns the numbers of a CEF version, `[154, 0, 33]` for
/// `154.0.33+ga03e714+chromium-154.0.8037.94`.
fn numeric_version(cef_version: &str) -> Option<Vec<u64>> {
    let version = cef_version
        .split_once('+')
        .map_or(cef_version, |(version, _)| version);
    version.split('.').map(|part| part.parse().ok()).collect()
}

#[derive(Deserialize, Serialize)]
pub struct CefVersion {
    pub channel: Channel,
    pub cef_version: String,
    pub files: Vec<CefFile>,
}

impl CefVersion {
    pub fn download_archive<P>(
        &self,
        url: &str,
        location: P,
        show_progress: bool,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        let file = self.minimal()?;
        let (file, sha) = (file.name.as_str(), file.sha1.as_str());

        fs_err::create_dir_all(&location)?;
        let download_file = location.as_ref().join(file);

        if download_file.exists() {
            if calculate_file_sha1(&download_file)? == sha {
                if show_progress {
                    println!("Verified archive: {}", download_file.display());
                }
                return Ok(download_file);
            }

            if show_progress {
                println!("Cleaning corrupted archive: {}", download_file.display());
            }
            let corrupted_file = location.as_ref().join(format!("corrupted_{file}"));
            fs_err::rename(&download_file, &corrupted_file)?;
            fs_err::remove_file(&corrupted_file)?;
        }

        let cef_url = format!("{url}/{file}");
        if show_progress {
            println!("Using archive url: {cef_url}");
        }

        let mut file = File::create(&download_file)?;

        let resp = ureq::get(&cef_url).call()?;
        let expected = resp
            .headers()
            .get("Content-Length")
            .ok_or(Error::MissingContentLength)?;
        let expected = expected.to_str()?;
        let expected = expected
            .parse::<u64>()
            .map_err(|_| Error::InvalidContentLength(expected.to_owned()))?;

        let downloaded = if show_progress && io::stdout().is_terminal() {
            const DOWNLOAD_TEMPLATE: &str = "{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})";

            let bar = indicatif::ProgressBar::new(expected);
            bar.set_style(
                indicatif::ProgressStyle::with_template(DOWNLOAD_TEMPLATE)
                    .expect("invalid template")
                    .progress_chars("##-"),
            );
            bar.set_message("Downloading");
            std::io::copy(
                &mut bar.wrap_read(resp.into_body().into_reader()),
                &mut file,
            )
        } else {
            let mut reader = resp.into_body().into_reader();
            std::io::copy(&mut reader, &mut file)
        }?;

        if downloaded != expected {
            return Err(Error::UnexpectedFileSize {
                downloaded,
                expected,
            });
        }

        if show_progress {
            println!("Verifying SHA1 hash: {sha}...");
        }
        if calculate_file_sha1(&download_file)? != sha {
            return Err(Error::CorruptedFile(download_file.display().to_string()));
        }

        if show_progress {
            println!("Downloaded archive: {}", download_file.display());
        }
        Ok(download_file)
    }

    pub fn download_archive_with_retry<P>(
        &self,
        url: &str,
        location: P,
        show_progress: bool,
        retry_delay: Duration,
        max_retries: u32,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        let mut result = self.download_archive(url, &location, show_progress);

        let mut retry = 0;
        while let Err(Error::Io(_)) = &result {
            if retry >= max_retries {
                break;
            }

            retry += 1;
            thread::sleep(retry_delay * retry);

            result = self.download_archive(url, &location, show_progress);
        }

        result
    }

    pub fn minimal(&self) -> Result<&CefFile> {
        self.files
            .iter()
            .find(|f| f.file_type == "minimal")
            .ok_or_else(|| Error::VersionNotFound(self.cef_version.clone()))
    }

    pub fn write_archive_json<P>(&self, location: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.minimal()?.write_archive_json(location)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CefFile {
    #[serde(rename = "type")]
    pub file_type: String,
    pub name: String,
    pub sha1: String,
}

impl CefFile {
    pub fn write_archive_json<P>(&self, location: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let archive_version = serde_json::to_string_pretty(self)?;
        let mut archive_json = File::create(archive_json_path(location))?;
        archive_json.write_all(archive_version.as_bytes())?;
        Ok(())
    }
}

impl TryFrom<&Path> for CefFile {
    type Error = Error;

    fn try_from(location: &Path) -> Result<Self> {
        let file_type = "minimal".to_string();
        let name = location
            .file_name()
            .map(|f| f.display().to_string())
            .ok_or_else(|| Error::InvalidArchiveFile(location.display().to_string()))?;
        let sha1 = calculate_file_sha1(location)?;
        Ok(Self {
            file_type,
            name,
            sha1,
        })
    }
}

pub fn download_target_archive<P>(
    url: &str,
    target: &str,
    cef_version: &str,
    location: P,
    show_progress: bool,
) -> Result<PathBuf>
where
    P: AsRef<Path>,
{
    if show_progress {
        println!("Downloading CEF archive for {target}...");
    }

    let index = CefIndex::download(url)?;
    let platform = index.platform(target)?;
    let version = platform.version(cef_version)?;

    version.download_archive_with_retry(url, location, show_progress, Duration::from_secs(15), 3)
}

pub fn extract_target_archive<P, Q>(
    target: &str,
    archive: P,
    location: Q,
    show_progress: bool,
) -> Result<PathBuf>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let archive = archive.as_ref();
    if show_progress {
        println!("Extracting archive: {}", archive.display());
    }
    let decoder = BzDecoder::new(BufReader::new(File::open(archive)?));
    tar::Archive::new(decoder).unpack(&location)?;

    let extracted_dir = archive
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".tar.bz2"))
        .ok_or_else(|| Error::InvalidArchiveFile(archive.display().to_string()))?;
    let extracted_dir = location.as_ref().join(extracted_dir);

    let os_and_arch = OsAndArch::try_from(target)?;
    let OsAndArch { os, arch } = os_and_arch;
    let cef_dir = os_and_arch.to_string();
    let cef_dir = location.as_ref().join(cef_dir);

    if cef_dir.exists() {
        let old_dir = location.as_ref().join(format!("old_{os}_{arch}"));
        if show_progress {
            println!("Cleaning up: {}", old_dir.display());
        }
        fs_err::rename(&cef_dir, &old_dir)?;
        fs_err::remove_dir_all(old_dir)?;
    }
    const RELEASE_DIR: &str = "Release";
    fs_err::rename(extracted_dir.join(RELEASE_DIR), &cef_dir)?;

    if os != "macos" {
        let resources = extracted_dir.join("Resources");

        for entry in fs_err::read_dir(&resources)? {
            let entry = entry?;
            fs_err::rename(entry.path(), cef_dir.join(entry.file_name()))?;
        }
    }

    // Build files for update-bindings, then CEF's notices
    for name in [
        "CMakeLists.txt",
        "cmake",
        "include",
        "libcef_dll",
        "CREDITS.html",
        "LICENSE.txt",
    ] {
        fs_err::rename(extracted_dir.join(name), cef_dir.join(name))?;
    }

    if show_progress {
        println!("Moved contents to: {}", cef_dir.display());
    }

    // Cleanup whatever is left in the extracted directory.
    let old_dir = extracted_dir
        .parent()
        .map(|parent| parent.join(format!("extracted_{os}_{arch}")))
        .ok_or_else(|| Error::InvalidArchiveFile(extracted_dir.display().to_string()))?;
    if show_progress {
        println!("Cleaning up: {}", old_dir.display());
    }
    fs_err::rename(&extracted_dir, &old_dir)?;
    fs_err::remove_dir_all(old_dir)?;

    Ok(cef_dir)
}

/// Where every project of the user that builds with tetsu finds its CEF,
/// `tetsu/cef` under the local data directory (`~/.local/share`,
/// `%LOCALAPPDATA%`, `~/Library/Application Support`). `tetsu_sys::cef_install_dir`
/// names the same place at run time.
pub fn cef_install_root() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("tetsu").join("cef"))
}

/// The distribution of `cef_version` for `os_arch` under [`cef_install_root`].
pub fn cef_install_dir(cef_version: &str, os_arch: &OsAndArch) -> Option<PathBuf> {
    cef_install_root().map(|root| root.join(cef_version).join(os_arch.to_string()))
}

/// Installs the distribution of `cef_version` for `target` under
/// [`cef_install_root`]; returns its directory.
///
/// An installation whose `archive.json` names that version and platform is
/// kept. Anything else in its place is replaced. Concurrent installs of one
/// version are safe; the first to finish stays.
pub fn install(
    target: &str,
    cef_version: &str,
    download_url: &str,
    show_progress: bool,
) -> Result<PathBuf> {
    let os_arch = OsAndArch::try_from(target)?;
    let dir = cef_install_dir(cef_version, &os_arch).ok_or(Error::NoDataDir)?;
    let installed = |dir: &Path| check_archive_json(dir, cef_version, target).is_ok();
    if installed(&dir) {
        return Ok(dir);
    }

    let parent = dir
        .parent()
        .expect("an install directory sits in its version's");
    fs_err::create_dir_all(parent)?;
    let staging = parent.join(format!(".installing-{}", std::process::id()));
    if staging.exists() {
        fs_err::remove_dir_all(&staging)?;
    }

    let result = (|| {
        let index = CefIndex::download(download_url)?;
        let version = index.platform(target)?.version(cef_version)?;
        let archive = version.download_archive_with_retry(
            download_url,
            &staging,
            show_progress,
            Duration::from_secs(15),
            3,
        )?;
        let extracted = extract_target_archive(target, &archive, &staging, show_progress)?;
        version.write_archive_json(&extracted)?;

        // Another install of this version may have finished first
        if installed(&dir) {
            return Ok(dir.clone());
        }
        if dir.exists() {
            fs_err::remove_dir_all(&dir)?;
        }

        match fs_err::rename(&extracted, &dir) {
            Ok(()) => Ok(dir.clone()),
            Err(_) if installed(&dir) => Ok(dir.clone()),
            Err(error) => Err(Error::Io(error)),
        }
    })();
    let _ = fs_err::remove_dir_all(&staging);

    result
}

fn calculate_file_sha1(path: &Path) -> Result<String> {
    let mut file = BufReader::new(File::open(path)?);
    let mut sha1 = Sha1::new();
    let mut buffer = [0; 8192];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        sha1.update(&buffer[..count]);
    }

    Ok(sha1.digest().to_string())
}

pub struct OsAndArch {
    pub os: &'static str,
    pub arch: &'static str,
}

impl Display for OsAndArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let os = self.os;
        let arch = self.arch;
        write!(f, "cef_{os}_{arch}")
    }
}

impl TryFrom<&str> for OsAndArch {
    type Error = Error;

    fn try_from(target: &str) -> Result<Self> {
        find_target(target).map(|target| OsAndArch {
            os: target.os,
            arch: target.arch,
        })
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const DEFAULT_TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const DEFAULT_TARGET: &str = "aarch64-unknown-linux-gnu";
#[cfg(all(target_os = "linux", target_arch = "arm"))]
pub const DEFAULT_TARGET: &str = "arm-unknown-linux-gnueabi";

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub const DEFAULT_TARGET: &str = "x86_64-pc-windows-msvc";
#[cfg(all(target_os = "windows", target_arch = "x86"))]
pub const DEFAULT_TARGET: &str = "i686-pc-windows-msvc";
#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
pub const DEFAULT_TARGET: &str = "aarch64-pc-windows-msvc";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const DEFAULT_TARGET: &str = "x86_64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const DEFAULT_TARGET: &str = "aarch64-apple-darwin";

#[cfg(test)]
mod tests {
    use super::*;

    /// A build of `cef_version` on `channel`, as CEF's index lists it.
    fn build(channel: Channel, cef_version: &str) -> CefVersion {
        CefVersion {
            channel,
            cef_version: format!("{cef_version}+gabcdef0+chromium-154.0.8037.94"),
            files: Vec::new(),
        }
    }

    #[test]
    fn the_latest_build_is_the_highest_version_of_its_channel() {
        let platform = CefPlatform {
            versions: vec![
                build(Channel::Stable, "154.0.9"),
                build(Channel::Beta, "155.0.1"),
                build(Channel::Stable, "154.0.33"),
                build(Channel::Stable, "153.1.40"),
            ],
        };

        let stable = platform.latest(&Channel::Stable).unwrap();
        assert!(stable.cef_version.starts_with("154.0.33+"));
        let beta = platform.latest(&Channel::Beta).unwrap();
        assert!(beta.cef_version.starts_with("155.0.1+"));
    }

    #[test]
    fn every_supported_target_has_a_platform_name() {
        for target in supported_targets() {
            assert!(cef_platform_name(target).is_ok(), "{target}");
        }
        assert!(supported_targets().any(|target| target == DEFAULT_TARGET));
    }

    #[test]
    fn channels_parse_by_name() {
        assert!(matches!("stable".parse(), Ok(Channel::Stable)));
        assert!(matches!("beta".parse(), Ok(Channel::Beta)));
        assert!("nightly".parse::<Channel>().is_err());
    }

    /// A fresh directory under the system's temporary directory.
    fn tmp(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tetsu-download-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A distribution whose `archive.json` names CEF `cef_version` for `platform`.
    fn distribution(test: &str, cef_version: &str, platform: &str) -> PathBuf {
        let dir = tmp(test);
        let name = format!(
            "cef_binary_{cef_version}+ga03e714+chromium-154.0.8037.94_{platform}_minimal.tar.bz2"
        );
        std::fs::write(
            dir.join("archive.json"),
            format!(r#"{{"type":"minimal","name":"{name}","sha1":""}}"#),
        )
        .unwrap();
        dir
    }

    #[test]
    fn an_archive_name_parses_with_or_without_its_extension() {
        let name = "cef_binary_154.0.33+ga03e714+chromium-154.0.8037.94_windows64_minimal";
        for name in [name.to_string(), format!("{name}.tar.bz2")] {
            let archive = Archive::parse(&name).unwrap();
            assert_eq!(archive.cef_version, "154.0.33");
            assert_eq!(archive.chromium_version, "154.0.8037.94");
            assert_eq!(archive.platform, "windows64");
            assert_eq!(archive.name, name);
        }

        assert_eq!(Archive::parse("random.tar.bz2"), None);
        assert_eq!(
            Archive::parse("cef_binary_154.0.33_windows64_minimal"),
            None
        );
    }

    #[test]
    fn only_the_exact_version_passes_the_archive_check() {
        let target = "x86_64-unknown-linux-gnu";
        for (test, found, passes) in [
            ("same", "154.0.33", true),
            ("older", "154.0.32", false),
            ("newer", "154.0.34", false),
        ] {
            let dir = distribution(test, found, "linux64");
            let checked = check_archive_json(&dir, "154.0.33", target);
            assert_eq!(checked.is_ok(), passes, "an archive of CEF {found}");
            if !passes {
                assert!(matches!(checked, Err(Error::VersionMismatch { .. })));
            }
        }
    }

    #[test]
    fn another_platform_fails_the_archive_check() {
        let dir = distribution("platform", "154.0.33", "macosarm64");

        let checked = check_archive_json(&dir, "154.0.33", "x86_64-unknown-linux-gnu");

        assert!(matches!(
            checked,
            Err(Error::PlatformMismatch { found, expected, .. })
                if found == "macosarm64" && expected == "linux64"
        ));
    }

    #[test]
    fn a_directory_without_archive_json_fails_the_archive_check() {
        let dir = tmp("missing");

        assert!(read_archive_json(&dir).unwrap().is_none());
        assert!(matches!(
            check_archive_json(&dir, "154.0.33", "x86_64-unknown-linux-gnu"),
            Err(Error::NoArchiveJson { .. })
        ));
    }

    #[test]
    fn every_target_has_a_platform_name_and_os_and_arch() {
        for target in TARGETS {
            assert_eq!(cef_platform_name(target.triple).unwrap(), target.platform);
            assert!(OsAndArch::try_from(target.triple).is_ok());
        }
        assert!(cef_platform_name("wasm32-unknown-unknown").is_err());
    }

    #[test]
    fn the_installation_is_where_tetsu_sys_looks_for_it() {
        let cef_version = format!(
            "{}.{}.{}",
            tetsu_sys::CEF_VERSION_MAJOR,
            tetsu_sys::CEF_VERSION_MINOR,
            tetsu_sys::CEF_VERSION_PATCH
        );
        let os_arch = OsAndArch::try_from(DEFAULT_TARGET).unwrap();

        assert_eq!(
            cef_install_dir(&cef_version, &os_arch),
            tetsu_sys::cef_install_dir()
        );
    }
}
