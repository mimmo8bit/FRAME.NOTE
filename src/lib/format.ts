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
