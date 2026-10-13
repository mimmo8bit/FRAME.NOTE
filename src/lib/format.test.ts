import { describe, expect, it } from "vitest";
import { formatBytes, formatDateTime, formatDuration, formatScale } from "./format";

describe("formatBytes", () => {
  it("formatta in italiano con multipli di 1024", () => {
    expect(formatBytes(0)).toBe("0 byte");
    expect(formatBytes(1536)).toBe("1,5 KB");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16 GB");
    expect(formatBytes(1.25 * 1024 ** 4)).toBe("1,3 TB");
  });
  it("valori mancanti", () => {
    expect(formatBytes(null)).toBe("n/d");
    expect(formatBytes(undefined)).toBe("n/d");
  });
});

describe("formatDateTime", () => {
  it("lascia invariata una data non valida", () => {
    expect(formatDateTime("non una data")).toBe("non una data");
  });
  it("formatta una data valida", () => {
    expect(formatDateTime("2026-10-13T15:07:00Z")).toMatch(/13\/10\/2026/);
  });
});

describe("formatDuration e formatScale", () => {
  it("minuti e secondi", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(83_500)).toBe("1:23");
    expect(formatDuration(600_000)).toBe("10:00");
  });
  it("livello di zoom", () => {
    expect(formatScale(2)).toBe("2,0×");
    expect(formatScale(1.75)).toBe("1,8×");
  });
});
