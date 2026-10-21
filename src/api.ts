import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { EngineVersion, FileUpdate, InspectResult, JobPayload, Settings } from "./api-types";
import { t } from "./i18n";

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function plain<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export async function defaultOutput(): Promise<string> {
  return invoke<string>("default_output");
}

export async function inspectPaths(paths: string[]): Promise<InspectResult> {
  return invoke<InspectResult>("inspect", { paths: plain(paths) });
}

export async function validateOutput(output: string, roots: string[]): Promise<void> {
  await invoke("validate_output", { output, roots: plain(roots) });
}

export async function loadSettings(): Promise<Settings> {
  return invoke<Settings>("load_settings");
}

export async function saveSettings(settings: Settings): Promise<void> {
  await invoke("save_settings", { settings: plain(settings) });
}

export async function engineVersions(): Promise<EngineVersion[]> {
  return invoke<EngineVersion[]>("engine_versions");
}

export async function startBatch(payload: JobPayload): Promise<void> {
  await invoke("start_batch", { request: plain(payload) });
}

export async function enqueueJobs(jobs: JobPayload["jobs"]): Promise<void> {
  await invoke("enqueue_jobs", { jobs: plain(jobs) });
}

export async function cancelBatch(): Promise<void> {
  await invoke("cancel_batch");
}

export async function removeWaiting(id: string): Promise<void> {
  await invoke("remove_waiting", { id });
}

export async function pickFiles(): Promise<string[]> {
  const selected = await open({
    multiple: true,
    filters: [{ name: t("filesFilter"), extensions: ["jpg", "jpeg", "png", "gif", "svg", "zip"] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickDirectory(): Promise<string | null> {
  const selected = await open({ directory: true });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function revealDirectory(path: string): Promise<void> {
  await invoke("open_output", { path });
}

export async function pickSave(defaultPath: string, extension: string): Promise<string | null> {
  const selected = await save({
    defaultPath,
    filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
  });
  return selected ?? null;
}

export async function saveCopy(source: string, dest: string): Promise<void> {
  await invoke("save_copy", { source, dest });
}

export async function exportZip(dest: string, files: Array<{ path: string; name: string }>): Promise<void> {
  await invoke("export_zip", { dest, files: plain(files) });
}

export async function readPreview(path: string): Promise<{ mime: string; base64: string }> {
  return invoke("read_preview", { path });
}

export async function readThumbnail(path: string): Promise<{ mime: string; base64: string }> {
  return invoke("read_thumbnail", { path });
}

export async function onDragDrop(
  handler: (event: { type: string; paths?: string[] }) => void,
): Promise<UnlistenFn> {
  const webview = getCurrentWebview();
  return webview.onDragDropEvent((event) => {
    handler(event.payload);
  });
}

export function onFileUpdated(handler: (update: FileUpdate) => void): Promise<UnlistenFn> {
  return listen<FileUpdate>("file-updated", (event) => handler(event.payload));
}

export function onBatchFinished(handler: () => void): Promise<UnlistenFn> {
  return listen("batch-finished", () => handler());
}
