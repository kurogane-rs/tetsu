#![doc = include_str!("../README.md")]

use clap::Parser;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::Duration,
};
use tetsu_download::{CefFile, CefIndex, OsAndArch, DEFAULT_TARGET};

fn default_version() -> &'static str {
    static DEFAULT_VERSION: OnceLock<String> = OnceLock::new();
    DEFAULT_VERSION
        .get_or_init(|| tetsu_download::default_version(env!("CARGO_PKG_VERSION")))
        .as_str()
}

fn default_download_url() -> &'static str {
    static DEFAULT_DOWNLOAD_URL: OnceLock<String> = OnceLock::new();
    DEFAULT_DOWNLOAD_URL
        .get_or_init(tetsu_download::default_download_url)
        .as_str()
}

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    #[arg(short, long)]
    force: bool,
    #[arg(short, long)]
    save_archive: bool,
    /// Take the distribution from the `cef-binary` package of `nixpkgs`
    #[arg(short, long)]
    nix: bool,
    #[arg(short, long, default_value = DEFAULT_TARGET)]
    target: String,
    #[arg(short, long, default_value = default_version())]
    version: String,
    #[arg(short, long, default_value = default_download_url())]
    mirror_url: String,
    #[arg(short, long)]
    archive: Option<String>,
    /// The directory to export to; without one, the distribution goes into
    /// tetsu's shared installation
    output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let Some(output) = args.output.as_deref().map(PathBuf::from) else {
        return install(&args);
    };
    let url = args.mirror_url.as_str();

    let parent = PathBuf::from(
        output
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid target directory: {}", output.display()))?,
    );

    if fs::exists(&output)? {
        if !args.force {
            return Err(anyhow::anyhow!(
                "target directory already exists: {}",
                output.display()
            ));
        }

        let dir = output
            .file_name()
            .and_then(|dir| dir.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid target directory: {}", output.display()))?;
        let old_output = parent.join(format!("old_{dir}"));
        fs::rename(&output, &old_output)?;
        println!("Cleaning up: {}", old_output.display());
        fs::remove_dir_all(old_output)?
    }

    let target = args.target.as_str();
    let os_arch = OsAndArch::try_from(target)?;
    let cef_dir = os_arch.to_string();
    let cef_dir = parent.join(&cef_dir);

    if fs::exists(&cef_dir)? {
        let dir = cef_dir
            .file_name()
            .and_then(|dir| dir.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid target directory: {}", output.display()))?;
        let old_cef_dir = parent.join(format!("old_{dir}"));
        fs::rename(&cef_dir, &old_cef_dir)?;
        println!("Cleaning up: {}", old_cef_dir.display());
        fs::remove_dir_all(old_cef_dir)?
    }

    let (archive, extracted_dir) = match args.archive {
        Some(archive) => {
            let extracted_dir =
                tetsu_download::extract_target_archive(target, &archive, &parent, true)?;
            let archive = CefFile::try_from(Path::new(&archive))?;
            (archive, extracted_dir)
        }
        None => {
            let cef_version = args.version.as_str();

            if args.nix {
                return install_nix_cef(cef_version, &output);
            } else {
                let index = CefIndex::download(url)?;
                let platform = index.platform(target)?;
                let version = platform.version(cef_version)?;

                let archive = version.download_archive_with_retry(
                    url,
                    &parent,
                    true,
                    Duration::from_secs(15),
                    3,
                )?;
                let extracted_dir =
                    tetsu_download::extract_target_archive(target, &archive, &parent, true)?;

                if !args.save_archive {
                    println!("Cleaning up: {}", archive.display());
                    fs::remove_file(archive)?;
                }

                let archive = version.minimal()?.clone();
                (archive, extracted_dir)
            }
        }
    };

    if extracted_dir != cef_dir {
        return Err(anyhow::anyhow!(
            "extracted dir {extracted_dir:?} does not match cef_dir {cef_dir:?}",
        ));
    }

    archive.write_archive_json(extracted_dir)?;

    if output != cef_dir {
        println!("Renaming: {}", output.display());
        fs::rename(cef_dir, output)?;
    }

    Ok(())
}

/// Installs the distribution into tetsu's shared installation, where an
/// application started without `CEF_PATH` finds it (`tetsu_sys::find_cef_dir`).
fn install(args: &Args) -> anyhow::Result<()> {
    anyhow::ensure!(
        !args.nix && args.archive.is_none(),
        "--nix and --archive export to an output directory"
    );

    let os_arch = OsAndArch::try_from(args.target.as_str())?;
    let dir = tetsu_download::cef_install_dir(&args.version, &os_arch)
        .ok_or(tetsu_download::Error::NoDataDir)?;
    if args.force && fs::exists(&dir)? {
        println!("Cleaning up: {}", dir.display());
        fs::remove_dir_all(&dir)?;
    }

    let dir = tetsu_download::install(&args.target, &args.version, &args.mirror_url, true)?;
    println!("Installed: {}", dir.display());

    Ok(())
}

/// Installs CEF from `nixpkgs` into `location`.
fn install_nix_cef(cef_version: &str, location: &Path) -> anyhow::Result<()> {
    let nix_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("nix");
    let status = Command::new("nix-build")
        .arg(&nix_dir)
        .args(["--arg", "version", &format!(r#""{cef_version}""#)])
        .arg("--out-link")
        .arg(location)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()?;

    anyhow::ensure!(status.success(), "nix-build failed: {status}");
    Ok(())
}
