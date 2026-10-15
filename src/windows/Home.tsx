import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "../lib/api";

interface Props {
  onOpenDiagnostics: () => void;
  onOpenZoomTest: () => void;
  onOpenPrivacy: () => void;
}

export default function Home({ onOpenDiagnostics, onOpenZoomTest, onOpenPrivacy }: Props) {
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
        <h2 className="font-medium">Prova zoom (M1)</h2>
        <p className="mt-1 text-sm text-neutral-600 dark:text-neutral-400">
          Registra lo schermo per qualche secondo e fai qualche clic: FrameNote crea un video con zoom
          automatici e cursore smussato.
        </p>
        <button
          type="button"
          onClick={onOpenZoomTest}
          className="mt-4 rounded-md bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700"
        >
          Prova zoom
        </button>
      </section>

      <section className="rounded-lg border border-neutral-200 p-5 dark:border-neutral-700">
        <h2 className="font-medium">Privacy e sicurezza</h2>
        <p className="mt-1 text-sm text-neutral-600 dark:text-neutral-400">
          Controlla che le tue registrazioni restino solo su questo computer: rete, cifratura del disco,
          cartelle, app escluse dai video.
        </p>
        <button
          type="button"
          onClick={onOpenPrivacy}
          className="mt-4 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium hover:bg-neutral-100 dark:border-neutral-600 dark:hover:bg-neutral-800"
        >
          Privacy e sicurezza
        </button>
      </section>

      <section className="rounded-lg border border-neutral-200 p-5 dark:border-neutral-700">
        <h2 className="font-medium">Info e diagnostica</h2>
        <p className="mt-1 text-sm text-neutral-600 dark:text-neutral-400">
          Encoder video, GPU, cartelle e database: utile da allegare in chat se qualcosa non va.
        </p>
        <button
          type="button"
          onClick={onOpenDiagnostics}
          className="mt-4 rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium hover:bg-neutral-100 dark:border-neutral-600 dark:hover:bg-neutral-800"
        >
          Info e diagnostica
        </button>
      </section>
    </main>
  );
}
