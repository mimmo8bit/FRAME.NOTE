// Ponte tipizzato verso i comandi Rust (src-tauri/src/commands.rs).
import { invoke } from "@tauri-apps/api/core";

export interface EncoderProbe {
  name: string;
  description: string;
  hardware: boolean;
  working: boolean;
  durationMs: number;
  error: string | null;
}

export interface FfmpegReport {
  found: boolean;
  path: string;
  version: {
    version: string | null;
    configuration: string | null;
    licenseLgpl: boolean | null;
  };
  encoders: EncoderProbe[];
  preferred: string | null;
  preferredHardware: boolean | null;
  error: string | null;
}

export interface GpuInfo {
  name: string;
  backend: string;
  deviceType: string;
  driver: string;
}

export interface DbStatus {
  ok: boolean;
  schemaVersion: number | null;
  sizeBytes: number | null;
  fts5: boolean;
  sqliteVersion: string;
  createdWith: string | null;
  error: string | null;
}

export interface DiagnosticInfo {
  generatedAt: string;
  app: {
    name: string;
    version: string;
    commit: string;
    identifier: string;
    debugBuild: boolean;
  };
  os: { name: string; version: string; kernel: string; arch: string };
  cpu: { brand: string; physicalCores: number | null; logicalCores: number };
  memory: { totalBytes: number; availableBytes: number };
  gpus: GpuInfo[];
  gpuError: string | null;
  ffmpeg: FfmpegReport;
  paths: {
    projectsDir: string;
    dataDir: string;
    logsDir: string;
    modelsDir: string;
    database: string;
  };
  database: DbStatus;
  disk: { projectsFreeBytes: number | null; dataFreeBytes: number | null };
  models: { installed: { fileName: string; sizeBytes: number }[] };
}

export interface ExportResult {
  path: string;
  entries: string[];
}

export const api = {
  diagnosticInfo: () => invoke<DiagnosticInfo>("diagnostic_info"),
  startupProblems: () => invoke<string[]>("startup_problems"),
  defaultDiagnosticsFileName: () => invoke<string>("default_diagnostics_file_name"),
  exportDiagnostics: (dest: string) => invoke<ExportResult>("export_diagnostics", { dest }),
  log: (level: "info" | "warn" | "error", message: string) =>
    invoke<void>("log_frontend", { level, message }).catch(() => {
      // Se anche il log fallisce non c'è altro da fare: l'interfaccia non deve rompersi.
    }),
};

/** Testo di un errore restituito da un comando (sempre una stringa in italiano). */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Errore sconosciuto";
}
