#[cfg(target_os = "windows")]
include!("src/shared/resources.rs");

fn main() {
    tetsu_build::embed_windows_manifest();

    // The icons alone: the manifest is the linker's, and a second one fails the link
    #[cfg(target_os = "windows")]
    winres::WindowsResource::new()
        .set_icon_with_id("resources/win/cefsimple.ico", &IDI_CEFSIMPLE.to_string())
        .set_icon_with_id("resources/win/small.ico", &IDI_SMALL.to_string())
        .compile()
        .unwrap();
}
