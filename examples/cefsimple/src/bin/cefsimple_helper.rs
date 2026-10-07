use tetsu::{args::Args, *};

fn main() {
    let args = Args::new();

    #[cfg(all(target_os = "macos", feature = "sandbox"))]
    let _sandbox = {
        let mut sandbox = tetsu::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    };

    let cef = sys::find_cef_dir().expect("CEF not found");
    unsafe { sys::load_libcef(&cef.libcef()) }.expect("cannot load libcef");

    execute_process(
        Some(args.as_main_args()),
        None::<&mut App>,
        std::ptr::null_mut(),
    );
}
