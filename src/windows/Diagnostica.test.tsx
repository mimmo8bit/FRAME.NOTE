import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { DiagnosticInfo } from "../lib/api";

const invoke = vi.fn();
const save = vi.fn();
const revealItemInDir = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: (...a: unknown[]) => save(...a) }));
vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: (...a: unknown[]) => revealItemInDir(...a),
}));

const { default: Diagnostica } = await import("./Diagnostica");

// Dati nel formato prodotto da src-tauri/src/diagnostics/info.rs.
const info: DiagnosticInfo = {
  generatedAt: "2026-10-13T15:07:00+02:00",
  app: {
    name: "FrameNote",
    version: "0.0.1",
    commit: "37cc80a8eb39",
    identifier: "com.mimmo8bit.framenote",
    debugBuild: false,
  },
  os: { name: "Darwin", version: "macOS 26.6", kernel: "25.6.0", arch: "aarch64" },
  cpu: { brand: "Apple M1 Pro", physicalCores: 10, logicalCores: 10 },
  memory: { totalBytes: 16 * 1024 ** 3, availableBytes: 8 * 1024 ** 3 },
  gpus: [{ name: "Apple M1 Pro", backend: "Metal", deviceType: "integrata", driver: "" }],
  gpuError: null,
  ffmpeg: {
    found: true,
    path: "/Applications/FrameNote.app/Contents/MacOS/ffmpeg",
    version: { version: "8.1.2", configuration: "--disable-gpl", licenseLgpl: true },
    encoders: [
      {
        name: "h264_videotoolbox",
        description: "VideoToolbox H.264 Encoder (codec h264)",
        hardware: true,
        working: true,
        durationMs: 180,
        error: null,
      },
      {
        name: "hevc_videotoolbox",
        description: "VideoToolbox H.265 Encoder (codec hevc)",
        hardware: true,
        working: false,
        durationMs: 40,
        error: "Error while opening encoder",
      },
    ],
    preferred: "h264_videotoolbox",
    preferredHardware: true,
    error: null,
  },
  paths: {
    projectsDir: "~/Documents/FrameNote/Progetti",
    dataDir: "~/Library/Application Support/com.mimmo8bit.framenote",
    logsDir: "~/Library/Application Support/com.mimmo8bit.framenote/logs",
    modelsDir: "~/Library/Application Support/com.mimmo8bit.framenote/models",
    database: "~/Library/Application Support/com.mimmo8bit.framenote/framenote.sqlite",
  },
  database: {
    ok: true,
    schemaVersion: 1,
    sizeBytes: 32768,
    fts5: true,
    sqliteVersion: "3.50.0",
    createdWith: "0.0.1",
    error: null,
  },
  disk: { projectsFreeBytes: 200 * 1024 ** 3, dataFreeBytes: 200 * 1024 ** 3 },
  models: { installed: [] },
};

function mockCommands(overrides: Record<string, () => unknown> = {}) {
  invoke.mockImplementation(async (cmd: string) => {
    if (overrides[cmd]) return overrides[cmd]();
    switch (cmd) {
      case "diagnostic_info":
        return info;
      case "default_diagnostics_file_name":
        return "framenote-diagnostica-20261013-1507.zip";
      case "export_diagnostics":
        return { path: "/Users/dom/Desktop/d.zip", entries: ["info.json", "logs/a.log"] };
      case "diagnostics_preview":
        return [
          { name: "info.json", description: "Dati tecnici" },
          { name: "logs/a.log", description: "Registro tecnico dell'app" },
        ];
      default:
        return undefined;
    }
  });
}

beforeEach(() => {
  invoke.mockReset();
  save.mockReset();
  revealItemInDir.mockReset();
});

describe("Info e diagnostica", () => {
  it("mostra encoder, GPU, database e modelli", async () => {
    mockCommands();
    render(<Diagnostica onBack={() => {}} />);
    expect(screen.getByRole("status")).toHaveTextContent("qualche secondo");
    expect(await screen.findByText("h264_videotoolbox", { selector: "strong" })).toBeVisible();
    expect(screen.getByText("hardware")).toBeVisible();
    expect(screen.getByText(/non disponibile: Error while opening encoder/)).toBeVisible();
    expect(screen.getByText("Metal")).toBeVisible();
    expect(screen.getByText("nessun modello installato")).toBeVisible();
    expect(screen.getByText(/ok · schema v1/)).toBeVisible();
    expect(screen.getByText("8.1.2 · LGPL")).toBeVisible();
    expect(screen.getByText("16 GB (libera 8 GB)")).toBeVisible();
  });

  it("segnala FFmpeg mancante", async () => {
    mockCommands({
      diagnostic_info: () => ({
        ...info,
        ffmpeg: { ...info.ffmpeg, found: false, encoders: [], preferred: null, error: "FFmpeg non trovato accanto all'app" },
      }),
    });
    render(<Diagnostica onBack={() => {}} />);
    expect(await screen.findByText("FFmpeg non trovato accanto all'app")).toBeVisible();
  });

  it("esporta lo zip e lo mostra nella cartella", async () => {
    mockCommands();
    save.mockResolvedValue("/Users/dom/Desktop/d.zip");
    revealItemInDir.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<Diagnostica onBack={() => {}} />);
    await screen.findByText("h264_videotoolbox", { selector: "strong" });

    await user.click(screen.getByRole("button", { name: "Esporta diagnostica" }));
    // Prima l'elenco dei file, poi la finestra di salvataggio.
    expect(await screen.findByText("Cosa contiene la diagnostica")).toBeVisible();
    expect(screen.getByText("logs/a.log")).toBeVisible();
    expect(save).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Salva…" }));
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: "framenote-diagnostica-20261013-1507.zip" }),
    );
    expect(invoke).toHaveBeenCalledWith("export_diagnostics", { dest: "/Users/dom/Desktop/d.zip" });
    expect(await screen.findByText(/Diagnostica salvata \(2 file\)/)).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Mostra nella cartella" }));
    expect(revealItemInDir).toHaveBeenCalledWith("/Users/dom/Desktop/d.zip");
  });

  it("annullare il salvataggio non esporta nulla", async () => {
    mockCommands();
    save.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<Diagnostica onBack={() => {}} />);
    await screen.findByText("h264_videotoolbox", { selector: "strong" });
    await user.click(screen.getByRole("button", { name: "Esporta diagnostica" }));
    await user.click(await screen.findByRole("button", { name: "Salva…" }));
    expect(invoke).not.toHaveBeenCalledWith("export_diagnostics", expect.anything());
    expect(screen.queryByText("Cosa contiene la diagnostica")).toBeNull();
  });

  it("mostra l'errore di esportazione", async () => {
    mockCommands({
      export_diagnostics: () => {
        throw "Non sono riuscito a salvare il file di diagnostica (disco pieno).";
      },
    });
    save.mockResolvedValue("/x/d.zip");
    const user = userEvent.setup();
    render(<Diagnostica onBack={() => {}} />);
    await screen.findByText("h264_videotoolbox", { selector: "strong" });
    await user.click(screen.getByRole("button", { name: "Esporta diagnostica" }));
    await user.click(await screen.findByRole("button", { name: "Salva…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("disco pieno");
  });

  it("Esc torna indietro", async () => {
    mockCommands();
    const onBack = vi.fn();
    const user = userEvent.setup();
    render(<Diagnostica onBack={onBack} />);
    await user.keyboard("{Escape}");
    expect(onBack).toHaveBeenCalled();
  });
});
