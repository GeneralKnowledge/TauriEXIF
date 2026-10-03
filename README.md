# TauriEXIF

A Tauri + Rust port of [ExifCleaner](https://github.com/szTheory/exifcleaner).

The Electron/Node main process is replaced by a Rust backend that speaks ExifTool’s `-stay_open` protocol. The UI is a small vanilla TypeScript frontend (no React).

## Why this exists

ExifCleaner is a privacy tool that strips metadata from images, media, and PDFs. This project is an MVP port meant to show the practical difference between:

| Layer | ExifCleaner | TauriEXIF |
| --- | --- | --- |
| Shell | Electron | Tauri 2 |
| Backend language | TypeScript / Node | Rust |
| UI | React 19 | Vanilla TypeScript |
| Metadata engine | Bundled ExifTool | Same ExifTool (system or bundled) |

## MVP features

- Drag/drop or pick files and folders
- Batch metadata stripping via ExifTool stay-open
- Before/after metadata inspection
- Save as copy
- Preserve orientation / color profile / resolution
- Light / dark / system theme
- RAF refused (same safety stance as upstream)
- RAW files forced to copy mode when overwrite is selected

Not in MVP: i18n, macOS xattr removal, preserve timestamps, native menus, full verified output transactions for every guarded format.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install)
- Node.js 20+
- [ExifTool](https://exiftool.org/) on your `PATH` for development
- Platform Tauri prerequisites ([docs](https://tauri.app/start/prerequisites/))

## Develop

```bash
npm install
npm run tauri:dev
```

## Build

```bash
npm run tauri:build
```

Optional: place platform ExifTool binaries under `src-tauri/resources/bin/` so packaged apps do not depend on a system install. See [`src-tauri/resources/README.md`](src-tauri/resources/README.md).

## Architecture

```text
UI (vanilla TS)
  └─ invoke() / dialog plugin
       └─ Tauri commands (Rust)
            ├─ classify_paths / expand_folder
            ├─ get_settings / set_settings
            └─ read_metadata / remove_metadata
                 └─ ExifToolAdapter
                      └─ ExiftoolProcess (-stay_open True -@ -)
```

Core Rust modules:

- `src-tauri/src/exiftool/` — stay-open process, stdout parser, sanitize/read adapter
- `src-tauri/src/domain/` — file types, cleaned paths, settings, outcome classification
- `src-tauri/src/commands/` — Tauri command surface

## License

MIT, same spirit as upstream ExifCleaner. ExifTool remains under its own license.
