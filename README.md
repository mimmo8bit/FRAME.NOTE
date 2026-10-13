# FrameNote

Registratore di schermo con montaggio automatico, video da inviare ai clienti e note delle call
senza bot. Tutto resta sul computer: nessun cloud, nessun account, nessun abbonamento.

> Stato: **M0** (prima build di prova). L'app si apre e mostra la schermata "Info e diagnostica";
> la registrazione arriva nelle milestone successive. Dettagli in [`PROGRESS.md`](PROGRESS.md).

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

### macOS (26 Tahoe e precedenti)
FrameNote è firmata con un certificato personale, non con uno Apple a pagamento: la prima volta
macOS la blocca. È normale e succede una volta sola per ogni nuova versione.

1. Apri il `.dmg` e trascina **FrameNote** su **Applicazioni**.
2. Apri FrameNote da Applicazioni. Compare l'avviso "Apple non ha potuto verificare…": premi **Fine**.
3. Apri **Impostazioni di Sistema → Privacy e sicurezza**, scorri fino in fondo e premi
   **Apri comunque** accanto a "FrameNote è stata bloccata", poi inserisci la password del Mac.
4. Conferma con **Apri** nella finestra che compare.
5. Al primo avvio macOS chiede l'accesso alla cartella **Documenti**: rispondi **Consenti**
   (lì FrameNote crea `Documenti/FrameNote/Progetti`).

Dato che la firma è sempre la stessa, i permessi concessi (Documenti oggi; schermo, microfono e
fotocamera nelle prossime versioni) restano validi anche dopo gli aggiornamenti.

### Windows
1. Apri il file `.msi`. Se compare "Windows ha protetto il PC" (SmartScreen), premi
   **Ulteriori informazioni** e poi **Esegui comunque**.
2. Segui l'installazione e apri FrameNote dal menu Start.

## Inviare la diagnostica
Se qualcosa non va, la diagnostica è il modo più rapido per capire cosa succede:

1. In FrameNote premi **Info e diagnostica**.
2. Premi **Esporta diagnostica** e salva il file `.zip` (per esempio sulla Scrivania).
3. Allegalo in chat. In alternativa basta uno screenshot della schermata.

Lo zip contiene solo: i dati della schermata (`info.json`), i log dell'app e il `project.json`
dell'ultimo progetto se esiste. **Mai video, audio o note.** I percorsi hanno il nome utente
sostituito da `~`.

## Dove sono i file
- Progetti: `Documenti/FrameNote/Progetti/`
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
| `ci.yml` (Test) | push su qualsiasi branch tranne `main` | fmt, clippy, test Rust su Linux; test Rust su macOS; tsc, Vitest, build frontend |
| `build.yml` (Build) | push su `main`, tag `v*`, avvio manuale | test, poi `.dmg` firmato e `.msi` (artefatti per 3 giorni); sul tag crea la Release |
| `ffmpeg.yml` | chiamato da `build.yml` | se manca, crea la release `ffmpeg-<versione>-lgpl-<n>` con i sidecar |
| `signing-bootstrap.yml` | solo a mano, una volta | crea il certificato di firma macOS nei secret |

- **Versione:** si cambia in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` e `package.json`
  (`scripts/check-version.sh` controlla che coincidano, e che coincidano col tag).
- **Nuova release:** tag `vX.Y.Z` sul commit di `main` → la CI pubblica la pre-release.
- **FFmpeg:** versione e checksum in `scripts/ffmpeg.env`. Per cambiare versione o opzioni
  aggiorna i valori e il suffisso di `FFMPEG_RELEASE_TAG`.

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
