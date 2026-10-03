mod commands;
mod domain;
mod exiftool;

use domain::settings::{load_settings, Settings};
use exiftool::{resolve_exiftool_bin, ExifToolAdapter, ExiftoolProcess};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

/// Shared helpers for integration tests.
#[doc(hidden)]
pub mod test_support {
    pub use crate::domain::settings::Settings;
    pub use crate::exiftool::{resolve_exiftool_bin, ExifToolAdapter, ExiftoolProcess};

    pub fn default_settings() -> Settings {
        Settings::default()
    }
}

pub struct AppState {
    pub exiftool: Arc<ExifToolAdapter>,
    pub settings: Mutex<Settings>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let resource_dir = app.path().resource_dir().ok();
            let bin = resolve_exiftool_bin(resource_dir.as_deref());
            eprintln!("Using ExifTool at {}", bin.display());

            let process = ExiftoolProcess::new(bin);
            let adapter = Arc::new(ExifToolAdapter::new(process));
            let adapter_for_open = adapter.clone();

            tauri::async_runtime::block_on(async move {
                adapter_for_open
                    .open()
                    .await
                    .map_err(|e| e.to_string())
            })?;

            app.manage(AppState {
                exiftool: adapter,
                settings: Mutex::new(load_settings()),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::exif::read_metadata,
            commands::exif::remove_metadata,
            commands::files::classify_paths,
            commands::files::expand_folder,
            commands::settings::get_settings,
            commands::settings::set_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    let exiftool = state.exiftool.clone();
                    let _ = tauri::async_runtime::block_on(async move {
                        let _ = exiftool.close().await;
                    });
                }
            }
        });
}
