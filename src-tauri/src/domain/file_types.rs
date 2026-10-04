const SUPPORTED: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "tiff", "tif", "webp", "heic", "heif", "bmp", "avif",
    "svg", "cr2", "cr3", "nef", "arw", "orf", "rw2", "raf", "dng", "pef", "srw", "mp4",
    "mov", "avi", "m4a", "m4v", "3gp", "wmv", "pdf",
];

const RAW: &[&str] = &[
    "raf", "cr2", "cr3", "nef", "arw", "orf", "rw2", "dng", "pef", "srw",
];

const MEDIA: &[&str] = &["mp4", "mov", "avi", "m4a", "m4v", "3gp", "wmv"];
const TIFF: &[&str] = &["tif", "tiff"];

pub fn extension_of(filename: &str) -> Option<String> {
    let name = std::path::Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let dot = name.rfind('.')?;
    if dot == 0 {
        return None;
    }
    Some(name[dot + 1..].to_ascii_lowercase())
}

pub fn is_supported(filename: &str) -> bool {
    extension_of(filename)
        .map(|ext| SUPPORTED.contains(&ext.as_str()))
        .unwrap_or(false)
}

pub fn is_raw(filename: &str) -> bool {
    extension_of(filename)
        .map(|ext| RAW.contains(&ext.as_str()))
        .unwrap_or(false)
}

pub fn is_raf(filename: &str) -> bool {
    extension_of(filename).as_deref() == Some("raf")
}

pub fn is_media(filename: &str) -> bool {
    extension_of(filename)
        .map(|ext| MEDIA.contains(&ext.as_str()))
        .unwrap_or(false)
}

pub fn is_tiff(filename: &str) -> bool {
    extension_of(filename)
        .map(|ext| TIFF.contains(&ext.as_str()))
        .unwrap_or(false)
}
