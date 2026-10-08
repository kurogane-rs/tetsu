#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

pub mod shared;

#[cfg(target_os = "macos")]
mod mac;

// Windows runs sandboxed only through CEF's bootstrap, which loads the
// library instead
fn main() -> Result<(), &'static str> {
    shared::load_cef();

    let args = tetsu::args::Args::new();
    let Some(cmd_line) = args.as_cmd_line() else {
        return Err("Failed to parse command line arguments");
    };

    shared::run_main(args.as_main_args(), &cmd_line, std::ptr::null_mut());
    Ok(())
}
