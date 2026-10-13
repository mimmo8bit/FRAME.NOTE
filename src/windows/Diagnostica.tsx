import { useCallback, useEffect, useState, type ReactNode } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorText, type DiagnosticInfo, type ExportResult } from "../lib/api";
import { formatBytes, formatDateTime } from "../lib/format";

interface Props {
  onBack: () => void;
}

type Load =
  | { state: "loading" }
  | { state: "error"; message: string }
  | { state: "ready"; info: DiagnosticInfo };

type Export =
  | { state: "idle" }
  | { state: "saving" }
  | { state: "done"; result: ExportResult }
  | { state: "error"; message: string };

export default function Diagnostica({ onBack }: Props) {
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [exp, setExp] = useState<Export>({ state: "idle" });

  const refresh = useCallback(() => {
    setLoad({ state: "loading" });
    api
      .diagnosticInfo()
      .then((info) => setLoad({ state: "ready", info }))
      .catch((e) => setLoad({ state: "error", message: errorText(e) }));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack]);

  const exportZip = async () => {
    try {
      const defaultPath = await api.defaultDiagnosticsFileName();
      const dest = await save({
        title: "Salva la diagnostica",
        defaultPath,
        filters: [{ name: "Archivio zip", extensions: ["zip"] }],
      });
      if (!dest) return; // annullato
      setExp({ state: "saving" });
      const result = await api.exportDiagnostics(dest);
      setExp({ state: "done", result });
    } catch (e) {
      setExp({ state: "error", message: errorText(e) });
    }
  };

  return (
    <main className="mx-auto flex max-w-3xl flex-col gap-6 px-8 py-8">
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={onBack}
          className="rounded-md px-2 py-1 text-sm text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
        >
          ← Indietro
        </button>
        <h1 className="text-2xl font-semibold tracking-tight">Info e diagnostica</h1>
        <div className="ml-auto flex gap-2">
          <button
            type="button"
            onClick={refresh}
            disabled={load.state === "loading"}
            className="rounded-md border border-neutral-300 px-3 py-1.5 text-sm hover:bg-neutral-100 disabled:opacity-50 dark:border-neutral-600 dark:hover:bg-neutral-800"
          >
            Aggiorna
          </button>
          <button
            type="button"
            onClick={exportZip}
            disabled={exp.state === "saving"}
            className="rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-50"
          >
            {exp.state === "saving" ? "Salvataggio…" : "Esporta diagnostica"}
          </button>
        </div>
      </div>

      <ExportStatus exp={exp} />

      {load.state === "loading" && (
        <p role="status" className="text-neutral-600 dark:text-neutral-400">
          Sto controllando FFmpeg, gli encoder video e la GPU: ci vuole qualche secondo…
        </p>
      )}
      {load.state === "error" && (
        <p role="alert" className="text-red-700 dark:text-red-400">
          {load.message}
        </p>
      )}
      {load.state === "ready" && <Report info={load.info} />}
    </main>
  );
}

function ExportStatus({ exp }: { exp: Export }) {
  if (exp.state === "done") {
    return (
      <div
        role="status"
        className="flex flex-wrap items-center gap-3 rounded-lg border border-green-300 bg-green-50 p-3 text-sm text-green-900 dark:border-green-800 dark:bg-green-950 dark:text-green-100"
      >
        <span>
          Diagnostica salvata ({exp.result.entries.length} file). Allegala in chat.
        </span>
        <button
          type="button"
          onClick={() => {
            revealItemInDir(exp.result.path).catch((e) =>
              api.log("warn", `mostra nella cartella: ${errorText(e)}`),
            );
          }}
          className="rounded-md border border-green-400 px-2 py-1 hover:bg-green-100 dark:border-green-700 dark:hover:bg-green-900"
        >
          Mostra nella cartella
        </button>
      </div>
    );
  }
  if (exp.state === "error") {
    return (
      <p role="alert" className="text-sm text-red-700 dark:text-red-400">
        {exp.message}
      </p>
    );
  }
  return null;
}

