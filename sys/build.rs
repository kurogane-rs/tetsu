#[cfg(not(feature = "dox"))]
fn main() -> anyhow::Result<()> {
    use fs_err as fs;
    use std::{
        env,
        path::{Path, PathBuf},
    };
    use tetsu_download::OsAndArch;

    println!("cargo::rerun-if-changed=build.rs");

    let target = env::var("TARGET")?;
    let os_arch = OsAndArch::try_from(target.as_str())?;

    // The runtime goes next to the binaries only when asked for; an application
    // that names CEF's paths itself (resources_dir_path, locales_dir_path) needs
    // no copy of up to 1.5 GB per profile
    println!("cargo::rerun-if-env-changed=TETSU_STAGE_RUNTIME");
    let stage_runtime = env::var("TETSU_STAGE_RUNTIME").is_ok_and(|value| value == "1");

    // tetsu's macOS application code uses AppKit
    if os_arch.os == "macos" {
        println!("cargo::rustc-link-lib=framework=AppKit");
    }

    // Nothing links libcef; the application loads it when it starts
    // (tetsu_sys::load_libcef), so a build needs CEF only for the runtime copy,
    // which macOS leaves to the application's bundle
    if os_arch.os == "macos" || !stage_runtime {
        return Ok(());
    }

    println!("cargo::rerun-if-env-changed=FLATPAK");
    println!("cargo::rerun-if-env-changed=NIX_CEF_BINARY");
    println!("cargo::rerun-if-env-changed=CEF_PATH");
    let package_version = env::var("CARGO_PKG_VERSION")?;
    let cef_version = tetsu_download::default_version(&package_version);

    let check_archive = |path: &Path| -> anyhow::Result<()> {
        tetsu_download::check_archive_json(&package_version, &path.to_string_lossy())?;
        Ok(())
    };

    let resolve_cef_dir = |location: &Path| -> anyhow::Result<PathBuf> {
        let cef_dir = location.join(os_arch.to_string());

        if !fs::exists(&cef_dir)? {
            if env::var("NIX_CEF_BINARY").is_ok() {
                tetsu_download::install_nix_cef(&cef_version, &cef_dir, false)?;
            } else {
                use tetsu_download::CefIndex;

                let download_url = tetsu_download::default_download_url();
                let index = CefIndex::download_from(&download_url)?;
                let platform = index.platform(&target)?;
                let version = platform.version(&cef_version)?;

                let archive = version.download_archive_from(&download_url, location, false)?;
                let extracted_dir =
                    tetsu_download::extract_target_archive(&target, &archive, location, false)?;
                let extracted_dir_canonical = fs::canonicalize(&extracted_dir)?;
                let cef_dir_canonical = fs::canonicalize(&cef_dir)?;
                if extracted_dir_canonical != cef_dir_canonical {
                    return Err(anyhow::anyhow!(
                        "extracted dir {extracted_dir_canonical:?} does not match cef_dir {cef_dir_canonical:?}",
                    ));
                }

                version.write_archive_json(extracted_dir)?;
            }
        }

        Ok(cef_dir)
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let cef_dir = if env::var("FLATPAK").is_ok() {
        let cef_path = String::from("/usr/lib");
        println!("Using CEF path from FLATPAK: {cef_path}");
        let cef_path = PathBuf::from(cef_path);
        check_archive(&cef_path)?;
        cef_path
    } else if let Ok(cef_path) = env::var("CEF_PATH") {
        // A set CEF_PATH names the distribution to build against; a wrong one is an
        // error, never a download into it
        let configured_path = PathBuf::from(cef_path);
        let fix = format!(
            "point CEF_PATH at a CEF {cef_version} distribution \
             (`cargo run -p export-cef-dir -- <dir>` writes one), \
             or unset it to download one into the build directory"
        );
        if !fs::exists(&configured_path)? {
            return Err(anyhow::anyhow!(
                "CEF_PATH ({}) does not exist; {fix}",
                configured_path.display()
            ));
        }
        // The layout an earlier download into CEF_PATH left
        let versioned = configured_path.join(&cef_version).join(os_arch.to_string());
        let cef_dir = if fs::exists(&versioned)? {
            versioned
        } else {
            configured_path.clone()
        };
        check_archive(&cef_dir).map_err(|error| {
            anyhow::anyhow!(
                "CEF_PATH ({}) is not a CEF {cef_version} distribution for {os_arch}: {error}; {fix}",
                cef_dir.display()
            )
        })?;
        println!("Using CEF path from environment: {}", cef_dir.display());
        cef_dir
    } else if env::var("NIX_CEF_BINARY").is_ok() {
        resolve_cef_dir(&out_dir)?
    } else {
        // The user's shared installation, which every project and profile
        // builds and runs with; OUT_DIR only for a user without a data directory
        match tetsu_download::install(
            &target,
            &cef_version,
            &tetsu_download::default_download_url(),
            false,
        ) {
            Err(tetsu_download::Error::NoDataDir) => resolve_cef_dir(&out_dir)?,
            installed => installed?,
        }
    };

    // TODO: far from ideal, but there's no other way to get the target dir, see <https://github.com/rust-lang/cargo/issues/9661>
    let target_dir = out_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let cef_dir_str = cef_dir.to_string_lossy().into_owned();

    // Re-run when the resolved CEF directory changes/deletes.
    println!("cargo::rerun-if-changed={cef_dir_str}");

    println!("cargo::metadata=CEF_DIR={cef_dir_str}");

    // A binary run from target/ finds the runtime next to it
    copy_cef_runtime_files(&cef_dir, target_dir)?;

    Ok(())
}

#[cfg(not(feature = "dox"))]
fn copy_directory(src: &std::path::Path, dest: &std::path::Path) -> Result<(), std::io::Error> {
    fs_err::create_dir_all(dest)?;
    for entry in fs_err::read_dir(src)? {
        let entry = entry?;
        if entry.path().is_file() {
            let dest = dest.join(entry.file_name());
            if dest.is_file() {
                fs_err::remove_file(&dest)?;
            }
            fs_err::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(not(feature = "dox"))]
fn copy_cef_runtime_files(
    cef_dir: &std::path::Path,
    target_dir: &std::path::Path,
) -> anyhow::Result<()> {
    const LOCALES_DIR: &str = "locales";
    let locales = cef_dir.join(LOCALES_DIR);

    // Checked before anything is copied; an unpacked official archive keeps the
    // runtime under Release/ and Resources/
    anyhow::ensure!(
        locales.is_dir(),
        "{} has no {LOCALES_DIR} directory; CEF_PATH must point at a distribution laid out \
         as `export-cef-dir` writes it, not an unpacked official archive, which keeps these \
         files under Release/ and Resources/",
        cef_dir.display(),
    );

    copy_directory(cef_dir, target_dir)?;
    copy_directory(&locales, &target_dir.join(LOCALES_DIR))?;

    Ok(())
}

#[cfg(feature = "dox")]
fn main() {}
