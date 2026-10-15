# FrameNote

Registratore di schermo con montaggio automatico, video da inviare ai clienti e note delle call
senza bot. Tutto resta sul computer: nessun cloud, nessun account, nessun abbonamento.

> Stato: **M1** (spike zoom) + protezione dei dati. **Prova zoom** registra lo schermo del Mac e crea un
> video con zoom automatici sui clic e cursore smussato; **Privacy e sicurezza** mostra come sono protetti
> i tuoi dati. Dettagli in [`PROGRESS.md`](PROGRESS.md).

## Scaricare la build

1. Apri la pagina **[Releases](https://github.com/mimmo8bit/FRAME.NOTE/releases)**.
2. Prendi l'ultima versione `v…` (non le release `ffmpeg-…`: sono componenti interni).
3. Scarica:
   - **Mac (Apple Silicon, M1 o successivi):** il file `FrameNote_…_aarch64.dmg`;
   - **Windows:** il file `FrameNote_…_x64_it-IT.msi`.

### Controllare il file (facoltativo)
Nella stessa release c'è `SHA256SUMS.txt`. Per confrontarlo:
- **Mac:** apri Terminale e scrivi `shasum -a 256 ` seguito dal file trascinato nella finestra, poi Invio.
- **Windows:** apri PowerShell e scrivi `Get-FileHash ` seguito dal percorso del file.

Il codice stampato deve essere uguale a quello accanto al nome del file in `SHA256SUMS.txt`.

## Installare e aprire la prima volta

### macOS (13 Ventura o successivo, Apple Silicon)
FrameNote è firmata con un certificato personale, non con uno Apple a pagamento: la prima volta
macOS la blocca. È normale e succede una volta per ogni nuova versione. Tra parentesi i nomi che
compaiono se il Mac è in inglese.

1. Apri il `.dmg` e trascina **FrameNote** sull'icona **Applicazioni** (*Applications*) nella stessa finestra.
   Se c'è già una versione precedente scegli **Sostituisci** (*Replace*).
2. Apri FrameNote da Applicazioni (o con ⌘ + barra spaziatrice, "FrameNote", Invio).
   Compare l'avviso "Impossibile aprire FrameNote" (*"FrameNote" Not Opened*): premi **Fine** (*Done*).
3. Apri **Impostazioni di Sistema → Privacy e sicurezza** (*System Settings → Privacy & Security*), scorri
   fino in fondo e premi **Apri comunque** (*Open Anyway*) accanto a FrameNote, poi inserisci la password del Mac.
4. Conferma con **Apri** (*Open Anyway*) nella finestra che compare.
5. Per **Prova zoom** serve il permesso **Registrazione schermo**: FrameNote lo chiede alla prima
   registrazione. Se non compare la richiesta: Impostazioni di Sistema → Privacy e sicurezza →
   **Registrazione schermo e audio di sistema** (*Screen & System Audio Recording*) → attiva FrameNote,
   poi chiudi e riapri l'app.

Dato che la firma è sempre la stessa, i permessi concessi (Registrazione schermo; microfono e fotocamera
nelle prossime versioni) restano validi anche dopo gli aggiornamenti.

### Windows
1. Apri il file `.msi`. Se compare "Windows ha protetto il PC" (SmartScreen), premi
   **Ulteriori informazioni** e poi **Esegui comunque**.
2. Segui l'installazione e apri FrameNote dal menu Start.

## Inviare la diagnostica
Se qualcosa non va, la diagnostica è il modo più rapido per capire cosa succede:

1. In FrameNote premi **Info e diagnostica**.
2. Premi **Esporta diagnostica** e salva il file `.zip` (per esempio sulla Scrivania).
3. Allegalo in chat. In alternativa basta uno screenshot della schermata.

Prima di salvarlo FrameNote mostra l'elenco esatto dei file. Lo zip contiene solo: i dati della
schermata (`info.json`), lo stato della protezione (`privacy.json`), i log dell'app e il `project.json`
dell'ultimo progetto senza titolo. **Mai video, audio, note o movimenti del mouse.** Il nome utente
diventa `~` e i nomi delle cartelle dei progetti diventano `progetto-1`, `progetto-2`…

## Privacy e sicurezza
I tuoi dati non escono dal computer. In FrameNote, **Privacy e sicurezza** mostra lo stato di ogni
protezione e cosa fare se qualcosa va sistemato:
- **Nessuna connessione a internet:** l'app non contiene componenti di rete (un controllo automatico
  blocca ogni build che ne includa uno) e FFmpeg può usare solo file sul computer.
- **Cartella dei progetti fuori dal cloud:** `~/FrameNote/Progetti`, che iCloud "Scrivania e Documenti"
  e OneDrive non sincronizzano. Le cartelle sono leggibili solo dal tuo utente ed escluse da Spotlight.
