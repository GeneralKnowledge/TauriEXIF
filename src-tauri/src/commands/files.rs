use crate::domain::file_types::is_supported;
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyResult {
    pub files: Vec<ClassifiedFile>,
    pub folders: Vec<String>,
    pub unsupported: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandResult {
    pub files: Vec<ClassifiedFile>,
    pub skipped_count: usize,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn classify_paths(paths: Vec<String>) -> Result<ClassifyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut files = Vec::new();
        let mut folders = Vec::new();
        let mut unsupported = Vec::new();

        for path in paths {
            let p = PathBuf::from(&path);
            let meta = match std::fs::metadata(&p) {
                Ok(m) => m,
                Err(_) => {
                    unsupported.push(path);
                    continue;
                }
            };
            if meta.is_dir() {
                folders.push(path);
            } else if meta.is_file() {
                if is_supported(&path) {
                    files.push(ClassifiedFile {
                        path,
                        size: meta.len(),
                    });
                } else {
                    unsupported.push(path);
                }
            } else {
                unsupported.push(path);
            }
        }

        ClassifyResult {
            files,
            folders,
            unsupported,
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn expand_folder(dir_path: String) -> Result<ExpandResult, String> {
    tauri::async_runtime::spawn_blocking(move || expand_folder_sync(&dir_path))
        .await
        .map_err(|e| e.to_string())
}

fn expand_folder_sync(dir_path: &str) -> ExpandResult {
    let root = Path::new(dir_path);
    if !root.is_dir() {
        return ExpandResult {
            files: vec![],
            skipped_count: 0,
            error: Some("Not a directory".into()),
        };
    }

    let mut files = Vec::new();
    let mut skipped_count = 0usize;

    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let path_str = path.to_string_lossy().into_owned();
        if !is_supported(&path_str) {
            skipped_count += 1;
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        files.push(ClassifiedFile {
            path: path_str,
            size,
        });
    }

    ExpandResult {
        files,
        skipped_count,
        error: None,
    }
}
