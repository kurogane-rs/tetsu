#[cfg(target_os = "macos")]
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cargo metadata error: {0:?}")]
    Metadata(#[from] cargo_metadata::Error),
    #[cfg(target_os = "macos")]
    #[error("Missing package metadata for {0}")]
    MissingPackageMetadata(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
struct PackageMetadata {
    cef: Cef,
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
struct Cef {
    bundle: CargoBundleMetadata,
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
struct CargoBundleMetadata {
    helper_name: String,
    resources_path: Option<String>,
}

#[cfg(target_os = "macos")]
pub struct BundleMetadata {
    pub helper_name: String,
    pub resources_path: Option<PathBuf>,
}

#[cfg(target_os = "macos")]
impl BundleMetadata {
    pub fn parse(executable: &str, metadata: &cargo_metadata::Metadata) -> Option<Self> {
        let package = metadata
            .packages
            .iter()
            .find(|p| p.targets.iter().any(|t| t.name == executable))?;
        let package_metadata =
            serde_json::from_value::<PackageMetadata>(package.metadata.clone()).ok()?;
        let resources_path = package_metadata
            .cef
            .bundle
            .resources_path
            .as_deref()
            .and_then(|resources_path| {
                package
                    .manifest_path
                    .clone()
                    .into_std_path_buf()
                    .parent()
                    .map(|manifest_dir| manifest_dir.join(resources_path))
            });
        Some(Self {
            helper_name: package_metadata.cef.bundle.helper_name,
            resources_path,
        })
    }
}

pub struct CargoMetadata(cargo_metadata::Metadata);

impl CargoMetadata {
    pub fn target_directory(&self) -> PathBuf {
        PathBuf::from(&self.0.target_directory)
    }

    /// Returns whether the package of `executable` builds a cdylib, the library
    /// CEF's sandbox bootstrap loads.
    #[cfg(target_os = "windows")]
    pub fn builds_cdylib(&self, executable: &str) -> bool {
        self.0
            .packages
            .iter()
            .find(|package| package.targets.iter().any(|t| t.name == executable))
            .is_some_and(|package| {
                package.targets.iter().any(|target| {
                    target
                        .crate_types
                        .contains(&cargo_metadata::CrateType::CDyLib)
                })
            })
    }

    #[cfg(target_os = "macos")]
    pub fn parse_bundle_metadata(&self, executable: &str) -> Result<BundleMetadata> {
        BundleMetadata::parse(executable, &self.0)
            .ok_or_else(|| Error::MissingPackageMetadata(executable.to_owned()))
    }
}

/// Run `cargo metadata` to determine the configuration for the current workspace/package.
pub fn get_cargo_metadata() -> Result<CargoMetadata> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .other_options(vec!["--frozen".to_string()])
        .exec()?;
    Ok(CargoMetadata(metadata))
}
