const nf1 = new Intl.NumberFormat("it-IT", { maximumFractionDigits: 1 });

/** 1536 → "1,5 KB"; usa multipli di 1024 come Finder/Esplora risorse. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "n/d";
  const units = ["byte", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return unit === 0 ? `${value} byte` : `${nf1.format(value)} ${units[unit]}`;
}

/** "2026-10-13T15:07:00+02:00" → "13/10/2026, 15:07". */
export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("it-IT", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** 83500 → "1:23". */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

const nf2 = new Intl.NumberFormat("it-IT", { minimumFractionDigits: 1, maximumFractionDigits: 1 });

/** 2 → "2,0×". */
export function formatScale(scale: number): string {
  return `${nf2.format(scale)}×`;
}
