import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";

type ThemeMode = "light" | "dark" | "system";

interface Settings {
  preserveOrientation: boolean;
  preserveColorProfile: boolean;
  preserveResolution: boolean;
  saveAsCopy: boolean;
  themeMode: ThemeMode;
}

interface ClassifiedFile {
  path: string;
  size: number;
}

interface ClassifyResult {
  files: ClassifiedFile[];
  folders: string[];
  unsupported: string[];
}

interface ExpandResult {
  files: ClassifiedFile[];
  skippedCount: number;
  error?: string | null;
}

type OutcomeKind =
  | "cleaned"
  | "already-clean"
  | "unchanged"
  | "refused"
  | "failed"
  | "pending"
  | "processing";

interface RemoveMetadataResult {
  success: boolean;
  outputPath?: string | null;
  outputSize?: number | null;
  wroteFile: boolean;
  wasForcedCopy: boolean;
  outcomeKind: OutcomeKind;
  beforeMetadata: Record<string, unknown>;
  afterMetadata: Record<string, unknown>;
  removedCount: number;
  beforeCount: number;
  afterCount: number;
  error?: string | null;
  refusalReason?: string | null;
}

interface FileEntry {
  id: string;
  path: string;
  name: string;
  size: number;
  status: OutcomeKind;
  beforeMetadata: Record<string, unknown>;
  afterMetadata: Record<string, unknown>;
  removedCount: number;
  beforeCount: number;
  afterCount: number;
  outputPath?: string;
  outputSize?: number;
  error?: string;
  expanded: boolean;
}

const files = new Map<string, FileEntry>();
let settings: Settings = {
  preserveOrientation: true,
  preserveColorProfile: true,
  preserveResolution: true,
  saveAsCopy: true,
  themeMode: "system",
};
let queue: string[] = [];
let processing = false;

const dropzone = () => el<HTMLElement>("#dropzone");
const tbody = () => el<HTMLTableSectionElement>("#file-tbody");
const results = () => el<HTMLElement>("#results");
const statusSummary = () => el<HTMLElement>("#status-summary");
const drawer = () => el<HTMLElement>("#settings-drawer");

function el<T extends HTMLElement>(selector: string): T {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`Missing element ${selector}`);
  return node;
}

