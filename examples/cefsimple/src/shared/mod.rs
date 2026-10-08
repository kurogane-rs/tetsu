//! Rust port of the [`cefsimple`](https://github.com/chromiumembedded/cef/tree/master/tests/cefsimple) example.

use tetsu::*;

pub mod resources;
pub mod simple_app;
pub mod simple_handler;

#[allow(dead_code)]
pub fn load_cef() {
    let cef = sys::find_cef_dir().expect("CEF not found");
    unsafe { sys::load_libcef(&cef.libcef()) }.expect("cannot load libcef");

    #[cfg(target_os = "macos")]
    crate::mac::setup_simple_application();
}

#[allow(dead_code)]
pub fn run_main(main_args: &MainArgs, cmd_line: &CommandLine, sandbox_info: *mut u8) {
    let switch = CefString::from("type");
    let is_browser_process = cmd_line.has_switch(Some(&switch)) != 1;

    let ret = execute_process(Some(main_args), None, sandbox_info);

    if is_browser_process {
        println!("launch browser process");
        assert_eq!(ret, -1, "cannot execute browser process");
    } else {
        let process_type = CefString::from(&cmd_line.switch_value(Some(&switch)));
        println!("launch process {process_type}");
        assert!(ret >= 0, "cannot execute non-browser process");
        // non-browser process does not initialize cef
        return;
    }

    let mut app = simple_app::SimpleApp::new();

    // Windows runs sandboxed only through CEF's bootstrap, which passes its
    // sandbox information
    let settings = Settings {
        no_sandbox: (cfg!(target_os = "windows") && sandbox_info.is_null()) as _,
        ..Default::default()
    };
    assert_eq!(
        initialize(
            Some(main_args),
            Some(&settings),
            Some(&mut app),
            sandbox_info,
        ),
        1
    );

    #[cfg(target_os = "macos")]
    let _delegate = crate::mac::setup_simple_app_delegate();

    run_message_loop();

    shutdown();
}
