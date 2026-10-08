use clap::Parser;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0:?}")]
    Io(#[from] io::Error),
    #[error("Metadata error: {0:?}")]
    Metadata(#[from] super::metadata::Error),
    #[error("CEF not found: {0}")]
    Cef(#[from] tetsu_sys::FindError),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Bundles `executable_name` with CEF. With `bootstrap`, the package's library
/// goes beside CEF's sandbox bootstrap, under the executable's name.
///
/// See https://bitbucket.org/chromiumembedded/cef/wiki/GeneralUsage.md#markdown-header-windows
pub fn bundle(
    app_path: &Path,
    target_path: &Path,
    executable_name: &str,
    bootstrap: bool,
) -> Result<PathBuf> {
    let cef_path = tetsu_sys::find_cef_dir()?.path;
    copy_directory(&cef_path, app_path)?;

    const LOCALES_DIR: &str = "locales";
    copy_directory(&cef_path.join(LOCALES_DIR), &app_path.join(LOCALES_DIR))?;

    copy_app(app_path, target_path, executable_name, bootstrap)
}

/// Similar to [`bundle`], but this will invoke `cargo build` to build the executable target.
///
/// A package that builds a cdylib is bundled for CEF's sandbox bootstrap.
pub fn build_bundle(
    app_path: &Path,
    executable_name: &str,
    release: bool,
    features: Option<&str>,
) -> Result<PathBuf> {
    let cargo_metadata = super::metadata::get_cargo_metadata()?;
    let target_path =
        cargo_metadata
            .target_directory()
            .join(if release { "release" } else { "debug" });

    let bootstrap = cargo_metadata.builds_cdylib(executable_name);

    println!("Building {executable_name}...");
    let mut args = if bootstrap {
        vec!["-p", executable_name, "--lib"]
    } else {
        vec!["--bin", executable_name]
    };
    if release {
        args.push("--release");
    }
    super::cargo_build(&args, features)?;

    bundle(app_path, &target_path, executable_name, bootstrap)
}

fn copy_app(
    app_path: &Path,
    target_path: &Path,
    executable_name: &str,
    bootstrap: bool,
) -> Result<PathBuf> {
    let mut manifest_file =
        fs::File::create(app_path.join(format!("{executable_name}.exe.manifest")))?;
    manifest_file.write_all(tetsu_build::WINDOWS_MANIFEST.as_bytes())?;

    let executable_path = app_path.join(format!("{executable_name}.exe"));
    if bootstrap {
        let dll_name = format!("{executable_name}.dll");
        fs::copy(target_path.join(&dll_name), app_path.join(&dll_name))?;

        // A release build without debug information has no PDB
        let pdb_name = format!("{executable_name}.pdb");
        let target_pdb = target_path.join(&pdb_name);
        if target_pdb.exists() {
            fs::copy(&target_pdb, app_path.join(&pdb_name))?;
        }

        let cef_path = tetsu_sys::find_cef_dir()?.path;
        fs::copy(cef_path.join("bootstrap.exe"), &executable_path)?;
    } else {
        fs::copy(
            target_path.join(format!("{executable_name}.exe")),
            &executable_path,
        )?;
    }

    Ok(executable_path)
}

fn copy_directory(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_file()
            && !entry
                .path()
                .extension()
                .map(|ext| ext == "exe")
                .unwrap_or_default()
        {
            fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Args {
    name: String,
    #[arg(long, default_value_t = false)]
    release: bool,
    #[arg(short, long)]
    output: Option<String>,
    /// Features of the example's package to enable, comma separated.
    #[arg(short = 'F', long)]
    features: Option<String>,
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let output = match args.output {
        Some(output) => PathBuf::from(output),
        None => env::current_dir()?,
    };

    let bundle_path = build_bundle(
        output.as_path(),
        &args.name,
        args.release,
        args.features.as_deref(),
    )?;
    let bundle_path = bundle_path.display();
    println!("Run the app from {bundle_path}");
    Ok(())
}
