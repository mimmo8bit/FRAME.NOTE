import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "../lib/api";

interface Props {
  onOpenDiagnostics: () => void;
}

export default function Home({ onOpenDiagnostics }: Props) {
  const [version, setVersion] = useState<string | null>(null);
  const [problems, setProblems] = useState<string[]>([]);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
    api
      .startupProblems()
      .then(setProblems)
      .catch(() => setProblems([]));
  }, []);

  return (
    <main className="mx-auto flex min-h-screen max-w-2xl flex-col gap-8 px-8 py-12">
      <header>
        <h1 className="text-3xl font-semibold tracking-tight">FrameNote</h1>
        <p className="mt-2 text-neutral-600 dark:text-neutral-400">
          Registrazioni, video per i clienti e note delle call. Tutto resta su questo computer.
        </p>
        {version && (
          <p className="mt-1 text-sm text-neutral-500">Versione {version}</p>
        )}
      </header>

      {problems.length > 0 && (
        <section
          role="alert"
          className="rounded-lg border border-amber-300 bg-amber-50 p-4 text-amber-900 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-100"
        >
          <h2 className="font-medium">Qualcosa non è andato all'avvio</h2>
          <ul className="mt-2 list-disc space-y-1 pl-5 text-sm">
            {problems.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </section>
      )}

      <section className="rounded-lg border border-neutral-200 p-5 dark:border-neutral-700">
        <h2 className="font-medium">Prima versione di prova</h2>
        <p className="mt-1 text-sm text-neutral-600 dark:text-neutral-400">
          La registrazione arriva nelle prossime versioni. Per ora puoi controllare che FrameNote
          veda correttamente il tuo Mac: encoder video, GPU, cartelle e database.
        </p>
        <button
          type="button"
          onClick={onOpenDiagnostics}
          className="mt-4 rounded-md bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700"
        >
          Info e diagnostica
        </button>
      </section>
    </main>
  );
}
