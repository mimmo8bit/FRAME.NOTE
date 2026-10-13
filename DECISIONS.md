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