- **Fuori dai video:** finestre di FrameNote, banner delle notifiche e password manager (Password,
  Accesso Portachiavi, 1Password, Bitwarden, Dashlane, LastPass, KeePassXC, Proton Pass, Enpass),
  anche se li apri durante la registrazione.
- **Cifratura del disco:** la schermata controlla che FileVault sia attivo e spiega come attivarlo.
- **App protetta:** firma stabile con Hardened Runtime, così nessun altro programma può inserirsi in
  FrameNote e usarne il permesso di registrare lo schermo.

Il piano completo è in [`PIANO_SICUREZZA.md`](PIANO_SICUREZZA.md).

## Dove sono i file
- Progetti: `~/FrameNote/Progetti/` (la tua cartella Inizio; fino alla versione 0.1.0 erano in
  `Documenti/FrameNote`, e al primo avvio della 0.1.1 vengono spostati da soli)
- Dati dell'app (database, log, modelli AI):
  - Mac: `~/Library/Application Support/com.mimmo8bit.framenote/`
  - Windows: `%LOCALAPPDATA%\com.mimmo8bit.framenote\`

## FFmpeg e licenze
FrameNote include **FFmpeg 8.1.2** come programma separato, in una build **LGPL**
(niente componenti GPL o non liberi). Su Mac è compilato dai sorgenti ufficiali nella CI di questo
repository; su Windows è la build `win64-lgpl` di BtbN/FFmpeg-Builds. Sorgenti, configurazione
esatta e testo della licenza sono nella release [`ffmpeg-8.1.2-lgpl-1`](https://github.com/mimmo8bit/FRAME.NOTE/releases/tag/ffmpeg-8.1.2-lgpl-1)
e in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), incluso anche nell'app.

---

## Per lo sviluppo (CI)

Sul PC non si installa nulla: il codice si compila e si testa in GitHub Actions.

| Workflow | Quando | Cosa fa |
|---|---|---|
| `ci.yml` (Test) | push su qualsiasi branch tranne `main` | fmt, clippy, test Rust su Linux (anche con FFmpeg e GPU software) e su macOS (con il sidecar vero); tsc, Vitest, build frontend; guardiano della rete, vulnerabilità note (`cargo audit`, `npm audit`), nessuna registrazione o chiave nel repository |
| `build.yml` (Build) | push su `main`, tag `v*`, avvio manuale | test, poi `.dmg` firmato (con avvio di prova dell'app firmata) e `.msi` (artefatti per 3 giorni); sul tag, o a mano con "Pubblica", crea la Release |
| `ffmpeg.yml` | chiamato da `build.yml` | se manca, crea la release `ffmpeg-<versione>-lgpl-<n>` con i sidecar |
| `signing-bootstrap.yml` | solo a mano, una volta | crea il certificato di firma macOS nei secret |

- **Versione:** si cambia in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` e `package.json`
  (`scripts/check-version.sh` controlla che coincidano, e che coincidano col tag).
- **Nuova release:** aggiorna la versione, unisci su `main`, poi **Actions → Build → Run workflow**
  (branch `main`) con **Pubblica** attivo: la CI crea tag e pre-release `vX.Y.Z`. In alternativa
  basta fare push di un tag `vX.Y.Z` sul commit di `main`.
- **FFmpeg:** versione e checksum in `scripts/ffmpeg.env`. Per cambiare versione o opzioni
  aggiorna i valori e il suffisso di `FFMPEG_RELEASE_TAG`; dopo la prima build aggiorna anche
  `FFMPEG_PINNED_TAG` e gli hash `FFMPEG_SIDECAR_SHA256_*` (la build fallita li stampa).

### Certificato di firma macOS (una volta sola)
1. GitHub → foto profilo → **Settings → Developer settings → Personal access tokens →
   Fine-grained tokens → Generate new token**: nome `framenote-bootstrap`, scadenza 1 giorno,
   *Only select repositories* → FRAME.NOTE, *Repository permissions → Secrets: Read and write*.
2. Copia il token (non incollarlo in chat). Nel repository: **Settings → Secrets and variables →
   Actions → New repository secret**, nome `SIGNING_BOOTSTRAP_TOKEN`, valore il token.
3. **Actions → Crea certificato di firma → Run workflow** e aspetta la spunta verde.
4. Controlla che tra i secret ci siano `MACOS_CERT_P12`, `MACOS_CERT_PASSWORD`,
   `MACOS_SIGNING_IDENTITY`.
5. Elimina il secret `SIGNING_BOOTSTRAP_TOKEN` e revoca il token in Developer settings.

Senza certificato le build di `main` vengono firmate ad-hoc (con un avviso), le release no.
