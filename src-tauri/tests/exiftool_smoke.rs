use std::process::Command;
use std::time::Duration;
use tauri_exif_lib::test_support::{
    default_settings, resolve_exiftool_bin, ExifToolAdapter, ExiftoolProcess,
};
use tokio::time::timeout;

fn exiftool_available() -> bool {
    Command::new("exiftool").arg("-ver").output().is_ok()
}

/// Compact 1×1 JPEG that ExifTool can annotate.
fn tiny_jpeg() -> Vec<u8> {
    hex::decode(concat!(
        "ffd8ffe000104a46494600010100000100010000",
        "ffdb004300080606070605080707070909080a0c140d0c0b0b0c",
        "1912130f141d1a1f1e1d1a1c1c20242e2720222c231c1c283729",
        "2c30313434341f27393d38323c2e333432",
        "ffc0000b080001000101011100",
        "ffc4001f0000010501010101010100000000000000000102030405060708090a0b",
        "ffc400b5100002010303020403050504040000017d010203000411051221314106",
        "13516107227114328191a1082342b1c11552d1f02433627282090a161718191a25",
        "262728292a3435363738393a434445464748494a535455565758595a6364656667",
        "68696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6",
        "a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2",
        "e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9fa",
        "ffda0008010100003f00fbd5db20a8f147ffd9"
    ))
    .expect("valid hex")
}

#[tokio::test]
async fn stay_open_read_and_strip_jpeg() {
    if !exiftool_available() {
        eprintln!("skipping: exiftool not on PATH");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let image = dir.path().join("sample.jpg");
    std::fs::write(&image, tiny_jpeg()).expect("write jpeg");

    let tagged = Command::new("exiftool")
        .args(["-overwrite_original", "-Artist=TauriEXIF", image.to_str().unwrap()])
        .status()
        .expect("tag jpeg");
    assert!(tagged.success(), "exiftool failed to tag sample jpeg");

    let bin = resolve_exiftool_bin(None);
    let adapter = ExifToolAdapter::new(ExiftoolProcess::new(bin));
    timeout(Duration::from_secs(10), adapter.open())
        .await
        .expect("open timeout")
        .expect("open exiftool");

    let before = adapter
        .read_display_metadata(image.to_str().unwrap())
        .await
        .expect("read before");
    assert!(
        before.keys().any(|k| k.contains("Artist")),
        "expected Artist in before metadata: {before:?}"
    );

    adapter
        .strip_metadata(image.to_str().unwrap(), None, &default_settings())
        .await
        .expect("strip");

    let after = adapter
        .read_display_metadata(image.to_str().unwrap())
        .await
        .expect("read after");
    assert!(
        !after.keys().any(|k| k.contains("Artist")),
        "Artist should be removed: {after:?}"
    );

    adapter.close().await.expect("close");
}
