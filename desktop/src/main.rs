// Release builds are GUI apps on Windows: no console window behind the webview.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod boot;
mod chrome;
mod shell_path;
mod sidecar;
mod splash;

use sidecar::Sidecar;
use tauri::{Manager, RunEvent};

fn main() -> Result<(), tauri::Error> {
    let app = tauri::Builder::default()
        .manage(Sidecar::default())
        .setup(|app| {
            chrome::build_window(app.handle(), boot::MAIN_WINDOW)?;
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("opman-boot".into())
                .spawn(move || boot::run(&handle))?;
            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(|app, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            app.state::<Sidecar>().shutdown();
        }
    });
    Ok(())
}
