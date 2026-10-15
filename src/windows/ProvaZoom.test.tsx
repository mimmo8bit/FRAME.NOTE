import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { RecordingSummary } from "../lib/api";

const invoke = vi.fn();
const listeners: Record<string, (ev: { payload: unknown }) => void> = {};
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, cb: (ev: { payload: unknown }) => void) => {
    listeners[name] = cb;
    return () => delete listeners[name];
  },
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));

const { default: ProvaZoom } = await import("./ProvaZoom");

const summary: RecordingSummary = {
  projectDir: "/Users/dom/Documents/FrameNote/Progetti/2026-10-13_1915_prova-zoom",
  durationMs: 31_000,
  width: 3024,
  height: 1964,
  clicks: 5,
  zoomBlocks: [
    { startMs: 2600, endMs: 5700, scale: 2, centerX: 756, centerY: 491, clicks: 2 },
    { startMs: 10_600, endMs: 12_500, scale: 2, centerX: 2268, centerY: 1473, clicks: 1 },
  ],
  holdMs: 2000,
  report: {
    durationMs: 31_000,
    framesReceived: 640,
    framesIdle: 200,
    framesDropped: 0,
    framesWritten: 930,
    framesRepeated: 290,
    mouseEvents: 2500,
    clicks: 5,
    cpuAvgPercent: 42,
    cpuMaxPercent: 60,
    cpuAvgTotalPercent: 4.2,
    encoder: "h264_videotoolbox",
  },
};

type Handlers = Record<string, (args?: unknown) => unknown>;

function mockCommands(h: Handlers) {
  invoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const fn = h[cmd];
    if (!fn) return undefined;
    return fn(args);
  });
}

const ok = { supported: true, permissionGranted: true, message: null, recording: false };

beforeEach(() => {
  invoke.mockReset();
});

describe("Prova zoom", () => {
  it("su Windows spiega che non è ancora disponibile", async () => {
    mockCommands({
      capture_status: () => ({
        supported: false,
        permissionGranted: false,
        message: "Su questo sistema la registrazione arriva in una prossima versione",
        recording: false,
      }),
    });
    render(<ProvaZoom onBack={() => {}} />);
    expect(await screen.findByText(/prossima versione/)).toBeVisible();
    expect(screen.queryByRole("button", { name: /Avvia registrazione/ })).toBeNull();
  });

  it("senza permesso guida a concederlo", async () => {
    let granted = false;
    mockCommands({
      capture_status: () => ({ ...ok, permissionGranted: granted, message: "Serve il permesso" }),
      request_screen_permission: () => {
        granted = true;
        return true;
      },
    });
    const user = userEvent.setup();
    render(<ProvaZoom onBack={() => {}} />);
    await user.click(await screen.findByRole("button", { name: "Concedi il permesso" }));
    expect(invoke).toHaveBeenCalledWith("request_screen_permission");
    expect(await screen.findByRole("button", { name: /Avvia registrazione/ })).toBeVisible();
  });

  it("registra, mostra gli zoom ed esporta", async () => {
    mockCommands({
      capture_status: () => ok,
      start_recording: () => ({ projectDir: summary.projectDir }),
      recording_status: () => ({
        recording: true,
        elapsedMs: 12_300,
        frames: 369,
        clicks: 3,
        dropped: 0,
        error: null,
      }),
      stop_recording: () => summary,
      export_zoom: () => {
        listeners["zoom-export-progress"]?.({ payload: { done: 450, total: 930 } });
        return {
          output: `${summary.projectDir}/exports/zoom.mp4`,
          width: 1920,
          height: 1246,
          frames: 930,
          renderMs: 41_500,
        };
      },
    });
    const user = userEvent.setup();
    render(<ProvaZoom onBack={() => {}} />);
    await user.click(await screen.findByRole("button", { name: /Avvia registrazione/ }));
    expect(await screen.findByText("0:12")).toBeVisible();
    expect(screen.getByText(/Clic rilevati: 3/)).toBeVisible();

    await user.click(screen.getByRole("button", { name: /Ferma/ }));
    expect(await screen.findByText("2 zoom calcolati")).toBeVisible();
    expect(screen.getByText(/0:02–0:05 a 2,0× \(2 clic\)/)).toBeVisible();
    expect(screen.getByText(/Frame persi: 0/)).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Esporta con zoom" }));
    expect(invoke).toHaveBeenCalledWith("export_zoom", { projectDir: summary.projectDir });
    expect(await screen.findByText("Video pronto")).toBeVisible();
    expect(screen.getByText(/1920×1246 · 930 fotogrammi · esportato in 41,5 s/)).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Apri il video" }));
    expect(invoke).toHaveBeenCalledWith("open_project_file", {
      path: `${summary.projectDir}/exports/zoom.mp4`,
    });
  });

  it("ricalcola gli zoom con un altro ritorno a schermo intero", async () => {
    mockCommands({
      capture_status: () => ok,
      last_recording: () => summary,
      replan_zooms: () => ({ ...summary, holdMs: 4000, zoomBlocks: [summary.zoomBlocks[0]] }),
    });
    const user = userEvent.setup();
    render(<ProvaZoom onBack={() => {}} />);
    await user.click(await screen.findByRole("button", { name: /Riapri l'ultima registrazione \(0:31\)/ }));
    expect(screen.getByText("2 zoom calcolati")).toBeVisible();

    const select = screen.getByLabelText("Torna a schermo intero dopo");
    expect(select).toHaveValue("2000");
    await user.selectOptions(select, "4000");
    await user.click(screen.getByRole("button", { name: "Ricalcola zoom" }));
    expect(invoke).toHaveBeenCalledWith("replan_zooms", {
      projectDir: summary.projectDir,
      holdMs: 4000,
    });
    expect(await screen.findByText("1 zoom calcolato")).toBeVisible();
    expect(screen.getByLabelText("Torna a schermo intero dopo")).toHaveValue("4000");
  });

  it("spiega perché con clic sparsi non ci sono zoom", async () => {
    mockCommands({
      capture_status: () => ok,
      last_recording: () => ({ ...summary, zoomBlocks: [] }),
    });
    const user = userEvent.setup();
    render(<ProvaZoom onBack={() => {}} />);
    await user.click(await screen.findByRole("button", { name: /Riapri l'ultima registrazione/ }));
    expect(screen.getByText(/la ripresa resta a schermo intero/)).toBeVisible();
  });

  it("mostra gli errori della registrazione", async () => {
    mockCommands({
      capture_status: () => ok,
      start_recording: () => {
        throw "FFmpeg non si avvia (permesso negato)";
      },
    });
    const user = userEvent.setup();
    render(<ProvaZoom onBack={() => {}} />);
    await user.click(await screen.findByRole("button", { name: /Avvia registrazione/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("FFmpeg non si avvia");
    expect(screen.getByRole("button", { name: /Avvia registrazione/ })).toBeEnabled();
  });
});
