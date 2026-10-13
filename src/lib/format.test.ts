import { describe, expect, it } from "vitest";
import { formatBytes, formatDateTime } from "./format";

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
