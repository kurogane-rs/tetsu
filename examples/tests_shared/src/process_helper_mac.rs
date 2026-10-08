use crate::{
    common::{client_app::*, client_app_other::*},
    renderer::client_app_renderer::*,
};
use tetsu::{args::Args, *};

pub fn run_main(
    args: Args,
    custom_schemes: Vec<ClientAppCustomScheme>,
    app_renderer_delegates: Vec<Box<dyn Delegate>>,
) -> Result<(), i32> {
    let _sandbox = {
        let mut sandbox = tetsu::sandbox::Sandbox::new().expect("cannot load the sandbox");
        sandbox
            .initialize(args.as_main_args())
            .expect("cannot enter the sandbox");
        sandbox
    };

    let cef = sys::find_cef_dir().expect("CEF not found");
    unsafe { sys::load_libcef(&cef.libcef()) }.expect("cannot load libcef");

    let app = ClientApp::new(custom_schemes);
    let mut app = match args.as_cmd_line().map(|cmd| ProcessType::from(&cmd)) {
        Some(ProcessType::Renderer) => {
            let app_renderer = ClientAppRenderer::new(app_renderer_delegates);
            ClientAppRendererApp::new(app, app_renderer)
        }
        _ => ClientAppOther::new(app),
    };

    match execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    ) {
        0 => Ok(()),
        err => Err(err),
    }
}
