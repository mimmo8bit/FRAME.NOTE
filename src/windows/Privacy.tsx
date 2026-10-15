import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, errorText, type Check, type PrivacyReport } from "../lib/api";

interface Props {
  onBack: () => void;
}

const SYMBOL: Record<Check["level"], { sign: string; label: string; cls: string }> = {
  ok: { sign: "✓", label: "A posto", cls: "text-green-700 dark:text-green-400" },
  warn: { sign: "!", label: "Da sistemare", cls: "text-amber-700 dark:text-amber-400" },
  unknown: { sign: "?", label: "Non verificabile", cls: "text-neutral-500" },
};

export default function Privacy({ onBack }: Props) {
  const [report, setReport] = useState<PrivacyReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    setError(null);
    api
      .privacyReport()
      .then(setReport)
      .catch((e) => setError(errorText(e)));
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
        <h1 className="text-2xl font-semibold tracking-tight">Privacy e sicurezza</h1>
        <button
          type="button"
          onClick={refresh}
          className="ml-auto rounded-md border border-neutral-300 px-3 py-1.5 text-sm hover:bg-neutral-100 dark:border-neutral-600 dark:hover:bg-neutral-800"
        >
          Controlla di nuovo
        </button>
      </div>

      {error && (
        <p role="alert" className="text-red-700 dark:text-red-400">
          {error}
        </p>
      )}
      {!report && !error && <p role="status">Controllo la protezione dei tuoi dati…</p>}

      {report && (
        <ul className="flex flex-col gap-3">
          <Row title="Connessioni a internet" check={report.network} />
          <Row title="Cifratura del disco (FileVault)" check={report.diskEncryption}>
            {report.diskEncryption.level === "warn" && (
              <div className="mt-2 text-sm">
                <p>
                  Attivala in <em>Impostazioni di Sistema → Privacy e sicurezza → FileVault → Attiva</em>{" "}
                  (<em>Privacy &amp; Security → FileVault → Turn On</em>). Salva la chiave di recupero in un posto
                  sicuro: senza password e senza chiave i dati non si recuperano.
                </p>
                <button
                  type="button"
                  onClick={() => api.openFilevaultSettings().catch((e) => setError(errorText(e)))}
                  className="mt-2 rounded-md border border-neutral-300 px-3 py-1.5 hover:bg-neutral-100 dark:border-neutral-600 dark:hover:bg-neutral-800"
                >
                  Apri le impostazioni di FileVault
                </button>
              </div>
            )}
          </Row>
          <Row title="Cartella dei progetti" check={report.cloudSync}>
            <p className="mt-1 text-sm">
              <code className="text-xs">{report.projectsDir}</code>
            </p>
          </Row>
          <Row title="Altri utenti di questo computer" check={report.otherUsers} />
          <Row title="Firma dell'app" check={report.signature} />
          <Row
            title="Esclusi dalla registrazione"
            check={{
              level: "ok",
              detail: "Non compaiono mai nei video, anche se si aprono durante la registrazione:",
            }}
          >
            <p className="mt-1 text-sm">{report.captureExclusions.join(", ")}.</p>
          </Row>
          <Row
            title="Diagnostica"
            check={{
              level: "ok",
              detail:
                "Esce dal computer solo se la salvi e la alleghi tu. Contiene dati tecnici, mai video o audio; prima di salvarla vedi l'elenco esatto dei file.",
            }}
          />
        </ul>
      )}
    </main>
  );
}

function Row({ title, check, children }: { title: string; check: Check; children?: ReactNode }) {
  const s = SYMBOL[check.level];
  return (
    <li className="flex gap-3 rounded-lg border border-neutral-200 p-4 dark:border-neutral-700">
      <span aria-label={s.label} title={s.label} className={`w-5 shrink-0 text-center font-bold ${s.cls}`}>
        {s.sign}
      </span>
      <div className="min-w-0">
        <h2 className="font-medium">{title}</h2>
        <p className="text-sm text-neutral-600 dark:text-neutral-400">{check.detail}</p>
        {children}
      </div>
    </li>
  );
}
