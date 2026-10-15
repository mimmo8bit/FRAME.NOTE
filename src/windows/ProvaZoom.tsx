import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  api,
  errorText,
  type CaptureStatus,
  type ExportProgress,
  type ExportSummary,
  type RecordingStatus,
  type RecordingSummary,
} from "../lib/api";
import { formatDuration, formatScale } from "../lib/format";

interface Props {
  onBack: () => void;
}

type Phase =
  | { kind: "loading" }
  | { kind: "idle" }
  | { kind: "starting" }
  | { kind: "recording"; live: RecordingStatus | null }
  | { kind: "stopping" }
  | { kind: "recorded"; summary: RecordingSummary }
  | { kind: "exporting"; summary: RecordingSummary; progress: ExportProgress | null }
  | { kind: "exported"; summary: RecordingSummary; result: ExportSummary };

const btn =
  "rounded-md px-4 py-2 text-sm font-medium disabled:opacity-50";
const primary = `${btn} bg-red-600 text-white hover:bg-red-700`;
const secondary = `${btn} border border-neutral-300 hover:bg-neutral-100 dark:border-neutral-600 dark:hover:bg-neutral-800`;

/** Scelte per il ritorno a schermo intero (ms). */
export const HOLD_CHOICES_MS = [1000, 1500, 2000, 3000, 4000, 5000];

