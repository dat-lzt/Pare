export type OutputFormat = "original" | "webp" | "jpeg" | "png";
export type Appearance = "system" | "light" | "dark";
export type Language = "system" | "zh-CN" | "en";
export type Preset = "photo" | "ui" | "lossless";
export type Kind = "jpeg" | "png" | "gif" | "svg" | "other";
export type Status =
  | "waiting"
  | "running"
  | "done"
  | "unchanged"
  | "first-frame"
  | "unsupported"
  | "failed"
  | "cancelled";

export interface EngineToggles {
  oxipng: boolean;
  pngquant: boolean;
  zopflipng: boolean;
  jpegtran: boolean;
  jpegoptim: boolean;
  gifsicle: boolean;
  svgcleaner: boolean;
}

export interface Settings {
  outputFormat: OutputFormat;
  preset: Preset;
  lossy: boolean;
  engines: EngineToggles;
  concurrency: number;
  gifFirstFrame: boolean;
  debugLog: boolean;
  appearance: Appearance;
  language: Language;
}

export interface FileRecord {
  inputPath: string;
  relativePath: string;
  inputBytes: number;
  kind: Kind;
  animated: boolean;
  hasAlpha: boolean;
  note?: string;
}

export interface InspectResult {
  files: FileRecord[];
  roots: string[];
  suggestedOutput: string | null;
}

export interface FileRow extends FileRecord {
  id: string;
  status: Status;
  outputBytes: number | null;
  deltaRatio: number | null;
  note: string;
  outputPath: string | null;
}

export interface EngineVersion {
  name: string;
  version: string;
}

export interface FileUpdate {
  id: string;
  status: Status;
  outputBytes: number | null;
  deltaRatio: number | null;
  note: string;
  outputPath: string | null;
}

export interface EngineVersion {
  name: string;
  version: string;
}

export const STATUS_LABEL: Record<Status, string> = {
  waiting: "等待",
  running: "处理中",
  done: "完成",
  unchanged: "未变小",
  "first-frame": "已用第一帧",
  unsupported: "不支持",
  failed: "失败",
  cancelled: "已取消",
};

export function formatBytes(bytes: number | null): string {
  if (bytes == null) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatDelta(ratio: number | null): string {
  if (ratio == null) return "—";
  const pct = Math.round(ratio * 100);
  if (pct === 0) return "0%";
  return pct > 0 ? `+${pct}%` : `${pct}%`;
}
