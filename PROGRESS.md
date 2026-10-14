# FrameNote — Avanzamento

## Stato attuale
- **Milestone in corso:** M1 — spike zoom: implementato, in attesa della prova di Domenico sulla build `v0.1.0`
- **In parallelo:** layer "Sicurezza e privacy" — piano `PIANO_SICUREZZA.md` in attesa di approvazione
- **Ultima milestone completata:** M0 ✅ (2026-10-13)
- **Ultima build provata da Domenico:** `v0.0.1` (commit `b91e98a`), macOS 26.6.2, MacBook M1 Pro

## 2026-10-14 — Release v0.1.0 e piano di sicurezza
**Fatto**
- Release `v0.1.0` pubblicata (run Build #6, commit `17c2f62`): `.dmg`, `.msi`, `SHA256SUMS.txt`. Requisito designato
  identico a `v0.0.1` (`certificate leaf = H"21823567a97f4321ad26c27b50832eeb3c91ef2d"`), anche per il sidecar FFmpeg.
- Richiesta di Domenico: un layer di sicurezza perché i dati non escano se non necessario. Analisi delle vie d'uscita
  e piano in `PIANO_SICUREZZA.md` (fase A subito, fase B come regole per le milestone successive).

**In attesa di Domenico**
1. Prova di M1 sulla `v0.1.0` (passi in chat).
2. Approvazione di `PIANO_SICUREZZA.md` e decisioni D1 (cartella progetti) e D2 (esclusioni dalla registrazione).

## 2026-10-13 — Sessione 1, terza parte: M1 (spike zoom)
**Fatto**
- Piano `PIANO_M1.md` approvato. Richiesta di Domenico sulla webcam stile Loom (interruttore, grandezze, colori, animazioni) messa in specifica per M2/M3 (`DECISIONS.md`).
- Cattura macOS (`capture/macos.rs`): ScreenCaptureKit sullo schermo principale a risoluzione nativa, 30 fps, NV12, cursore escluso, finestre di FrameNote escluse; frame via pipe al sidecar FFmpeg (`h264_videotoolbox`) in MP4 frammentato; ritmo costante con `Pacer`; contatori di frame/drop e CPU.
- Mouse (`input_events/macos.rs`): posizione e pulsanti a 120 Hz con CoreGraphics, **senza permessi aggiuntivi**; `events.jsonl` nel formato §4 con lo stesso orologio dei frame.
- Zoom (`render/zoom.rs`): raggruppamento dei clic, isteresi, durata minima, livello 1,5–2×, inquadratura sempre dentro il video, molle critiche deterministiche, telecamera che segue il cursore fuori dalla zona morta.
- Cursore (`render/cursor.rs`, `render/sprite.rs`): filtro One Euro, ingrandimento 1,5×, dissolvenza dopo 2 s fermo, anello animato sul clic, freccia disegnata in software.
- Renderer (`render/gpu.rs`, `shaders/compose.wgsl`): compositore wgpu, NV12→RGB BT.709, bicubico in ingrandimento e media a griglia in riduzione; export (`render/export.rs`) FFmpeg → GPU → FFmpeg con avanzamento e annullamento.
- Progetto (`project/`): cartella `AAAA-MM-GG_HHMM_prova-zoom/`, `project.json` v1 con blocchi zoom, impostazioni cursore, report della registrazione ed export.
- Schermata "Prova zoom (M1)": permesso, registrazione con timer e clic, zoom calcolati, report, export con avanzamento, apri/mostra il video.
- macOS minimo portato a 13.0; versione 0.1.0. CI: renderer e pipeline di export provati su GPU software (lavapipe) in Linux; clippy anche su macOS per il codice di cattura.

**Verificato (ambiente cloud e CI)**
- 56 test Rust (logica, pixel del renderer su GPU software, export completo con FFmpeg reale) e 16 test frontend verdi; clippy pulito su Linux.
- Prova visiva di export da una "schermata" sintetica 3024×1964: testo nitido a 1× e 2×, cursore e anello corretti.
- Il codice di cattura macOS compila in CI (macOS runner).

**Non ancora verificato**
- La cattura reale (permesso, ScreenCaptureKit, mouse) si può provare solo sul Mac di Domenico.

**Prossimo passo**
- Release `v0.1.0`, prova di Domenico: video di 30 s con 5 clic, giudizio su fluidità degli zoom, cursore e nitidezza; report nella diagnostica.

## 2026-10-13 — Chiusura di M0: prova di Domenico
**Verificato da Domenico su macOS 26.6.2 (M1 Pro, 16 GB)**
- Release `v0.0.1` scaricata, installata da `.dmg`, aperta dopo "Open Anyway" in Privacy & Security (Gatekeeper, come previsto dal README).
- "Info e diagnostica": encoder preferito `h264_videotoolbox` **hardware**; prova di codifica reale riuscita per `h264_videotoolbox` (251 ms) e `hevc_videotoolbox` (198 ms); GPU **Apple M1 Pro (Metal, integrata)**; FFmpeg 8.1.2 LGPL; database ok (schema v1); FTS5 disponibile.
- **Criterio di uscita di M0 soddisfatto:** l'app si apre e la diagnostica mostra encoder e GPU rilevati.

**Verificato in CI (run Build #3, release `v0.0.1`)**
- `.dmg` firmato con il certificato stabile. Requisito designato da confrontare nelle build future (deve restare identico, altrimenti macOS ritira i permessi):
  `identifier "com.mimmo8bit.framenote" and certificate leaf = H"21823567a97f4321ad26c27b50832eeb3c91ef2d"`
- `.msi` Windows prodotto; non ancora provato su una macchina Windows (previsto in M9).

**Note per le prossime milestone**
- Su macOS in inglese i pulsanti sono "Done" e "Open Anyway"; il README li riporta in italiano: da allineare quando si aggiorna il README.
- Il sistema compare come "Darwin macOS 26.6.2 Tahoe": dettaglio estetico della diagnostica.
- "Esporta diagnostica" verificato da Domenico: lo zip contiene solo `info.json` e il log, percorsi anonimizzati, nessun errore nel log. Spazio libero sul Mac ≈ 30 GB, RAM 16 GB (≈ 6 GB liberi durante la prova).

## 2026-10-13 — Sessione 1, seconda parte: CI reale
**Fatto e verificato su GitHub Actions**
- Accesso in scrittura sistemato; PR mimmo8bit/FRAME.NOTE#1 unita su `main`.
- `ci.yml` verde (Linux + Rust su macOS). `build.yml` su `main` verde: FFmpeg 8.1.2 LGPL compilato per macOS arm64 (con `h264_videotoolbox`/`hevc_videotoolbox`) e preparato per Windows, pubblicato nella release `ffmpeg-8.1.2-lgpl-1`; `.msi` e `.dmg` prodotti.
- Certificato di firma creato da Domenico con `signing-bootstrap.yml` (secret `MACOS_*` presenti, token temporaneo eliminato).
- La build di `main` è firmata ad-hoc: era partita prima che esistesse il certificato (i secret si leggono all'avvio della run).

**Problemi aperti**
- Il push del tag `v0.0.1` dalla sessione cloud è rifiutato (403). Aggiunta l'opzione "Pubblica" a `build.yml` (vedi `DECISIONS.md`).

**Risolto**
- PR mimmo8bit/FRAME.NOTE#2 unita; release `v0.0.1` pubblicata con Build avviato a mano ("Pubblica").

## 2026-10-13 — Sessione 1 (cloud): implementazione di M0
**Fatto**
- Scaffolding Tauri 2 + React 19 + TypeScript + Vite + Tailwind 4. Identificatore `com.mimmo8bit.framenote`, versione `0.0.1`, icona.
- Backend Rust:
  - `paths.rs`: cartelle `Documenti/FrameNote/Progetti` e cartella dati (`logs/`, `models/`, `framenote.sqlite`), create al primo avvio; anonimizzazione della home (`~`).
  - `library/db.rs`: SQLite incluso, migrazioni con `PRAGMA user_version`, schema v1 (`meta`, `settings`), controllo FTS5.
  - `encoder/ffmpeg.rs`: ricerca del sidecar accanto all'eseguibile, lettura di `-version`/`-encoders`, **codifica di prova reale** (0,2 s, timeout 20 s) per ogni encoder H.264/HEVC candidato; scelta del preferito (hardware prima del software).
  - `diagnostics/`: log giornalieri (max 7 file) con panic hook e log del frontend; raccolta info (versione, commit, OS, CPU, RAM, GPU via wgpu, FFmpeg, encoder, database, spazio libero, modelli); export `.zip` (info.json, log anonimizzati, `project.json` dell'ultimo progetto, mai video).
- Frontend: Home e "Info e diagnostica" con Aggiorna, **Esporta diagnostica** (finestra di salvataggio) e **Mostra nella cartella**; errori in italiano; Esc per tornare indietro.
- CI (`.github/workflows/`): `ci.yml` (test su branch), `build.yml` (.dmg/.msi su `main`, tag e avvio manuale; Release sui tag `v*`), `ffmpeg.yml` (release dei sidecar), `signing-bootstrap.yml` (certificato di firma).
- Script: `build-ffmpeg.sh`, `fetch-ffmpeg.sh`, `macos-sign.sh`, `macos-dmg.sh`, `check-version.sh`; versione/checksum FFmpeg in `scripts/ffmpeg.env`.
- README con download, installazione (Gatekeeper/SmartScreen), diagnostica, licenze, istruzioni per il certificato. `THIRD_PARTY_NOTICES.md` incluso nel pacchetto.
- Decisioni nuove in `DECISIONS.md`: FFmpeg 8.1.2 (non 8.0), firma con script proprio, sidecar senza `tauri-plugin-shell`, cartella dati locale, build manuale dei branch.

**Verificato (nell'ambiente cloud Linux, non sul Mac)**
- `cargo fmt --check`, `cargo clippy -D warnings`: puliti. `cargo test`: 22 test verdi (percorsi, migrazione idempotente, FTS5, parsing encoder su output reali, prova di codifica reale con FFmpeg, contenuto dello zip, log, modelli).
- `tsc`, Vitest (10 test: formattazione, schermata diagnostica, flusso di esportazione), `vite build`: verdi.
- App avviata davvero sotto display virtuale: si apre, crea cartelle, database e log; la schermata di diagnostica mostra dati reali (FFmpeg 8.1.2 LGPL, DB schema v1, FTS5, spazio libero, "nessun modello installato").
- `scripts/build-ffmpeg.sh` eseguito su Linux: scarica i sorgenti, verifica lo SHA256, compila FFmpeg 8.1.2 LGPL statico e controlla dipendenze e licenza. La firma GPG del tarball è stata verificata a mano.
- Workflow controllati con `actionlint`, script con `shellcheck`.

**Non ancora verificato**
- Nessun workflow è ancora girato su GitHub: compilazione FFmpeg su macOS, firma, .dmg, .msi, release.
- Nulla è stato provato su macOS o Windows reali.

**Problemi aperti**
- **Push su GitHub rifiutato (403):** l'app GitHub di Claude non ha il permesso di scrittura su `mimmo8bit/FRAME.NOTE`, quindi i commit di questa sessione sono solo nel container cloud. Va sistemato l'accesso, poi si fa il push del branch `claude/loving-wozniak-x60g5b`.

**In attesa di Domenico**
1. Dare a Claude l'accesso in scrittura al repository (vedi messaggio in chat).
2. Dopo il push e l'unione su `main`: creare il certificato di firma (README, "Certificato di firma macOS").
3. Provare la build `v0.0.1` (passo D del piano).

**Prossimo passo**
- Push, CI verde su `ci.yml`, poi `build.yml` (anche avviandolo a mano sul branch) fino a .dmg e .msi; tag `v0.0.1`; prova di Domenico.

## 2026-10-13 — Sessione 0 (setup)
**Fatto**
- Letta la specifica. Decisi OS primario (macOS), luogo di sviluppo (Claude Code cloud) e visibilità del repo (pubblico): vedi `DECISIONS.md`.
- Creato il repository pubblico `mimmo8bit/FRAME.NOTE` e caricati `FRAMENOTE_SPEC.md`, `PROGRESS.md` e `DECISIONS.md`.
- Verificati sulla documentazione ufficiale GitHub i limiti attuali di GitHub Actions per i repo pubblici: runner standard macOS/Windows/Linux gratuiti e senza limite di minuti; massimo 5 job macOS in parallelo; 500 MB di storage artefatti; release con file fino a 2 GiB. Dettagli e conseguenze per la CI in `DECISIONS.md`.
- Scritto il piano di M0 (`PIANO_M0.md`): CI, firma self-signed stabile, scaffolding Tauri 2 + React, FFmpeg LGPL come sidecar, SQLite, cartelle, log, diagnostica, README.
- **Piano di M0 approvato da Domenico.**

**Verificato**
- Nulla di eseguibile: nessun codice scritto finora.
