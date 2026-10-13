# FrameNote — Decisioni

Registro delle decisioni di progetto. Ogni voce: data, decisione, motivo, alternative scartate.

## 2026-10-13 — OS primario: macOS
- **Decisione:** l'OS primario è macOS (MacBook M1 Pro, 16 GB RAM, macOS 26.6). I test manuali si fanno lì. Windows è compilato in CI e rifinito in M9.
- **Conseguenze:** encoder di riferimento `h264_videotoolbox` / `hevc_videotoolbox`; accelerazione AI via Metal. Con 16 GB il preset "Completo" (large-v3-turbo + Qwen2.5-7B Q4_K_M) è praticabile, ma va verificato che non giri insieme a una registrazione 4K.

## 2026-10-13 — Sviluppo in Claude Code cloud
- **Decisione:** il codice si scrive, compila e testa in sessioni Claude Code cloud collegate al repository GitHub. Sul Mac di Domenico non si installa nessuna toolchain.
- **Motivo:** sul Mac mancano Rust, Node e `gh`, e il vincolo zero-install vieta di installarli. In locale si potrebbe solo scrivere codice senza compilarlo.
- **Alternativa scartata:** scrivere il codice in locale e verificarlo solo in CI (ciclo di feedback troppo lento).

## 2026-10-13 — Repository GitHub pubblico
- **Decisione:** il repository è pubblico.
- **Motivo:** i minuti di GitHub Actions sui runner standard dovrebbero essere gratuiti per i repository pubblici; sui privati i runner macOS consumano la quota circa 10 volte più in fretta. Da ricontrollare sulla documentazione attuale di GitHub in M0.
- **Vincolo derivato:** nel repository non vanno mai chiavi, token, registrazioni o dati di clienti. Il certificato di firma self-signed resta solo nei secret di GitHub Actions.

## 2026-10-13 — Limiti di GitHub Actions verificati (repo pubblico)
- **Fonte:** documentazione ufficiale GitHub letta il 2026-10-13 (le pagine non riportano una data di aggiornamento):
  - https://docs.github.com/en/billing/concepts/product-billing/github-actions
  - https://docs.github.com/en/billing/reference/actions-runner-pricing
  - https://docs.github.com/en/actions/reference/limits
  - https://docs.github.com/en/actions/reference/runners/github-hosted-runners
  - https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases
- **Minuti:** sui repository pubblici i runner GitHub standard sono gratuiti e senza limite di minuti, **macOS e Windows compresi** ("free and unlimited on public repositories"). Confermata la scelta del repo pubblico.
- **Eccezione:** i *larger runners* si pagano sempre, anche sui repo pubblici (e richiedono piani Team/Enterprise). Non li usiamo: solo label standard.
- **Per confronto (repo privato, piano Free):** 2.000 minuti/mese inclusi; tariffe per minuto Linux 2-core $0,006, Windows $0,010, macOS $0,062 (circa 10× Linux). Superata la quota senza metodo di pagamento, i job si bloccano.
- **Limiti che valgono anche per noi:**
  - 6 ore massime per job; 35 giorni per run.
  - Piano Free: 20 job concorrenti, **massimo 5 job macOS** in parallelo.
  - Storage artefatti 500 MB e cache 10 GB per repository (la doc dice che il supporto non li alza). Gli artefatti di build vanno tenuti con retention breve (pochi giorni) e la distribuzione passa dalle **GitHub Release**.
- **Runner standard per repo pubblici:**
  - `macos-latest` = macOS 26 arm64 (M1, 3 CPU, 7 GB RAM, 14 GB SSD); anche `macos-14`, `macos-15`, `macos-26`. Intel solo con `macos-15-intel` / `macos-26-intel`.
  - `windows-latest` = Windows Server 2025 x64 (4 CPU, 16 GB RAM); anche `windows-2022`, `windows-11-arm`.
  - `ubuntu-latest` = 24.04 x64 (4 CPU, 16 GB RAM).
- **Release:** fino a 1.000 file per release, ogni file sotto 2 GiB, nessun limite di dimensione totale né di banda. Un .dmg con FFmpeg incluso ci sta ampiamente.
- **Conseguenze per la CI:**
  - Sui branch di lavoro solo test, su Linux dove possibile (più veloce, non occupa gli slot macOS).
  - Build .dmg (arm64, `macos-latest`) e .msi (`windows-latest`) solo su push a `main` e sui tag di release.
  - Il .dmg sarà solo Apple Silicon finché non serve Intel.
  - Il runner macOS ha solo 7 GB di RAM: attenzione alle build di whisper.cpp/llama.cpp nelle milestone successive.
