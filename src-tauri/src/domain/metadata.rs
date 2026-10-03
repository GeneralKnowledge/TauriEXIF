use serde_json::{Map, Value};

const COMPUTED_FIELDS: &[&str] = &["SourceFile", "ImageSize", "Megapixels"];
const STRUCTURAL_GROUPS: &[&str] = &["System", "JFIF", "ExifTool", "Composite"];

/// Removable File-group tags (writable and not structural override).
const REMOVABLE_FILE_TAGS: &[&str] = &["Comment"];

fn is_copy_instance(part: &str) -> bool {
    part.starts_with("Copy") && part[4..].chars().all(|c| c.is_ascii_digit())
}

fn normalize_key(key: &str) -> String {
    let parts: Vec<&str> = key.split(':').filter(|p| !is_copy_instance(p)).collect();
    if parts.len() >= 3 {
        parts[1..].join(":")
    } else {
        parts.join(":")
    }
}

fn is_removable_file_tag(key: &str) -> bool {
    let parts: Vec<&str> = key.split(':').filter(|p| !is_copy_instance(p)).collect();
    parts
        .last()
        .map(|tag| REMOVABLE_FILE_TAGS.contains(tag))
        .unwrap_or(false)
}

fn is_computed_field(key: &str) -> bool {
    if COMPUTED_FIELDS.contains(&key) {
        return true;
    }
    let parts: Vec<&str> = key.split(':').collect();
    if let Some(group1) = parts.first() {
        if STRUCTURAL_GROUPS.contains(group1) {
            return true;
        }
        if *group1 == "File" && !is_removable_file_tag(key) {
            return true;
        }
    }
    parts
        .last()
        .map(|tag| COMPUTED_FIELDS.contains(tag))
        .unwrap_or(false)
}

/// Strip ExifTool structural/computed fields so empty JPEGs look empty.
pub fn clean_exif_data(raw: &Map<String, Value>) -> Map<String, Value> {
    let mut cleaned = Map::new();
    for (key, value) in raw {
        if !is_computed_field(key) {
            cleaned.insert(normalize_key(key), value.clone());
        }
    }
    cleaned
}
