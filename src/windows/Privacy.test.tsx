import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { PrivacyReport } from "../lib/api";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

const { default: Privacy } = await import("./Privacy");

const report: PrivacyReport = {
  network: { level: "ok", detail: "Nessuna connessione: FrameNote non contiene componenti di rete." },
  diskEncryption: { level: "warn", detail: "FileVault è spento." },
  projectsDir: "~/FrameNote/Progetti",
  cloudSync: { level: "ok", detail: "Non sincronizzata con servizi cloud." },
  otherUsers: { level: "ok", detail: "Leggibili solo dal tuo utente." },
  signature: { level: "ok", detail: "Firma stabile con Hardened Runtime." },
  captureExclusions: ["Finestre di FrameNote", "Notifiche", "Password", "1Password"],
};

beforeEach(() => {
  invoke.mockReset();
});

describe("Privacy e sicurezza", () => {
  it("mostra lo stato e guida ad attivare FileVault", async () => {
    invoke.mockImplementation(async (cmd: string) => (cmd === "privacy_report" ? report : undefined));
    const user = userEvent.setup();
    render(<Privacy onBack={() => {}} />);

    const rete = (await screen.findByText("Connessioni a internet")).closest("li")!;
    expect(within(rete).getByLabelText("A posto")).toBeVisible();
    const fv = screen.getByText("Cifratura del disco (FileVault)").closest("li")!;
    expect(within(fv).getByLabelText("Da sistemare")).toBeVisible();
    expect(screen.getByText("~/FrameNote/Progetti")).toBeVisible();
    expect(screen.getByText(/Notifiche, Password, 1Password/)).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Apri le impostazioni di FileVault" }));
    expect(invoke).toHaveBeenCalledWith("open_filevault_settings");
  });

  it("con FileVault attivo non propone nulla", async () => {
    invoke.mockImplementation(async () => ({
      ...report,
      diskEncryption: { level: "ok", detail: "FileVault è attivo: il disco è cifrato." },
    }));
    render(<Privacy onBack={() => {}} />);
    expect(await screen.findByText("FileVault è attivo: il disco è cifrato.")).toBeVisible();
    expect(screen.queryByRole("button", { name: /FileVault/ })).toBeNull();
  });
});
