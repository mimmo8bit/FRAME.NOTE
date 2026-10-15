# FrameNote — Specifica di sviluppo per Claude Code (v2: zero-install, solo PC)

> Questo file è la fonte di verità del progetto. Leggilo tutto prima di scrivere codice.
> Rileggilo all'inizio di ogni sessione insieme a `PROGRESS.md` e `DECISIONS.md`.

## 0. Contesto e vincoli assoluti

FrameNote è un'app desktop per **uso personale di un solo utente** (Domenico, freelance Meta Ads, lavora in italiano con clienti in remoto). Unisce:

1. Registratore di schermo con montaggio automatico (stile Screen Studio)
2. Video pronti da inviare ai clienti come file (stile Loom, ma senza cloud)
3. Trascrizione e note AI delle call senza bot (stile Fathom/Granola)

**Vincoli non negoziabili:**

- **Zero-install.** Sul PC di Domenico non va installato nulla oltre al pacchetto di FrameNote (.dmg / .msi). Niente Ollama, FFmpeg di sistema, Python, Rust, Node, CMake, wrangler, driver o runtime aggiuntivi. Tutto ciò che serve è **incluso nell'app** (binari, librerie) oppure **scaricato dall'app stessa** al primo avvio (solo modelli AI, nella cartella dati dell'app). Claude Code non deve installare toolchain o programmi sul PC di Domenico.
- **Solo PC, nessun cloud.** Registrazioni, montaggio, trascrizione, AI e libreria restano sul computer. Nessun upload, nessun servizio esterno, nessun account. Unica eccezione: l'invio a Notion, solo se Domenico lo attiva e solo su richiesta.
- **Costo zero.** Nessun servizio o API a pagamento.
- **Single-user.** Niente account, login, piani, billing, licenze, team, filigrane, badge, referral.
- **Italiano prima lingua** per interfaccia, trascrizione, riassunti, lista intercalari.
- **Funzionante davvero.** Niente mock, dati finti, `TODO` o funzioni vuote nel codice consegnato. Se una cosa non è finita, non dichiararla finita.
- **Cross-platform** (macOS + Windows). All'inizio chiedi a Domenico quale sistema usa: quello è l'OS primario, su cui si fanno i test manuali. L'altro resta compilato in CI e viene rifinito in M9.

## 1. Come si sviluppa senza installare nulla sul PC

Poiché sul PC di Domenico non ci sono toolchain, la catena è questa:

1. Claude Code scrive il codice in un repository **GitHub** (privato o pubblico, a scelta di Domenico).
2. **GitHub Actions** compila e testa su runner macOS e Windows a ogni push: `cargo test`, `npm test`, build Tauri, produzione dei pacchetti `.dmg` e `.msi` come artefatti scaricabili. Per le versioni "da provare" crea una **GitHub Release** con i pacchetti allegati.
3. Domenico scarica il pacchetto, lo installa, esegue la verifica manuale descritta in `PROGRESS.md` e riporta l'esito.
4. Claude Code può compilare e testare la logica pura nel proprio ambiente (non sul PC di Domenico) e in CI.

Conseguenze da gestire fin da M0:

- **Diagnostica integrata:** log su file ruotati nella cartella dati, schermata "Info e diagnostica" con versione, OS, encoder rilevato, GPU, stato modelli, e pulsante **"Esporta diagnostica"** che crea uno .zip (log + `project.json` dell'ultimo progetto, mai i video) da allegare in chat. È l'unico modo con cui Claude Code "vede" cosa succede sulla macchina reale.
- **Modalità auto-test:** comando nell'app (menu nascosto o scorciatoia) che esegue i controlli dei criteri di accettazione realizzabili in automatico (es. registra 30 s, misura drop di frame, desync audio, CPU media) e salva un report leggibile da incollare in chat.
- **Firma stabile su macOS:** i permessi di sistema (Registrazione schermo, Microfono, Fotocamera, Accessibilità) sono legati alla firma. Per non doverli riconcedere a ogni build, usa un **certificato self-signed fisso** salvato come secret di GitHub Actions e firma sempre con quello. Spiega a Domenico come aprire la prima volta un'app non notarizzata (Gatekeeper) e, su Windows, come superare SmartScreen.
- **Limiti CI:** controlla i limiti gratuiti attuali di GitHub Actions per il tipo di repository scelto (i runner macOS consumano minuti più in fretta). Compila macOS e Windows solo su push al branch principale e sulle release; sui branch di lavoro esegui solo i test.

## 2. Stack tecnico (tutto incluso nell'app)

| Livello | Tecnologia | Come arriva sul PC |
|---|---|---|
| Guscio app | **Tauri 2** (Rust) + **React + TypeScript + Vite + Tailwind** | Pacchetto |
| Cattura schermo | `scap` o binding diretti: **ScreenCaptureKit** (macOS 12.3+) / **Windows.Graphics.Capture** (`windows-capture`) | Compilato nel binario |
| Audio | `cpal` per microfono; sistema via **ScreenCaptureKit audio** (macOS) e **WASAPI loopback** (Windows) | Compilato |
| Webcam | `nokhwa` | Compilato |
| Input globali | `rdev` o `device_query`, salvati come eventi JSON | Compilato |
| Codifica | **FFmpeg come sidecar incluso nel pacchetto** (build LGPL statica), encoder hardware `h264/hevc_videotoolbox` (Mac), `h264_nvenc/qsv/amf` (Win), fallback `libx264` o `libopenh264` secondo la licenza della build | Pacchetto |
| Renderer | Rust + **`wgpu`**, unico per anteprima ed export | Compilato |
| Trascrizione | **whisper.cpp** via `whisper-rs`, `large-v3-turbo` q5, fallback `small`; Metal su Mac, Vulkan su Win (niente CUDA: richiederebbe driver/toolkit extra) | Libreria compilata; modello scaricato dall'app |
| Diarizzazione | **sherpa-onnx** (segmentazione pyannote + embedding speaker) | Compilato; modelli scaricati dall'app |
| Pulizia audio | `nnnoiseless` + filtro `loudnorm` del FFmpeg incluso | Compilato |
| Segmentazione webcam | modello ONNX selfie-segmentation via `ort`, con onnxruntime **incluso nel pacchetto** | Pacchetto + modello scaricato |
| LLM riassunti | **llama.cpp integrato** (crate `llama-cpp-2` o equivalente), modello GGUF `Qwen2.5-7B-Instruct` Q4_K_M di default, fallback 3B su PC con poca RAM; Metal/Vulkan | Compilato; modello scaricato dall'app |
| Embedding | `bge-m3` GGUF via lo stesso llama.cpp + **`sqlite-vec`** | Compilato; modello scaricato |
| Database | **SQLite** (`rusqlite`, bundled) con **FTS5** | Compilato |
| Aggiornamenti app | Updater di Tauri che legge le **GitHub Releases** (opzionale, disattivabile) | Compilato |
| Test | `cargo test`, Vitest, Playwright dove possibile, tutti in CI | Solo CI |

Se una libreria non funziona, è abbandonata o richiederebbe un'installazione separata, scegli un'alternativa equivalente, gratuita e includibile nel pacchetto, e annota la decisione in `DECISIONS.md`.

### Download modelli al primo avvio
- Schermata guidata che mostra cosa viene scaricato, peso di ciascun modello e spazio libero su disco. Domenico può scegliere "Leggero" (whisper small + LLM 3B) o "Completo" (large-v3-turbo + 7B).
- Download riprendibile, verifica checksum, sorgente pubblica gratuita (es. Hugging Face). Nessuna altra connessione di rete da parte dell'app.
- Rilevamento RAM e GPU per suggerire il preset giusto.
- Possibilità di caricare un modello GGUF già scaricato da file.

## 3. Struttura del repository

```
framenote/
  FRAMENOTE_SPEC.md
  PROGRESS.md
  DECISIONS.md
  README.md              ← come scaricare e installare le build, come provarle
  .github/workflows/     ← test, build .dmg/.msi, release
  src-tauri/
    binaries/            ← sidecar FFmpeg per piattaforma (scaricati dallo script CI, in .gitignore)
    src/
      capture/           ← trait CaptureBackend + macos.rs / windows.rs
      audio/
      input_events/
      encoder/
      recovery/
      render/            ← compositor wgpu, zoom engine, cursore
      polish/            ← silenzi, intercalari, audio
      transcribe/
      diarize/
      ai/                ← llama.cpp, template, RAG
      models/            ← download, checksum, gestione modelli
      library/           ← SQLite, FTS, embedding
      deliver/           ← export per invio, pacchetto offline
      integrations/      ← Notion, ICS calendario
      diagnostics/       ← log, auto-test, export diagnostica
      commands.rs
  src/
    windows/             ← barra REC, bolla, "Pronto", editor, libreria, impostazioni, note call, diagnostica
```

## 4. Dove e come si salvano le registrazioni

- Cartella principale di default: `Documenti/FrameNote/` (macOS `~/Documents/FrameNote/`, Windows `%USERPROFILE%\Documents\FrameNote\`). Modificabile in Impostazioni, anche su disco esterno; spostamento dei progetti esistenti con conferma.
- Modelli AI e database in una cartella dati separata dell'app (non mescolati ai progetti).
- Controllo spazio libero prima di registrare e durante: avviso sotto 5 GB, stop pulito con salvataggio sotto 1 GB.
- Pulizia guidata: per progetti vecchi, opzione "Tieni solo l'export finale" che elimina le tracce grezze con conferma esplicita.

Ogni registrazione è una cartella `Progetti/AAAA-MM-GG_HHMM_titolo/`:

```
project.json        ← metadati, versione schema, impostazioni di montaggio
screen/             ← segmenti fMP4 da 2 s + init.mp4
webcam/
mic/                ← segmenti audio
system_audio/
events.jsonl        ← {t_ms, type: move|click|key|scroll|window_focus, x, y, button, display}
transcript.json     ← parole con start/end ms, speaker, confidenza
summary.json        ← riassunto, capitoli, decisioni, azioni, con ms sorgente
notes.md            ← note manuali durante la call
exports/            ← MP4, SRT, DOCX/PDF/MD, pacchetti per invio
```

Montaggio **non distruttivo**: le tracce originali non si riscrivono mai, `project.json` descrive come comporle.

## 5. Moduli funzionali e criteri di accettazione

Ogni modulo è "finito" solo quando i criteri sono verificati su una build installata sul PC di Domenico (o dal report di auto-test) e annotati in `PROGRESS.md`.

### 5.1 Registrazione
- Sorgenti: schermo intero, finestra, area, singola app. Multi-monitor.
- Webcam, microfono, audio di sistema: attivabili singolarmente, su tracce separate.
- **Fotocamera stile Loom:** prima di registrare, interruttore "Mostra fotocamera" con anteprima live e scelta della fotocamera; la webcam va sempre su traccia separata, quindi si può togliere anche dopo (richiesta di Domenico, 2026-10-13).
- **Anteprima live obbligatoria** della sorgente nella barra REC prima di avviare.
- Indicatore livello microfono in tempo reale.
- Conto alla rovescia 3 s, pausa/ripresa, stop, annulla. Scorciatoie globali configurabili (default `Alt+Shift+R` / `⌥⇧R` avvio-stop, `Alt+Shift+P` pausa).
- Fino a 4K/60fps con encoder hardware; CPU media < 25% in 1080p30 su macchina moderna.
- **Anti-perdita:** segmenti da 2 s con fsync. Al riavvio dopo crash, rileva progetti incompleti e propone il recupero. Criterio: kill del processo durante la registrazione → video recuperato con perdita ≤ 2 s.
- Esclusione dalla cattura di app scelte e delle finestre di FrameNote, con API native.
- Teleprompter visibile solo a Domenico, escluso dalla cattura.
- Indicatore di registrazione sempre visibile (tray/menu bar + bordo opzionale).
- **Icona nella barra dei menu** con tempo trascorso accanto e aspetto diverso in registrazione e in pausa; menu rapido: avvia/ferma, pausa/riprendi, annulla, microfono, audio di sistema, webcam (con scorciatoia per forma e colore della bolla), sorgente, trascrizione on/off, ultima registrazione, Info e diagnostica, Esci (richiesta di Domenico, 2026-10-15; M2, bolla M3, trascrizione M4).
- **Riquadro tratteggiato** visibile solo a Domenico, escluso dalla cattura, che mostra l'inquadratura dello zoom durante la registrazione (richiesta di Domenico, 2026-10-15; M2).
- **Modalità call:** rileva Zoom, Teams, Webex, Slack, browser con microfono attivo; propone "Solo audio + note" o "Audio + schermo".

### 5.2 Auto-Polish (automatico allo stop)
- **Zoom automatico** su clic e zone di digitazione da `events.jsonl`: raggruppa clic vicini, zoom 1.5–2.5×, transizioni morbide (spring/ease), durata minima e isteresi contro gli zoom nervosi. Ogni zoom è un blocco modificabile. Dopo N secondi senza clic (regolabile, predefinito 2 s) la ripresa torna sempre a schermo intero (richiesta di Domenico, 2026-10-15).
- **Cursore:** ingrandimento, smussatura, evidenziazione clic, nascosto dopo N secondi da fermo.
- **Taglio silenzi** (soglia e durata configurabili) e **intercalari** dalla trascrizione; lista italiana di default: "ehm", "ehmm", "uhm", "cioè", "allora", "diciamo", "praticamente", "tipo", "insomma", "vabbè". Tagli reversibili.
- **Audio:** riduzione rumore e normalizzazione a -16 LUFS, separate per mic e sistema.
- **Webcam:** bolla spostabile e ridimensionabile, sfondo sfocato o rimosso, si sposta se copre lo zoom attivo.
  - Grandezze preimpostate (Piccola, Media, Grande) più ridimensionamento libero; forme cerchio, quadrato arrotondato, rettangolo verticale; posizione negli angoli o libera.
  - Bordo con colori preimpostati, personalizzati o del brand kit, spessore regolabile.
  - Animazioni a scelta, spente di default e con intensità regolabile, mai fastidiose: "Respiro" (pulsazione lenta), "Galleggia" (leggero su e giù), "Reagisce alla voce" (lieve vibrazione o pulsazione col volume del microfono), ingresso/uscita con piccolo rimbalzo.
  - Tutto non distruttivo: modificabile dopo la registrazione (richiesta di Domenico, 2026-10-13; base in M2, colori e animazioni in M3).
- **Look:** sfondo, padding, angoli, ombra, cornice; preset salvabili come **brand kit** (logo, colori, font).
- **Sottotitoli** dalla trascrizione, stile personalizzabile, bruciati o in SRT.
- **Reframe social:** 16:9, 9:16, 1:1 seguendo lo zoom attivo.

### 5.3 Editor
- Timeline con tracce: schermo, webcam, mic, sistema, zoom, testi/frecce.
- Taglio, split, velocità, eliminazione segmenti, annulla/ripeti illimitato.
- **Editing dal testo:** cancellare parole nella trascrizione rimuove il segmento; clic su una parola → salto al punto.
- Più take nello stesso progetto, riordinabili.
- Anteprima in tempo reale con il renderer wgpu, identica nel risultato all'export.

### 5.4 Export e invio (senza link)
- **Formati:** MP4 (H.264/HEVC), WebM, GIF, solo audio (M4A/MP3), SRT, trascrizione e riassunto in MD/DOCX/PDF. Export in background con avanzamento.
- **Preset di invio:** "WhatsApp" (MP4 compresso, peso ridotto), "Email" (MP4 sotto un peso massimo configurabile), "Qualità piena". Il peso stimato è mostrato prima dell'export.
- **Pulsante Invia** nella finestra "Pronto": esporta con il preset scelto, poi offre *Copia file negli appunti* (incollabile in WhatsApp Desktop/Web, email, Slack), *Mostra nella cartella*, *Apri email con testo pronto* (titolo + riassunto breve; il file si allega a mano).
- **Pacchetto offline per il cliente** (opzionale): uno .zip con `index.html` autonomo + video MP4: player, capitoli cliccabili, trascrizione cliccabile, riassunto e pulsante CTA configurabile (testo + URL). Si apre con doppio clic in qualsiasi browser, senza internet né installazioni.
- Il cliente non ha bisogno di nulla oltre a un lettore video o un browser.

### 5.5 Trascrizione e AI (tutto locale)
- Trascrizione automatica a fine registrazione, in background, lingua rilevata (default italiano), offline.
- Trascrizione attivabile o disattivabile per la registrazione in corso anche dal menu della barra dei menu. Trascrizione in diretta durante registrazione o call: in valutazione (vedi `DECISIONS.md`, 2026-10-15).
- **Diarizzazione** con etichette; rinomina manuale e memoria della voce tramite embedding salvati.
- **Riassunto a capitoli con fonte:** ogni punto ha il ms di origine, cliccabile. Map-reduce per call lunghe. Prompt di sistema che vieta informazioni non presenti nella trascrizione.
- **Template:** call di vendita, kickoff cliente, report mensile, riunione interna, tutorial, intervista; personalizzabili come file Markdown.
- **Decisioni** e **azioni da fare** (chi, cosa, entro quando se detto).
- **Bozza email di follow-up** in italiano, diretta e umana, modificabile.
- **Titolo, descrizione e capitoli** automatici.
- **"Chiedi alla registrazione":** RAG con bge-m3 + sqlite-vec, risposte con citazione dei minuti.
- **Import file:** vocali WhatsApp (.opus/.ogg), MP3, M4A, MP4, MOV → stesso flusso.
- **Redazione dati sensibili** opzionale negli export: IBAN, carte, codici fiscali italiani, email, telefoni (regex + validazione).
- Pollice su/giù su ogni riassunto, salvato in locale.
- L'elaborazione AI non deve bloccare l'interfaccia; mostra tempo stimato e permette di metterla in pausa durante una nuova registrazione.

### 5.6 Note delle call senza bot
- Cattura audio di sistema + microfono durante qualsiasi app di call, nessun partecipante aggiunto.
- **Promemoria consenso:** al primo uso e a ogni call, banner con messaggio pronto da copiare (IT + EN, modificabile), es. *"Ciao, registro l'audio di questa call solo per prendere appunti, va bene per te?"*. Registro locale di quando è stato mostrato/copiato.
- Finestrella note sempre in primo piano, esclusa dalla cattura, con timestamp automatici.
- A fine call: trascrizione + riassunto che **fonde le note manuali** con la trascrizione, decisioni, azioni, bozza email.
- **Calendario:** URL ICS privato di Google Calendar/Outlook incollato nelle impostazioni (niente OAuth, nessuna app). Notifica 10 minuti prima di un evento con link Meet/Zoom/Teams: "Registrare le note?". Il progetto prende il nome dell'evento e i partecipanti come nomi candidati per i parlanti. È l'unica lettura di rete periodica, attiva solo se l'URL è impostato.

### 5.7 Libreria e ricerca
- Griglia/lista con miniatura, durata, data, tag, stato (trascritto, riassunto, esportato).
- Ricerca full-text nel parlato (FTS5) con salto al minuto; ricerca semantica opzionale.
- Filtri per tipo (video, call, import), tag, cliente.

### 5.8 Integrazioni gratuite (nessuna app richiesta)
- **Notion** con integration token personale: invio riassunto + azioni a una pagina o database. Disattivata di default.
- Azioni esportabili in CSV (compatibile Trello/Asana) ed eventi `.ics` per le scadenze.
- Copia rapida del riassunto in formato WhatsApp/email.

## 6. UX: regole

- **Regola d'oro:** dal clic su REC al file pronto da incollare in WhatsApp o email in meno di 15 secondi per un video di 2 minuti, senza aprire l'editor.
- Barra REC compatta. Dopo lo stop appare "Pronto" con anteprima già montata, titolo suggerito e pulsanti **Invia**, **Ritocca**, **Riregistra**, **Esporta**.
- Primo avvio: permessi chiesti uno alla volta con spiegazione, pulsanti che aprono il pannello di sistema giusto; poi la schermata dei modelli (§2).
- Interfaccia in italiano, testi brevi e umani, tema di sistema.
- Tutto usabile da tastiera. Errori che dicono cosa è successo e cosa fare, mai stack trace (i dettagli vanno nei log).

## 7. Milestone (in ordine, una alla volta)

Ogni milestone termina con: CI verde su macOS e Windows, pacchetto scaricabile dalla Release, verifica manuale (o report auto-test) annotata in `PROGRESS.md`, commit.

| # | Milestone | Criterio di uscita |
|---|---|---|
| **M0** | Repo GitHub, workflow CI (test + build .dmg/.msi + release), firma self-signed stabile su macOS, scaffolding Tauri 2 + React, FFmpeg sidecar incluso, SQLite, cartelle, log + "Esporta diagnostica", README con istruzioni di installazione | Domenico installa la build, l'app si apre, la diagnostica mostra encoder e GPU rilevati |
| **M1** | **Spike zoom:** registrazione schermo + eventi cursore, export MP4 con zoom automatico via wgpu | Video di 30 s con 5 clic, zoom fluidi e cursore smussato. Se la qualità non è buona, fermati e proponi soluzioni |
| **M2** | Registrazione completa: sorgenti, tracce separate, anteprima, segmenti + recovery, scorciatoie, esclusione finestre, barra REC, icona nella barra dei menu, riquadro di anteprima dello zoom, bolla, controllo spazio disco | Test di crash superato; 30 min senza drop né desync > 40 ms (report auto-test) |
| **M3** | Auto-Polish + "Pronto" + export in tutti i formati + preset di invio | REC → MP4 montato e copiabile negli appunti senza aprire l'editor |
| **M4** | Download modelli in-app, trascrizione, diarizzazione, sottotitoli, intercalari, libreria con FTS | Call di 20 min in italiano trascritta offline con parlanti distinti |
| **M5** | AI con llama.cpp integrato: riassunti con fonte, template, decisioni/azioni, email, titolo, import file, "chiedi alla registrazione", redazione | Ogni punto del riassunto rimanda a un minuto verificabile, senza programmi esterni |
| **M6** | Modalità call + note + promemoria consenso + calendario ICS + Notion | Flusso completo: notifica calendario → call → note → riassunto + email |
| **M7** | Editor timeline + editing dal testo + clip multiple + brand kit + reframe 9:16 | Reel 9:16 con sottotitoli e logo esportato in < 5 min di lavoro |
| **M8** | Pacchetto offline per il cliente (.zip con player HTML, capitoli, trascrizione, CTA) | Pacchetto aperto su un altro computer senza internet: tutto funziona |
| **M9** | Rifinitura sull'altro OS | Criteri M2–M4 rispettati anche lì (verifica su macchina disponibile o via auto-test) |
| **M10** | Performance, accessibilità, errori, auto-aggiornamento da GitHub Releases (opzionale), auto-avvio opzionale | Installazione pulita su macchina senza ambiente di sviluppo, aggiornamento a versione successiva senza perdere progetti né permessi |

## 8. Metodo di lavoro per Claude Code

1. A inizio sessione: leggi `FRAMENOTE_SPEC.md`, `PROGRESS.md`, `DECISIONS.md`. Riassumi in 3 righe dove siamo.
2. Per ogni milestone: plan mode, piano (file, crate, rischi), aspetta l'ok, poi implementa.
3. Piccoli passi verificabili. Test frequenti nel tuo ambiente e in CI.
4. Test automatici per la logica pura: calcolo zoom, intercalari, parsing eventi, chunking riassunti, regex dati sensibili, recovery segmenti, checksum modelli.
5. Quando serve un'azione di Domenico (installare una build, concedere permessi, provare a mano, esportare la diagnostica), **fermati e scrivi esattamente cosa fare**, passo per passo, e cosa incollarti in risposta.
6. Mai dichiarare completato qualcosa non eseguito e verificato. I test manuali vanno scritti come "da verificare da Domenico".
7. Aggiorna `PROGRESS.md` a fine sessione: fatto, verificato, problemi aperti, prossimo passo.
8. Commit chiari a ogni passo funzionante.
9. Niente feature fuori specifica senza chiedere. Niente dipendenze a pagamento o da installare a parte, mai.

## 9. Cosa serve a Domenico

- Un account GitHub gratuito (è un sito, non un'app).
- Scaricare e installare il pacchetto FrameNote a ogni versione da provare.
- Al primo avvio: concedere i permessi e scegliere il preset modelli. Spazio libero consigliato: almeno 10 GB per i modelli più quello per le registrazioni.

## 10. Privacy

- Nessun dato lascia il computer. Uniche connessioni di rete: download dei modelli, lettura del calendario ICS se impostato, invio a Notion se attivato, controllo aggiornamenti se attivato.
- Nessuna telemetria.
- Il consenso alla registrazione delle call resta responsabilità di Domenico: l'app aiuta solo con promemoria e messaggio pronto.
