use crate::domain::cleaned_path::generate_cleaned_path;
use crate::domain::file_types::{is_raf, is_raw};
use crate::domain::outcome::{classify_outcome, summarize_metadata_change, OutcomeKind};
use crate::AppState;
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveMetadataResult {
    pub success: bool,
    pub output_path: Option<String>,
    pub output_size: Option<u64>,
    pub wrote_file: bool,
    pub was_forced_copy: bool,
    pub outcome_kind: OutcomeKind,
    pub before_metadata: Map<String, Value>,
    pub after_metadata: Map<String, Value>,
    pub removed_count: usize,
    pub before_count: usize,
    pub after_count: usize,
    pub error: Option<String>,
    pub refusal_reason: Option<String>,
}

#[tauri::command]
pub async fn read_metadata(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<Map<String, Value>, String> {
    state.exiftool.read_display_metadata(&file_path).await
}

#[tauri::command]
pub async fn remove_metadata(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<RemoveMetadataResult, String> {
    if is_raf(&file_path) {
        return Ok(RemoveMetadataResult {
            success: false,
            output_path: None,
            output_size: None,
            wrote_file: false,
            was_forced_copy: false,
            outcome_kind: OutcomeKind::Refused,
            before_metadata: Map::new(),
            after_metadata: Map::new(),
            removed_count: 0,
            before_count: 0,
            after_count: 0,
            error: Some(
                "RAF files are refused — ExifCleaner cannot currently guarantee a safe cleaned RAF artifact."
                    .into(),
            ),
            refusal_reason: Some("unsafe-raf-write".into()),
        });
    }

    let settings = state.settings.lock().await.clone();

    let before = match state.exiftool.read_display_metadata(&file_path).await {
        Ok(m) => m,
        Err(e) => {
            return Ok(failed_result(e));
        }
    };

    if before.is_empty() {
        let size = tokio::fs::metadata(&file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        return Ok(RemoveMetadataResult {
            success: true,
            output_path: Some(file_path),
            output_size: Some(size),
            wrote_file: false,
            was_forced_copy: false,
            outcome_kind: OutcomeKind::AlreadyClean,
            before_metadata: before.clone(),
            after_metadata: before,
            removed_count: 0,
            before_count: 0,
            after_count: 0,
            error: None,
            refusal_reason: None,
        });
    }

    let was_forced_copy = is_raw(&file_path) && !settings.save_as_copy;
    let save_as_copy = settings.save_as_copy || was_forced_copy;
    let output_path = if save_as_copy {
        Some(generate_cleaned_path(&PathBuf::from(&file_path)))
    } else {
        None
    };

    if let Err(e) = state
        .exiftool
        .strip_metadata(&file_path, output_path.as_deref(), &settings)
        .await
    {
        return Ok(failed_result(e));
    }

    let actual_output = output_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.clone());

    let after = match state.exiftool.read_display_metadata(&actual_output).await {
        Ok(m) => m,
        Err(e) => {
            return Ok(failed_result(format!(
                "Wrote output but failed to re-read metadata: {e}"
            )));
        }
    };

    let summary = summarize_metadata_change(&before, &after);
    let outcome = classify_outcome(&summary, true);
    let output_size = tokio::fs::metadata(&actual_output)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(RemoveMetadataResult {
        success: true,
        output_path: Some(actual_output),
        output_size: Some(output_size),
        wrote_file: true,
        was_forced_copy,
        outcome_kind: outcome,
        before_metadata: before,
        after_metadata: after,
        removed_count: summary.removed_count,
        before_count: summary.before_count,
        after_count: summary.after_count,
        error: None,
        refusal_reason: None,
    })
}

fn failed_result(error: String) -> RemoveMetadataResult {
    RemoveMetadataResult {
        success: false,
        output_path: None,
        output_size: None,
        wrote_file: false,
        was_forced_copy: false,
        outcome_kind: OutcomeKind::Failed,
        before_metadata: Map::new(),
        after_metadata: Map::new(),
        removed_count: 0,
        before_count: 0,
        after_count: 0,
        error: Some(error),
        refusal_reason: None,
    }
}
