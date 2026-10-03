use crate::domain::settings::{save_settings, Settings};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let settings = state.settings.lock().await.clone();
    Ok(settings)
}

#[tauri::command]
pub async fn set_settings(
    state: State<'_, AppState>,
    patch: Settings,
) -> Result<Settings, String> {
    save_settings(&patch)?;
    let mut guard = state.settings.lock().await;
    *guard = patch.clone();
    Ok(patch)
}
