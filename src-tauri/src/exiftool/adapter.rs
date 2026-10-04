use super::process::{write_deadline_ms, ExifToolError, ExiftoolProcess};
use crate::domain::file_types::{is_media, is_raw, is_tiff};
use crate::domain::metadata::clean_exif_data;
use crate::domain::settings::Settings;
use serde_json::{Map, Value};
use std::path::Path;
use tokio::fs;

const QUICKTIME_DATE_REMOVAL_ARGS: &[&str] = &[
    "-QuickTime:CreateDate=",
    "-QuickTime:ModifyDate=",
    "-TrackCreateDate=",
    "-TrackModifyDate=",
    "-MediaCreateDate=",
    "-MediaModifyDate=",
];

const RAW_IDENTIFYING_TAG_DELETES: &[&str] = &[
    "-IFD0:Artist=",
    "-IFD0:Software=",
    "-IFD0:ImageDescription=",
    "-IFD0:Copyright=",
    "-IFD0:XPComment=",
    "-IFD0:XPAuthor=",
    "-IFD0:XPTitle=",
    "-IFD0:XPSubject=",
    "-IFD0:XPKeywords=",
    "-ExifIFD:UserComment=",
    "-ExifIFD:SerialNumber=",
    "-ExifIFD:LensSerialNumber=",
    "-ExifIFD:OwnerName=",
    "-IFD0:CameraSerialNumber=",
    "-IFD0:OriginalRawFileName=",
    "-IFD0:RawDataUniqueID=",
    "-MakerNotes:OwnerName=",
    "-MakerNotes:InternalSerialNumber=",
    "-ExifIFD:DateTimeOriginal=",
    "-ExifIFD:CreateDate=",
    "-IFD0:ModifyDate=",
    "-ExifIFD:OffsetTime=",
    "-ExifIFD:OffsetTimeOriginal=",
    "-ExifIFD:OffsetTimeDigitized=",
    "-ExifIFD:SubSecTime=",
    "-ExifIFD:SubSecTimeOriginal=",
    "-ExifIFD:SubSecTimeDigitized=",
];

const RESOLUTION_PRESERVE_ARGS: &[&str] = &[
    "-JFIF:XResolution>JFIF:XResolution",
    "-JFIF:YResolution>JFIF:YResolution",
    "-JFIF:ResolutionUnit>JFIF:ResolutionUnit",
    "-IFD0:XResolution>IFD0:XResolution",
    "-IFD0:YResolution>IFD0:YResolution",
    "-IFD0:ResolutionUnit>IFD0:ResolutionUnit",
    "-PNG:PixelsPerUnitX>PNG:PixelsPerUnitX",
    "-PNG:PixelsPerUnitY>PNG:PixelsPerUnitY",
    "-PNG:PixelUnits>PNG:PixelUnits",
];

const DISPLAY_ARGS: &[&str] = &["-G1:2:4"];

pub struct ExifToolAdapter {
    process: ExiftoolProcess,
}

impl ExifToolAdapter {
    pub fn new(process: ExiftoolProcess) -> Self {
        Self { process }
    }

    pub async fn open(&self) -> Result<u32, ExifToolError> {
        self.process.open().await
    }

    pub async fn close(&self) -> Result<(), ExifToolError> {
        self.process.close().await
    }

    pub async fn read_display_metadata(
        &self,
        file_path: &str,
    ) -> Result<Map<String, Value>, String> {
        let result = self
            .process
            .read_metadata(file_path, DISPLAY_ARGS)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(err) = result.error {
            return Err(err);
        }

        let Some(records) = result.data else {
            return Ok(Map::new());
        };
        let Some(first) = records.into_iter().next() else {
            return Ok(Map::new());
        };
        Ok(clean_exif_data(&first))
    }

    pub async fn strip_metadata(
        &self,
        file_path: &str,
        output_path: Option<&Path>,
        settings: &Settings,
    ) -> Result<(), String> {
        let mut extra_args: Vec<String> = vec!["-all=".into()];

        if is_media(file_path) {
            extra_args.extend(QUICKTIME_DATE_REMOVAL_ARGS.iter().map(|s| (*s).to_string()));
        }
        if is_tiff(file_path) {
            extra_args.push("-CommonIFD0=".into());
        }
        if is_raw(file_path) {
            extra_args.extend(RAW_IDENTIFYING_TAG_DELETES.iter().map(|s| (*s).to_string()));
            extra_args.extend(QUICKTIME_DATE_REMOVAL_ARGS.iter().map(|s| (*s).to_string()));
        }

        let mut preserve_tags: Vec<String> = Vec::new();
        if settings.preserve_orientation {
            preserve_tags.push("-Orientation".into());
        }
        if settings.preserve_color_profile {
            preserve_tags.push("-ICC_Profile".into());
        }
        if settings.preserve_resolution {
            preserve_tags.extend(RESOLUTION_PRESERVE_ARGS.iter().map(|s| (*s).to_string()));
        }
        if !preserve_tags.is_empty() {
            extra_args.push("-TagsFromFile".into());
            extra_args.push("@".into());
            extra_args.extend(preserve_tags);
        }

        if let Some(dest) = output_path {
            extra_args.push("-o".into());
            extra_args.push(dest.to_string_lossy().into_owned());
        } else {
            extra_args.push("-overwrite_original".into());
        }

        let source_bytes = fs::metadata(file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let deadline = write_deadline_ms(source_bytes);

        let result = self
            .process
            .write_metadata(file_path, &extra_args, deadline)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(err) = result.error {
            return Err(err);
        }
        Ok(())
    }
}
