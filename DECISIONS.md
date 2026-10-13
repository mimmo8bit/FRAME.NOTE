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

## 2026-10-13 — Identificatore app: `com.mimmo8bit.framenote`
- **Decisione:** l'identificatore Tauri/macOS è `com.mimmo8bit.framenote` e **non cambierà mai più**.
- **Motivo:** da lui dipendono i permessi di macOS (TCC, insieme alla firma) e la cartella dati dell'app.

## 2026-10-13 — FFmpeg 8.1.2 invece di 8.0
- **Decisione:** il sidecar è FFmpeg **8.1.2** (LGPL), non 8.0 come nel piano. Release dei binari: `ffmpeg-8.1.2-lgpl-1` (versione e suffisso in `scripts/ffmpeg.env`).
- **Motivo:** è la serie stabile corrente; BtbN/FFmpeg-Builds pubblica le build Windows solo per le serie recenti, quindi tenere Mac e Windows sulla stessa serie è più semplice con la 8.1.
- **Verifica dei sorgenti:** SHA256 di `ffmpeg-8.1.2.tar.xz` (`464beb5e…d97b524c`) fissato nel repo dopo aver verificato la firma GPG con la chiave di rilascio FFmpeg `FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8`. La CI scarica da ffmpeg.org e, se non raggiungibile, dal mirror Ubuntu (stesso file): conta solo lo SHA256.
- **macOS:** compilato in CI con `--disable-autodetect` (così non si aggancia a librerie di Homebrew del runner), `--disable-gpl --disable-nonfree --disable-network`, VideoToolbox/AudioToolbox, zlib di sistema, statico. Lo script fallisce se il binario dipende da librerie fuori da `/usr/lib` o `/System`. Configurazione provata con successo su Linux (build 8.1.2 LGPL funzionante).
- **Windows:** build `win64-lgpl` serie 8.1 di BtbN, verificata con il loro `checksums.sha256` al momento della copia; da lì in poi fa fede la copia nella nostra release con il nostro `SHA256SUMS.txt`.
- **Licenza:** sorgenti, `COPYING.LGPLv2.1` e configurazione esatta allegati alla release; `THIRD_PARTY_NOTICES.md` incluso nel pacchetto.

## 2026-10-13 — Firma macOS con script proprio, .dmg con `hdiutil`
- **Decisione:** niente `tauri-action`. Tauri produce solo `FrameNote.app`; `scripts/macos-sign.sh` firma sidecar e app con `codesign --options runtime` usando l'identità per hash SHA-1 (funziona anche con un certificato non "trusted"); `scripts/macos-dmg.sh` crea il .dmg con `hdiutil`.
- **Motivo:** è il rischio previsto nel piano (`tauri-action`/bundler potrebbero rifiutare un'identità non fidata). Con uno script nostro il passo è prevedibile e il log stampa il requisito designato da confrontare tra le build.
- **Certificato:** generato da `signing-bootstrap.yml` su runner Linux con OpenSSL (RSA 2048, 20 anni, EKU codeSigning), .p12 cifrato 3DES/SHA1 perché `security import` di macOS lo legga sempre. Il workflow non sovrascrive mai secret esistenti.
- **Senza certificato:** le build di `main` vengono firmate ad-hoc con un avviso; le release `v*` falliscono.

## 2026-10-13 — Sidecar avviato con `std::process`, senza `tauri-plugin-shell`
- **Decisione:** FFmpeg si avvia dal backend Rust con `std::process::Command`, cercandolo accanto all'eseguibile (dove Tauri mette gli `externalBin`). Niente `tauri-plugin-shell`.
- **Motivo:** il frontend non ha bisogno di lanciare processi; un plugin e un permesso in meno. Le codifiche di prova hanno timeout e su Windows non aprono finestre di console.
- **Dettaglio:** `externalBin` è dichiarato solo in `tauri.macos.conf.json` e `tauri.windows.conf.json`; i test Rust su macOS lo azzerano con `TAURI_CONFIG` perché non serve il binario.

## 2026-10-13 — Cartella dati: "local data" dell'app
- **Decisione:** database, log e modelli stanno nella cartella dati *locale* di Tauri: su macOS `~/Library/Application Support/com.mimmo8bit.framenote/`, su Windows `%LOCALAPPDATA%\com.mimmo8bit.framenote\` (non `Roaming`).
- **Motivo:** i modelli AI pesano GB e non devono finire in profili Windows sincronizzati.

## 2026-10-13 — Build manuale di un branch
- **Decisione:** `build.yml` si può avviare anche a mano (`workflow_dispatch`) su un branch, oltre che su push a `main` e sui tag.
- **Motivo:** permette di produrre un pacchetto di prova senza unire su `main`. Le release si creano comunque solo dai tag `v*`.