function basename(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let value = n;
  let i = -1;
  do {
    value /= 1024;
    i += 1;
  } while (value >= 1024 && i < units.length - 1);
  return `${value.toFixed(value >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}

function applyTheme(mode: ThemeMode) {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const dark = mode === "dark" || (mode === "system" && prefersDark);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

async function loadSettings() {
  settings = await invoke<Settings>("get_settings");
  applyTheme(settings.themeMode);
  el<HTMLInputElement>("#opt-save-copy").checked = settings.saveAsCopy;
  el<HTMLInputElement>("#opt-orientation").checked = settings.preserveOrientation;
  el<HTMLInputElement>("#opt-color").checked = settings.preserveColorProfile;
  el<HTMLInputElement>("#opt-resolution").checked = settings.preserveResolution;
  for (const radio of document.querySelectorAll<HTMLInputElement>('input[name="theme"]')) {
    radio.checked = radio.value === settings.themeMode;
  }
}

async function persistSettings() {
  settings = {
    saveAsCopy: el<HTMLInputElement>("#opt-save-copy").checked,
    preserveOrientation: el<HTMLInputElement>("#opt-orientation").checked,
    preserveColorProfile: el<HTMLInputElement>("#opt-color").checked,
    preserveResolution: el<HTMLInputElement>("#opt-resolution").checked,
    themeMode:
      (document.querySelector<HTMLInputElement>('input[name="theme"]:checked')?.value as ThemeMode) ||
      "system",
  };
  settings = await invoke<Settings>("set_settings", { patch: settings });
  applyTheme(settings.themeMode);
}

function upsertEntry(file: ClassifiedFile): FileEntry {
  const existing = [...files.values()].find((f) => f.path === file.path);
  if (existing) return existing;
  const entry: FileEntry = {
    id: crypto.randomUUID(),
    path: file.path,
    name: basename(file.path),
    size: file.size,
    status: "pending",
    beforeMetadata: {},
    afterMetadata: {},
    removedCount: 0,
    beforeCount: 0,
    afterCount: 0,
    expanded: false,
  };
  files.set(entry.id, entry);
  queue.push(entry.id);
  return entry;
}

function render() {
  const list = [...files.values()];
  results().classList.toggle("hidden", list.length === 0);

  const cleaned = list.filter((f) => f.status === "cleaned").length;
  const failed = list.filter((f) => f.status === "failed" || f.status === "refused").length;
  const pending = list.filter((f) => f.status === "pending" || f.status === "processing").length;
  statusSummary().textContent = `${list.length} file${list.length === 1 ? "" : "s"} · ${cleaned} cleaned · ${failed} failed · ${pending} pending`;

  tbody().innerHTML = list
    .map((file) => {
      const statusClass =
        file.status === "cleaned" || file.status === "already-clean"
          ? "ok"
          : file.status === "failed" || file.status === "refused"
            ? "err"
            : file.status === "unchanged"
              ? "warn"
              : "pending";
      const metaLabel =
        file.status === "pending" || file.status === "processing"
          ? "—"
          : `${file.removedCount} removed (${file.beforeCount} → ${file.afterCount})`;
      const metaDetails =
        file.expanded && Object.keys(file.beforeMetadata).length > 0
          ? `<div class="meta-panel">${escapeHtml(formatDiff(file))}</div>`
          : "";
      return `<tr data-id="${file.id}">
        <td>
          <div class="file-name">${escapeHtml(file.name)}</div>
          <span class="file-path">${escapeHtml(file.outputPath || file.path)}</span>
          ${file.error ? `<div class="file-path" style="color:var(--err)">${escapeHtml(file.error)}</div>` : ""}
        </td>
        <td class="col-size">${formatBytes(file.outputSize ?? file.size)}</td>
        <td>
          ${
            file.status === "pending" || file.status === "processing"
              ? metaLabel
              : `<button type="button" class="meta-btn" data-action="toggle-meta">${escapeHtml(metaLabel)}</button>`
          }
          ${metaDetails}
        </td>
        <td><span class="pill ${statusClass}">${escapeHtml(file.status)}</span></td>
      </tr>`;
    })
    .join("");
}

function formatDiff(file: FileEntry): string {
  const lines: string[] = [];
  for (const [key, value] of Object.entries(file.beforeMetadata)) {
    const gone = !(key in file.afterMetadata);
    lines.push(`${gone ? "−" : " "} ${key}: ${stringify(value)}`);
  }
  for (const [key, value] of Object.entries(file.afterMetadata)) {
    if (!(key in file.beforeMetadata)) {
      lines.push(`+ ${key}: ${stringify(value)}`);
    }
  }
  return lines.join("\n") || "(no removable metadata)";
}

function stringify(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

async function ingestPaths(paths: string[]) {
  if (paths.length === 0) return;
  const classified = await invoke<ClassifyResult>("classify_paths", { paths });
  for (const file of classified.files) {
    upsertEntry(file);
  }
  for (const folder of classified.folders) {
    const expanded = await invoke<ExpandResult>("expand_folder", { dirPath: folder });
    for (const file of expanded.files) {
      upsertEntry(file);
    }
  }
  render();
  void drainQueue();
}

async function drainQueue() {
  if (processing) return;
  processing = true;
  while (queue.length > 0) {
    const id = queue.shift();
    if (!id) continue;
    const entry = files.get(id);
    if (!entry) continue;
    entry.status = "processing";
    render();
    try {
      const result = await invoke<RemoveMetadataResult>("remove_metadata", {
        filePath: entry.path,
      });
      entry.status = result.outcomeKind;
      entry.beforeMetadata = result.beforeMetadata ?? {};
      entry.afterMetadata = result.afterMetadata ?? {};
      entry.removedCount = result.removedCount;
      entry.beforeCount = result.beforeCount;
      entry.afterCount = result.afterCount;
      entry.outputPath = result.outputPath ?? undefined;
      entry.outputSize = result.outputSize ?? undefined;
      entry.error = result.error ?? undefined;
    } catch (error) {
      entry.status = "failed";
      entry.error = error instanceof Error ? error.message : String(error);
    }
    render();
  }
  processing = false;
}

async function chooseFiles() {
  const selected = await open({
    multiple: true,
    title: "Choose files to clean",
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await ingestPaths(paths);
}

async function chooseFolder() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Choose a folder to clean",
  });
  if (!selected || Array.isArray(selected)) return;
  await ingestPaths([selected]);
}

function bindUi() {
  el<HTMLButtonElement>("#btn-files").addEventListener("click", () => void chooseFiles());
  el<HTMLButtonElement>("#btn-folder").addEventListener("click", () => void chooseFolder());
  el<HTMLButtonElement>("#btn-clear").addEventListener("click", () => {
    files.clear();
    queue = [];
    render();
  });

  el<HTMLButtonElement>("#btn-settings").addEventListener("click", () => {
    drawer().classList.add("open");
    drawer().setAttribute("aria-hidden", "false");
  });
  el<HTMLButtonElement>("#btn-close-settings").addEventListener("click", () => {
    drawer().classList.remove("open");
    drawer().setAttribute("aria-hidden", "true");
  });
  drawer().addEventListener("click", (event) => {
    if (event.target === drawer()) {
      drawer().classList.remove("open");
      drawer().setAttribute("aria-hidden", "true");
    }
  });

  for (const id of ["#opt-save-copy", "#opt-orientation", "#opt-color", "#opt-resolution"]) {
    el<HTMLInputElement>(id).addEventListener("change", () => void persistSettings());
  }
  for (const radio of document.querySelectorAll<HTMLInputElement>('input[name="theme"]')) {
    radio.addEventListener("change", () => void persistSettings());
  }

  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (settings.themeMode === "system") applyTheme("system");
  });

  const zone = dropzone();
  void getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "enter" || event.payload.type === "over") {
      zone.classList.add("dragover");
    } else if (event.payload.type === "leave") {
      zone.classList.remove("dragover");
    } else if (event.payload.type === "drop") {
      zone.classList.remove("dragover");
      void ingestPaths(event.payload.paths);
    }
  });

  tbody().addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest("tr[data-id]");
    if (!row) return;
    const id = row.getAttribute("data-id");
    if (!id) return;
    const entry = files.get(id);
    if (!entry) return;
    if (target.closest('[data-action="toggle-meta"]')) {
      entry.expanded = !entry.expanded;
      render();
    }
  });
}

window.addEventListener("DOMContentLoaded", () => {
  bindUi();
  void loadSettings().then(render);
});
