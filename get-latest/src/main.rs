#![doc = include_str!("../README.md")]

use clap::Parser;
use semver::{BuildMetadata, Version};
use std::{env, fs, io::Write, path::PathBuf};
use tetsu_download::{CefIndex, Channel};
use toml_edit::{value, DocumentMut};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Download error: {0}")]
    Download(#[from] tetsu_download::Error),
    #[error("Invalid version: {0}")]
    InvalidVersion(#[from] semver::Error),
    #[error("No version is published for every target")]
    NoVersionsFound,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid manifest file: {0}")]
    InvalidManifest(#[from] toml_edit::TomlError),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser)]
#[command(about, long_about = None)]
struct Args {
    /// Mirror of CEF's builds.
    #[arg(short, long, default_value_t = tetsu_download::default_download_url())]
    mirror_url: String,
    /// Release channel: stable or beta.
    #[arg(short, long, default_value = "stable")]
    channel: Channel,
    /// Move the workspace to the newest version when it is newer.
    #[arg(short, long)]
    update_version: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let channel = args.channel;

    let index = CefIndex::download(&args.mirror_url)?;
    let latest_version = tetsu_download::supported_targets()
        .map(|target| {
            let build = index.platform(target)?.latest(&channel)?;
            // `154.0.33+ga03e714+chromium-154.0.8037.94` is CEF 154.0.33
            let cef_version = build
                .cef_version
                .split_once('+')
                .map_or(build.cef_version.as_str(), |(version, _)| version);
            Ok(Version::parse(cef_version)?)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .min()
        .ok_or(Error::NoVersionsFound)?;

    println!("Latest {channel} version on every target: {latest_version}");

    if !args.update_version {
        return Ok(());
    }

    let current_version =
        Version::parse(&tetsu_download::default_version(env!("CARGO_PKG_VERSION")))?;
    if current_version >= latest_version {
        println!("The workspace is on {current_version} already");
        return Ok(());
    }

    let mut next_version = Version::parse(env!("CARGO_PKG_VERSION"))?;
    if next_version.major < latest_version.major {
        next_version.major = latest_version.major;
        next_version.minor = 0;
    } else {
        next_version.minor += 1;
    }
    next_version.patch = 0;
    next_version.build = BuildMetadata::new(&latest_version.to_string())?;
    let sys_version = Version {
        build: BuildMetadata::EMPTY,
        ..next_version.clone()
    };

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("get-latest sits in the workspace")
        .join("Cargo.toml");
    let mut doc = fs::read_to_string(&manifest)?.parse::<DocumentMut>()?;
    doc["workspace"]["package"]["version"] = value(next_version.to_string());
    doc["workspace"]["dependencies"]["tetsu-sys"]["version"] = value(sys_version.to_string());
    fs::write(&manifest, doc.to_string())?;

    println!("Moved the workspace to {next_version}");

    // The workflow commits the change and opens a pull request for review
    if let Ok(output) = env::var("GITHUB_OUTPUT") {
        let mut output = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(output)?;
        writeln!(output, "updated=true")?;
        writeln!(output, "version={next_version}")?;
        writeln!(
            output,
            "commit-message=feat!: update CEF to {latest_version}"
        )?;
    }

    Ok(())
}