export default function ProvaZoom({ onBack }: Props) {
  const [status, setStatus] = useState<CaptureStatus | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const [error, setError] = useState<string | null>(null);
  const [last, setLast] = useState<RecordingSummary | null>(null);
  const polling = useRef<number | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await api.captureStatus();
      setStatus(s);
      setPhase((p) =>
        p.kind === "loading" ? (s.recording ? { kind: "recording", live: null } : { kind: "idle" }) : p,
      );
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
    api
      .lastRecording()
      .then((r) => setLast(r ?? null))
      .catch(() => setLast(null));
  }, [refreshStatus]);

  // Aggiornamento dal vivo durante la registrazione.
  useEffect(() => {
    if (phase.kind !== "recording") return;
    const tick = async () => {
      try {
        const live = await api.recordingStatus();
        setPhase((p) => (p.kind === "recording" ? { kind: "recording", live } : p));
        if (live.error) setError(live.error);
      } catch (e) {
        setError(errorText(e));
      }
    };
    void tick();
    polling.current = window.setInterval(tick, 500);
    return () => {
      if (polling.current !== null) window.clearInterval(polling.current);
    };
  }, [phase.kind]);

  const start = async () => {
    setError(null);
    setPhase({ kind: "starting" });
    try {
      await api.startRecording();
      setPhase({ kind: "recording", live: null });
    } catch (e) {
      setError(errorText(e));
      setPhase({ kind: "idle" });
      void refreshStatus();
    }
  };

  const stop = async () => {
    setPhase({ kind: "stopping" });
    try {
      const summary = await api.stopRecording();
      setLast(summary);
      setPhase({ kind: "recorded", summary });
    } catch (e) {
      setError(errorText(e));
      setPhase({ kind: "idle" });
    }
  };

  const replan = async (summary: RecordingSummary, holdMs: number) => {
    setError(null);
    try {
      const updated = await api.replanZooms(summary.projectDir, holdMs);
      setLast(updated);
      setPhase({ kind: "recorded", summary: updated });
    } catch (e) {
      setError(errorText(e));
    }
  };

  const exportZoom = async (summary: RecordingSummary) => {
    setError(null);
    setPhase({ kind: "exporting", summary, progress: null });
    const unlisten = await listen<ExportProgress>("zoom-export-progress", (ev) => {
      setPhase((p) => (p.kind === "exporting" ? { ...p, progress: ev.payload } : p));
    });
    try {
      const result = await api.exportZoom(summary.projectDir);
      setPhase({ kind: "exported", summary, result });
    } catch (e) {
      setError(errorText(e));
      setPhase({ kind: "recorded", summary });
    } finally {
      unlisten();
    }
  };

  return (
    <main className="mx-auto flex max-w-3xl flex-col gap-6 px-8 py-8">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={onBack}
          disabled={phase.kind === "recording" || phase.kind === "exporting" || phase.kind === "stopping"}
          className="rounded-md px-2 py-1 text-sm text-neutral-600 hover:bg-neutral-100 disabled:opacity-40 dark:text-neutral-300 dark:hover:bg-neutral-800"
        >
          ← Indietro
        </button>
        <h1 className="text-2xl font-semibold tracking-tight">Prova zoom (M1)</h1>
      </div>

      {error && (
        <p role="alert" className="rounded-lg border border-red-300 bg-red-50 p-3 text-sm text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          {error}
        </p>
      )}

      {phase.kind === "loading" && <p role="status">Controllo il permesso di registrazione…</p>}

      {status && !status.supported && (
        <Card title="Non disponibile su questo sistema">
          <p>{status.message}</p>
        </Card>
      )}

      {status?.supported && !status.permissionGranted && phase.kind === "idle" && (
        <Permission onRecheck={refreshStatus} message={status.message} />
      )}

      {status?.supported && status.permissionGranted && (phase.kind === "idle" || phase.kind === "starting") && (
        <Card title="Registra lo schermo">
          <p>
            Registra circa <strong>30 secondi</strong> di qualcosa di non riservato (per esempio una pagina
            web) e fai <strong>5 clic</strong> in punti diversi. FrameNote calcolerà gli zoom da sola.
          </p>
          <p className="mt-2 text-sm text-neutral-500">
            Viene registrato lo schermo principale, senza audio. Le finestre di FrameNote, le notifiche e i password manager non compaiono nel video.
          </p>
          <div className="mt-4 flex flex-wrap gap-2">
            <button type="button" className={primary} onClick={start} disabled={phase.kind === "starting"}>
              {phase.kind === "starting" ? "Avvio…" : "● Avvia registrazione"}
            </button>
            {last && (
              <button
                type="button"
                className={secondary}
                disabled={phase.kind === "starting"}
                onClick={() => setPhase({ kind: "recorded", summary: last })}
              >
                Riapri l'ultima registrazione ({formatDuration(last.durationMs)})
              </button>
            )}
          </div>
        </Card>
      )}

      {phase.kind === "recording" && (
        <Card title="Registrazione in corso">
          <p className="font-mono text-4xl tabular-nums" aria-live="polite">
            {formatDuration(phase.live?.elapsedMs ?? 0)}
          </p>
          <p className="mt-1 text-sm text-neutral-500">
            Clic rilevati: {phase.live?.clicks ?? 0}
            {phase.live && phase.live.dropped > 0 && ` · frame persi: ${phase.live.dropped}`}
          </p>
          <div className="mt-4">
            <button type="button" className={primary} onClick={stop}>
              ■ Ferma
            </button>
          </div>
        </Card>
      )}

      {phase.kind === "stopping" && (
        <p role="status">Chiudo il video e calcolo gli zoom…</p>
      )}

      {(phase.kind === "recorded" || phase.kind === "exporting" || phase.kind === "exported") && (
        <Recorded
          summary={phase.summary}
          onReplan={phase.kind === "exporting" ? undefined : (holdMs) => replan(phase.summary, holdMs)}
        />
      )}

      {phase.kind === "recorded" && (
        <div className="flex gap-2">
          <button type="button" className={primary} onClick={() => exportZoom(phase.summary)}>
            Esporta con zoom
          </button>
          <button type="button" className={secondary} onClick={() => setPhase({ kind: "idle" })}>
            Nuova registrazione
          </button>
        </div>
      )}

      {phase.kind === "exporting" && (
        <Card title="Esportazione in corso">
          <Progress progress={phase.progress} />
          <div className="mt-3">
            <button type="button" className={secondary} onClick={() => void api.cancelExport()}>
              Annulla
            </button>
          </div>
        </Card>
      )}

      {phase.kind === "exported" && (
        <Card title="Video pronto">
          <p>
            {phase.result.width}×{phase.result.height} · {phase.result.frames} fotogrammi · esportato in{" "}
            {(phase.result.renderMs / 1000).toLocaleString("it-IT", { maximumFractionDigits: 1 })} s
          </p>
          <div className="mt-4 flex flex-wrap gap-2">
            <button
              type="button"
              className={primary}
              onClick={() => api.openProjectFile(phase.result.output).catch((e) => setError(errorText(e)))}
            >
              Apri il video
            </button>
            <button
              type="button"
              className={secondary}
              onClick={() => revealItemInDir(phase.result.output).catch((e) => setError(errorText(e)))}
            >
              Mostra nella cartella
            </button>
            <button type="button" className={secondary} onClick={() => setPhase({ kind: "idle" })}>
              Nuova registrazione
            </button>
          </div>
          <p className="mt-3 text-sm text-neutral-500">
            Guardalo e dimmi se gli zoom sono fluidi, se il cursore è morbido e se l'immagine è nitida.
          </p>
        </Card>
      )}
    </main>
  );
}

