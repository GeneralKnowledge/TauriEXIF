use std::path::{Path, PathBuf};

/// Generate a collision-free `*_cleaned.ext` path next to the source file.
pub fn generate_cleaned_path(file_path: &Path) -> PathBuf {
    let parent = file_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();

    let mut candidate = parent.join(format!("{stem}_cleaned{ext}"));
    let mut counter = 2u32;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}_cleaned_{counter}{ext}"));
        counter += 1;
    }
    candidate
}
