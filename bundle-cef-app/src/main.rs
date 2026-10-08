//! Bundles tetsu's examples into applications that run.
//!
//! A development tool of this repository. Applications ship with their own
//! bundler, such as Kurogane's.

mod metadata;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "windows")]
mod win;

use std::{env, io, process::Command};

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    linux::main()
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    mac::main()
}

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
}

/// Runs `cargo build` with `args`, enabling the comma-separated `features`.
///
/// Cargo is the one in `CARGO` when it is set, else the one on `PATH`.
fn cargo_build(args: &[&str], features: Option<&str>) -> io::Result<()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command.arg("build").args(args);
    if let Some(features) = features {
        command.args(["--features", features]);
    }

    if command.status()?.success() {
        Ok(())
    } else {
        Err(io::Error::other("cargo build failed"))
    }
}