function Permission({ onRecheck, message }: { onRecheck: () => void; message: string | null }) {
  const [asked, setAsked] = useState(false);
  return (
    <Card title="Serve il permesso «Registrazione schermo»">
      <p>{message}</p>
      <ol className="mt-3 list-decimal space-y-1 pl-5 text-sm">
        <li>Premi <strong>Concedi il permesso</strong>: macOS mostra una richiesta.</li>
        <li>
          Se non compare, premi <strong>Apri Impostazioni di Sistema</strong> e attiva FrameNote in{" "}
          <em>Privacy &amp; Security → Screen &amp; System Audio Recording</em>.
        </li>
        <li>Se macOS lo chiede, chiudi e riapri FrameNote.</li>
      </ol>
      <div className="mt-4 flex flex-wrap gap-2">
        <button
          type="button"
          className={primary}
          onClick={async () => {
            await api.requestScreenPermission().catch(() => false);
            setAsked(true);
            onRecheck();
          }}
        >
          Concedi il permesso
        </button>
        <button type="button" className={secondary} onClick={() => void api.openScreenPermissionSettings()}>
          Apri Impostazioni di Sistema
        </button>
        <button type="button" className={secondary} onClick={onRecheck}>
          Controlla di nuovo
        </button>
      </div>
      {asked && (
        <p className="mt-3 text-sm text-neutral-500">
          Se hai appena attivato il permesso e qui non cambia nulla, chiudi FrameNote (⌘Q) e riaprila.
        </p>
      )}
    </Card>
  );
}

function formatSeconds(ms: number): string {
  return `${(ms / 1000).toLocaleString("it-IT", { maximumFractionDigits: 1 })} s`;
}

function Recorded({
  summary,
  onReplan,
}: {
  summary: RecordingSummary;
  onReplan?: (holdMs: number) => Promise<void>;
}) {
  const r = summary.report;
  const [hold, setHold] = useState(summary.holdMs);
  const [busy, setBusy] = useState(false);
  useEffect(() => setHold(summary.holdMs), [summary.holdMs]);
  const choices = HOLD_CHOICES_MS.includes(summary.holdMs)
    ? HOLD_CHOICES_MS
    : [...HOLD_CHOICES_MS, summary.holdMs].sort((a, b) => a - b);
  return (
    <Card title="Registrazione">
      <p>
        {formatDuration(summary.durationMs)} · {summary.width}×{summary.height} · {r.clicks} clic
      </p>
      <h3 className="mt-3 text-sm font-medium">
        {summary.zoomBlocks.length === 0
          ? "Nessuno zoom: non sono stati rilevati clic."
          : `${summary.zoomBlocks.length} zoom ${summary.zoomBlocks.length === 1 ? "calcolato" : "calcolati"}`}
      </h3>
      {summary.zoomBlocks.length === 0 && r.clicks > 0 && (
        <p className="mt-1 text-sm text-neutral-500">
          I clic erano veloci e in punti lontani: la ripresa resta a schermo intero.
        </p>
      )}
      {summary.zoomBlocks.length > 0 && (
        <ol className="mt-1 list-decimal space-y-0.5 pl-5 text-sm">
          {summary.zoomBlocks.map((b, i) => (
            <li key={i}>
              {formatDuration(b.startMs)}–{formatDuration(b.endMs)} a {formatScale(b.scale)}
              {b.clicks > 1 && ` (${b.clicks} clic)`}
            </li>
          ))}
        </ol>
      )}
      {onReplan && (
        <div className="mt-4 flex flex-wrap items-end gap-2 border-t border-neutral-200 pt-4 dark:border-neutral-700">
          <label className="flex flex-col gap-1 text-sm">
            Torna a schermo intero dopo
            <select
              value={hold}
              onChange={(e) => setHold(Number(e.target.value))}
              className="rounded-md border border-neutral-300 bg-transparent px-2 py-1.5 dark:border-neutral-600"
            >
              {choices.map((ms) => (
                <option key={ms} value={ms}>
                  {formatSeconds(ms)} senza clic
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className={secondary}
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              await onReplan(hold);
              setBusy(false);
            }}
          >
            {busy ? "Ricalcolo…" : "Ricalcola zoom"}
          </button>
        </div>
      )}
      <p className="mt-3 text-xs text-neutral-500">
        Frame persi: {r.framesDropped} · CPU media {Math.round(r.cpuAvgPercent)}% di un core (
        {Math.round(r.cpuAvgTotalPercent)}% del totale) · encoder {r.encoder}
      </p>
    </Card>
  );
}

function Progress({ progress }: { progress: ExportProgress | null }) {
  const pct = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  return (
    <div>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct}
        className="h-2 w-full overflow-hidden rounded bg-neutral-200 dark:bg-neutral-700"
      >
        <div className="h-full bg-red-600 transition-[width]" style={{ width: `${pct}%` }} />
      </div>
      <p className="mt-1 text-sm text-neutral-500">{pct}%</p>
    </div>
  );
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-neutral-200 p-5 dark:border-neutral-700">
      <h2 className="mb-2 font-medium">{title}</h2>
      {children}
    </section>
  );
}
