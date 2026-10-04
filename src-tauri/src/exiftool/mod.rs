mod adapter;
mod parser;
mod process;

pub use adapter::ExifToolAdapter;
pub use process::{resolve_exiftool_bin, ExiftoolProcess};
