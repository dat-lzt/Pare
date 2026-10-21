import type { FileRecord, FileUpdate, Settings } from "./types";

export type { FileUpdate, Settings };

export interface InspectResult {
  files: FileRecord[];
  roots: string[];
  suggestedOutput: string | null;
}

export interface EngineVersion {
  name: string;
  version: string;
}

export interface JobPayload {
  jobs: Array<{
    id: string;
    inputPath: string;
    relativePath: string;
    kind: FileRecord["kind"];
    animated: boolean;
    inputBytes: number;
  }>;
  settings: Settings;
  outputDir: string;
  roots: string[];
}