export function Report({ info }: { info: DiagnosticInfo }) {
  const ff = info.ffmpeg;
  const preferred = ff.encoders.find((e) => e.name === ff.preferred);
  return (
    <div className="flex flex-col gap-4">
      <Section title="Encoder video">
        {!ff.found || ff.error ? (
          <Bad>{ff.error ?? "FFmpeg non trovato"}</Bad>
        ) : preferred ? (
          <p>
            <strong className="font-mono">{preferred.name}</strong>{" "}
            <Badge>{preferred.hardware ? "hardware" : "software"}</Badge>
            <span className="ml-2 text-sm text-neutral-500">{preferred.description}</span>
          </p>
        ) : ff.encoders.length === 0 ? (
          <Bad>Questo FFmpeg non contiene encoder H.264/HEVC adatti.</Bad>
        ) : (
          <Bad>Nessun encoder H.264/HEVC ha superato la prova di codifica.</Bad>
        )}
        {ff.encoders.length > 0 && (
          <ul className="mt-3 space-y-1 text-sm">
            {ff.encoders.map((e) => (
              <li key={e.name}>
                <span aria-hidden>{e.working ? "✅" : "❌"}</span>{" "}
                <span className="font-mono">{e.name}</span>{" "}
                <span className="text-neutral-500">
                  {e.working ? `prova riuscita in ${e.durationMs} ms` : `non disponibile: ${e.error ?? ""}`}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="GPU">
        {info.gpus.length === 0 ? (
          <Bad>{info.gpuError ?? "Nessuna GPU rilevata"}</Bad>
        ) : (
          <ul className="space-y-1">
            {info.gpus.map((g) => (
              <li key={`${g.name}-${g.backend}`}>
                {g.name} <Badge>{g.backend}</Badge>{" "}
                <span className="text-sm text-neutral-500">
                  {g.deviceType}
                  {g.driver && ` · ${g.driver}`}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title="App e sistema">
        <Rows
          rows={[
            ["Versione", `${info.app.version}${info.app.debugBuild ? " (debug)" : ""}`],
            ["Commit", info.app.commit],
            ["Sistema", `${info.os.name} ${info.os.version} (${info.os.arch})`],
            ["Processore", `${info.cpu.brand} · ${info.cpu.logicalCores} core logici`],
            [
              "Memoria",
              `${formatBytes(info.memory.totalBytes)} (libera ${formatBytes(info.memory.availableBytes)})`,
            ],
            [
              "FFmpeg",
              ff.version.version
                ? `${ff.version.version}${ff.version.licenseLgpl ? " · LGPL" : ""}`
                : "n/d",
            ],
            ["Rilevato il", formatDateTime(info.generatedAt)],
          ]}
        />
      </Section>

      <Section title="Database e spazio">
        <Rows
          rows={[
            [
              "Database",
              info.database.ok
                ? `ok · schema v${info.database.schemaVersion} · ${formatBytes(info.database.sizeBytes)}`
                : `problema: ${info.database.error ?? "sconosciuto"}`,
            ],
            ["Ricerca testo (FTS5)", info.database.fts5 ? "disponibile" : "non disponibile"],
            ["Spazio libero (progetti)", formatBytes(info.disk.projectsFreeBytes)],
            ["Spazio libero (dati app)", formatBytes(info.disk.dataFreeBytes)],
            [
              "Modelli AI",
              info.models.installed.length === 0
                ? "nessun modello installato"
                : info.models.installed
                    .map((m) => `${m.fileName} (${formatBytes(m.sizeBytes)})`)
                    .join(", "),
            ],
          ]}
        />
      </Section>

      <Section title="Cartelle">
        <Rows
          mono
          rows={[
            ["Progetti", info.paths.projectsDir],
            ["Dati app", info.paths.dataDir],
            ["Log", info.paths.logsDir],
            ["Modelli", info.paths.modelsDir],
          ]}
        />
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-neutral-200 p-4 dark:border-neutral-700">
      <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-neutral-500">
        {title}
      </h2>
      {children}
    </section>
  );
}

function Rows({ rows, mono }: { rows: [string, string][]; mono?: boolean }) {
  return (
    <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-neutral-500">{k}</dt>
          <dd className={`break-all ${mono ? "font-mono text-xs leading-5" : ""}`}>{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function Badge({ children }: { children: ReactNode }) {
  return (
    <span className="rounded bg-neutral-100 px-1.5 py-0.5 text-xs font-medium text-neutral-700 dark:bg-neutral-800 dark:text-neutral-300">
      {children}
    </span>
  );
}

function Bad({ children }: { children: ReactNode }) {
  return <p className="text-red-700 dark:text-red-400">{children}</p>;
}
