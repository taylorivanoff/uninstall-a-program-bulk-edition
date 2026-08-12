fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    // Release builds require admin (machine-wide uninstalls). Debug uses asInvoker
    // so `tauri dev` and cargo tests do not force a UAC prompt every launch.
    let manifest = if std::env::var("PROFILE").as_deref() == Ok("release") {
        include_str!("windows/app.manifest")
    } else {
        include_str!("windows/app.debug.manifest")
    };
    windows = windows.app_manifest(manifest);
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri build script");
}
