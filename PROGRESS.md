# FrameNote — Avanzamento

## Stato attuale
- **Milestone in corso:** M1 — spike zoom: correzione degli zoom pronta in `v0.1.1`, da riprovare
- **In parallelo:** layer "Sicurezza e privacy" fase A implementato in `v0.1.1` (`PIANO_SICUREZZA.md`)
- **Ultima milestone completata:** M0 ✅ (2026-10-13)
- **Ultima build provata da Domenico:** `v0.1.0` (commit `17c2f62`), macOS 26.6.2, MacBook Pro M1 Pro 16"

## 2026-10-15 — v0.1.1: correzione degli zoom e sicurezza (fase A)
**Fatto** (approvato da Domenico: "ok zoom, ok sicurezza D1 sì D2 sì")
- Zoom: ritorno a schermo intero dopo N s senza clic (predefinito 2 s), clic veloci e sparsi senza zoom, zona di lavoro
  che prevale sui clic isolati; **Ricalcola zoom** e **Riapri l'ultima registrazione** in Prova zoom.
- Sicurezza (dettagli in `DECISIONS.md`): progetti in `~/FrameNote/Progetti` con spostamento automatico da Documenti,
  cartelle e file privati, notifiche e password manager esclusi dalla cattura, FFmpeg limitato a file e pipe,
  navigazione e CSP chiuse, schermata **Privacy e sicurezza** (rete, FileVault, sincronizzazione cloud, altri utenti,
  firma, esclusioni), diagnostica anonima con elenco dei file prima di salvare.
- CI: guardiano della rete, controllo del repository, `cargo audit`, `npm audit`, test macOS con il sidecar vero, avvio
  di prova dell'app firmata, hash dei sidecar fissati. Versione 0.1.1.

**Verificato (ambiente cloud)**
- 75 test Rust (Linux, con FFmpeg reale: il test ha trovato il protocollo `fd` necessario a FFmpeg per le pipe) e 20
  test frontend verdi; clippy e fmt puliti su Linux; guardiano della rete verde per macOS, Windows e Linux;
  `npm audit` senza vulnerabilità.
- Zoom: 7 test nuovi, tra cui lo scenario della prova di Domenico (22 clic sparsi in 34 s → nessuno zoom).

**Verificato in CI (branch, commit `c7d2b21`: Test #16 e Build #8 verdi)**
- macOS: clippy del codice di cattura con le esclusioni; tutti i test con il sidecar FFmpeg 8.1.2 vero, compresi
  l'export completo e il rifiuto degli indirizzi di rete (il sidecar non ha proprio i protocolli di rete).
- App firmata con Hardened Runtime (`flags=0x10000(runtime)`) avviata davvero sul runner: `FRAMENOTE_SMOKE_OK`.
- Requisito designato identico a `v0.0.1`/`v0.1.0` (`certificate leaf = H"21823567a97f4321ad26c27b50832eeb3c91ef2d"`).
- `.dmg` e `.msi` prodotti; guardiano della rete, `cargo audit`, `npm audit`, controllo del repository verdi.

**Non ancora verificato**
- Comportamento reale sul Mac: esclusione di notifiche e password manager, FileVault, spostamento dei progetti.

**Prossimo passo**
- Unione su `main` e release `v0.1.1`; prova di Domenico: Ricalcola sulla registrazione di prova, nuova registrazione
  con notifica, schermata Privacy e sicurezza, diagnostica.

## 2026-10-15 — Feedback di Domenico sugli zoom e nuove richieste
**Feedback su M1**
- La ripresa deve tornare a schermo intero (1×) quando non clicca per un po'; utile un riquadro tratteggiato che mostri cosa
  finirà nel video. Causa individuata nel motore (fusione dei clic ravvicinati, livello minimo 1,5×): proposta di correzione
  per `v0.1.1` e riquadro in M2 (`DECISIONS.md`).

**Diagnostica della prova di M1 (v0.1.0, macOS 26.6.2)**
- Registrazione di 34 s a **3456×2234** (lo schermo di Domenico è da 16", non da 14" come ipotizzato in `PIANO_M1.md`), 30 fps,
  `h264_videotoolbox`: 1021 frame scritti, **2 persi** su 987 ricevuti, 2522 eventi del mouse, 22 clic.
- CPU del processo di cattura 24,6% di un core in media (2,5% del totale). Export 1920×1242 in 13,8 s (2,5× il tempo reale) su
  Apple M1 Pro (Metal). Nessun errore nei log; il primo avvio ha chiesto il permesso e l'app è stata riaperta, come previsto.
- Zoom: 22 clic → un blocco di 25 s a 1,5× (20 clic) + uno finale: conferma la causa del mancato ritorno a 1×.

**Messo in coda** (`DECISIONS.md`, `FRAMENOTE_SPEC.md` §5.1, §5.2, §5.5, §7)
- Icona nella barra dei menu con menu di controllo rapido (M2; bolla M3; trascrizione M4).
- Trascrizione on/off (M4); trascrizione in diretta in valutazione, stima dell'impatto data a Domenico.

**In attesa di Domenico**
1. Ok alla correzione degli zoom per `v0.1.1`.
2. Approvazione di `PIANO_SICUREZZA.md` (D1, D2).
3. Diagnostica della prova di M1 (contiene i blocchi zoom calcolati).

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
