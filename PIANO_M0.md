# FrameNote — Piano di M0

## Contesto
M0 crea la base su cui poggiano tutte le milestone successive: il repo con la CI, i pacchetti firmati e scaricabili dalla Release, lo scheletro dell'app e la diagnostica. La diagnostica è l'unico modo che ho per vedere cosa succede sul Mac.
**Criterio di uscita (§7):** Domenico installa la build, l'app si apre e la schermata di diagnostica mostra encoder e GPU rilevati.

**Due cose da sapere prima di iniziare:**
- **Questa sessione gira sul tuo Mac, non nel cloud.** Qui non posso compilare nulla, e il Mac non ha le credenziali GitHub, quindi il push è fallito. Il commit su `DECISIONS.md` (limiti CI) è pronto ma non è su GitHub; vedi il passo A più sotto.
- **Il codice di M0 va scritto nella sessione cloud**, come deciso in `DECISIONS.md`. Questo piano è il brief per quella sessione.

---

## 1. Repo e CI (`.github/workflows/`)

| File | Quando parte | Cosa fa |
|---|---|---|
| `ci.yml` | push su qualsiasi branch tranne `main`, e PR | Su `ubuntu-latest`: `cargo fmt --check`, `cargo clippy`, `cargo test`, `npm ci`, `tsc`, `vitest`. Su `macos-latest`: solo `cargo check`/`cargo test` del codice specifico macOS. Nessun pacchetto. |
| `build.yml` | push su `main` e tag `v*` | Matrice `macos-latest` (aarch64) + `windows-latest` con `tauri-apps/tauri-action`, dopo gli stessi test di `ci.yml`. Su `main` produce .dmg/.msi come artefatti (retention 3 giorni, per il limite di 500 MB). Sul tag `vX.Y.Z` crea la **GitHub Release** (pre-release) con .dmg, .msi e `SHA256SUMS.txt`. |
| `ffmpeg.yml` | chiamato da `build.yml` | Vedi §4. Se la release `ffmpeg-<versione>` esiste già non fa nulla, altrimenti compila e pubblica. |
| `signing-bootstrap.yml` | solo a mano (`workflow_dispatch`), una volta | Vedi §2. |

- **Versione:** la fonte unica è `src-tauri/tauri.conf.json`. Un controllo in CI fallisce se il tag non coincide. M0 esce come `v0.0.1`.
- **Commit nell'app:** lo SHA del commit viene incluso nel binario (`build.rs` + `GITHUB_SHA`) e mostrato in diagnostica.
- **Rischi:**
  - Le build Tauri su Windows sono lente (circa 15–20 min).
  - In parallelo girano al massimo 5 job macOS, che per noi bastano.
  - Su Linux servono i pacchetti `libwebkit2gtk-4.1-dev` e simili via `apt`. Sul runner non è un problema.

## 2. Firma self-signed stabile su macOS

**Idea:** il certificato nasce dentro GitHub e non passa mai né dal Mac né dalla chat.
- Il workflow `signing-bootstrap.yml` gira su `macos-latest` e genera con `openssl` un certificato *Code Signing* self-signed: CN "FrameNote Self-Signed", validità 20 anni, password casuale.
- Salva tutto nei secret con `gh secret set`, usando un token temporaneo creato da Domenico: `MACOS_CERT_P12` (in base64), `MACOS_CERT_PASSWORD` e `MACOS_SIGNING_IDENTITY`.
- Non stampa mai i valori. Se i secret esistono già, si rifiuta di sovrascriverli: è così che la firma resta stabile.

**Firma in `build.yml`:**
- Crea un keychain temporaneo, importa il .p12 e firma l'app e il sidecar FFmpeg con `codesign --options runtime`. L'identità è indicata con l'hash SHA-1, così funziona anche con un certificato non "trusted".
- Il job stampa `codesign -d -r-`, cioè il requisito designato, che contiene l'hash del certificato. Deve restare identico da una build all'altra: è da lì che dipendono i permessi TCC.

**Rischi:**
- `tauri-action` potrebbe non accettare un'identità non fidata. In quel caso firmo con uno step `codesign` mio dopo il bundle e rigenero il .dmg.
- Se il secret si perde, si rigenera una volta e Domenico deve riconcedere i permessi una volta sola.
- Niente notarizzazione: richiede l'Apple Developer Program a pagamento, quindi è esclusa.

**Windows:** in M0 il pacchetto .msi non è firmato. Il README spiega SmartScreen.

## 3. Scaffolding Tauri 2 + React/TS/Vite/Tailwind
- Si genera con `npm create tauri-app` (template react-ts) **nell'ambiente cloud**, poi si ripulisce.
- **Frontend:** `react`, `react-dom`, `typescript`, `vite`, `tailwindcss` v4 con `@tailwindcss/vite`, `@tauri-apps/api`, i plugin `dialog` e `opener`, `vitest` + `@testing-library/react`.
- **Rust:**
  - `tauri` 2, `tauri-plugin-dialog`, `tauri-plugin-opener`, `tauri-plugin-shell` (per il sidecar)
  - `serde`/`serde_json`, `thiserror`/`anyhow`
  - `tracing`, `tracing-subscriber`, `tracing-appender`
  - `rusqlite` (feature `bundled`), `zip`, `sysinfo`, `wgpu` (solo enumerazione degli adattatori)
- **Struttura (§3):**
  - `src-tauri/src/` con `lib.rs`, `commands.rs`, `paths.rs`, `library/db.rs`, `encoder/ffmpeg.rs`, `diagnostics/{mod,logging,info,export}.rs`
  - `src/windows/{Home,Diagnostica}.tsx`
  - Le cartelle degli altri moduli si creano quando servono: niente file vuoti.
- **Identificatore app:** `com.mimmo8bit.framenote`. Non si cambierà mai più, perché da lui dipendono i permessi e la cartella dati. Va annotato in `DECISIONS.md`.
- **UI:** in italiano, tema di sistema. Home minimale con il link a "Info e diagnostica".

## 4. FFmpeg LGPL come sidecar
- **macOS arm64:** si compila dai sorgenti ufficiali di FFmpeg 8.0, scaricati da `ffmpeg.org` e verificati con SHA256.
  - Opzioni di build: `--disable-gpl --disable-nonfree --enable-videotoolbox --enable-audiotoolbox`, statico e senza dipendenze esterne.
  - Encoder hardware: `h264_videotoolbox` e `hevc_videotoolbox`.
- **Windows x64:** build `win64-lgpl` di BtbN/FFmpeg-Builds, con versione fissa e checksum. Include gli encoder NVENC, QSV, AMF e libopenh264.
- **Pubblicazione:** i binari vanno su una release separata `ffmpeg-8.0-lgpl` dello stesso repo. `build.yml` li scarica, verifica lo SHA256 e li copia in `src-tauri/binaries/ffmpeg-<target-triple>`, che è in `.gitignore`. Nel codice sono dichiarati come `externalBin` in `tauri.conf.json`.
- **Licenza:** il testo LGPL, la configurazione di build e il link ai sorgenti vanno nel pacchetto (`THIRD_PARTY_NOTICES`) e nel README.
- **Rischi:**
  - La compilazione su macOS richiede circa 10–20 minuti, ma si fa una volta sola.
  - Su Mac non c'è un fallback software H.264 LGPL (libx264 è GPL). Su Mac c'è sempre VideoToolbox; se serve si aggiunge openh264 dopo.

## 5. SQLite bundled
- Il database `framenote.sqlite` sta nella cartella dati dell'app.
- Le migrazioni usano `PRAGMA user_version`. Lo schema v1 contiene solo le tabelle `meta` e `settings`; le tabelle vere arrivano con la libreria in M4.
- **Test:** la migrazione è idempotente e FTS5 è disponibile (si crea una tabella virtuale di prova in memoria).

## 6. Cartelle (§4)
- **Progetti:** `~/Documents/FrameNote/Progetti/` su macOS, `%USERPROFILE%\Documents\FrameNote\Progetti\` su Windows.
- **Dati app:** `~/Library/Application Support/com.mimmo8bit.framenote/` su macOS, con dentro `logs/`, `models/` e `framenote.sqlite`.
- Le cartelle si creano al primo avvio.
- **Rischio:** macOS chiede il permesso per la cartella Documenti. Serve la voce `NSDocumentsFolderUsageDescription` in italiano nell'Info.plist. Spostare la cartella da Impostazioni non fa parte di M0.

## 7. Log ruotati
- `tracing-appender` con rotazione giornaliera e al massimo 7 file in `logs/`.
- Viene registrato anche un panic hook.
- Il frontend manda i suoi errori al log con un comando dedicato.
- I log non contengono mai contenuti utente.

## 8. "Info e diagnostica" + "Esporta diagnostica"
**La schermata mostra:**
- versione e commit
- OS e versione, CPU, RAM
- GPU: adattatori `wgpu` con nome e backend Metal/DX12/Vulkan
- versione di FFmpeg ed encoder hardware rilevato
- percorsi, con la home sostituita da `~`
- stato del database, spazio libero su disco
- stato dei modelli: "nessun modello installato" (dato reale)

**Rilevamento dell'encoder:** prima legge `ffmpeg -encoders`, poi fa una **codifica di prova reale** di 0,2 s per ciascun encoder (`-f lavfi -i testsrc … -f null -`). Un encoder viene dichiarato solo se la prova riesce.

**Esporta diagnostica:**
- Crea uno .zip con `info.json`, i log e il `project.json` dell'ultimo progetto se esiste (mai i video).
- Lo salva dove sceglie Domenico (finestra di salvataggio) e poi offre "Mostra nella cartella".

**Test automatici:**
- Parsing dell'output di `-encoders` (con campioni reali come fixture)
- Contenuto dello zip
- Anonimizzazione dei percorsi
- Rendering della schermata con Vitest

**Smoke test in CI:** nel job macOS si avvia il sidecar FFmpeg incluso nell'app con `-version` e `-encoders`, e si verifica la firma con `codesign --verify --deep --strict`.

## 9. README
Contiene:
- dove scaricare: Releases → ultima `v*` → `.dmg` o `.msi`
- come verificare lo SHA256 (facoltativo)
- **macOS 26:**
  1. apri il .dmg e trascina FrameNote in Applicazioni
  2. apri l'app: compare l'avviso, premi "Fine"
  3. vai in Impostazioni di Sistema → Privacy e sicurezza → "Apri comunque" e inserisci la password
- **Windows:** SmartScreen → "Ulteriori informazioni" → "Esegui comunque"
- come esportare la diagnostica e allegarla in chat
- la nota di licenza per FFmpeg

## 10. Chiusura di M0
- Aggiornare `PROGRESS.md` e `DECISIONS.md` (identificatore app, fonte di FFmpeg, schema della firma).
- Fare il tag `v0.0.1`.
- Scrivere le istruzioni di prova per Domenico.

---

## Cosa dovrà fare Domenico a mano (tutto dal sito, niente da installare)

**A. Ora: portare su GitHub il commit dei limiti CI**
1. Apri github.com/mimmo8bit/FRAME.NOTE → **Add file** → **Upload files**.
2. Trascina `Desktop/Claude Code/FRAME NOTE/DECISIONS.md` (l'ho già aggiornato).
3. Scrivi il messaggio "DECISIONS: limiti GitHub Actions" → **Commit changes** su `main`.

In alternativa, lo rifà la sessione cloud come primo commit.

**B. Aprire la sessione cloud**
1. Su claude.ai/code collega GitHub, se non è già fatto.
2. Scegli il repo `mimmo8bit/FRAME.NOTE` e incolla questo piano.

**C. Certificato di firma (una volta sola, dopo che il workflow è su `main`)**
1. GitHub → foto profilo → **Settings** → **Developer settings** → **Personal access tokens** → **Fine-grained tokens** → **Generate new token**.
2. Impostazioni del token:
   - nome "framenote-bootstrap", scadenza **1 giorno**
   - *Repository access*: **Only select repositories** → FRAME.NOTE
   - *Permissions* → Repository → **Secrets: Read and write**
3. **Generate token** e copialo, senza incollarlo in chat.
4. Repo → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**: nome `SIGNING_BOOTSTRAP_TOKEN`, valore il token → **Add secret**.
5. Repo → **Actions** → "Crea certificato di firma" → **Run workflow** → aspetta la spunta verde.
6. Controlla che in Secrets ci siano `MACOS_CERT_P12`, `MACOS_CERT_PASSWORD` e `MACOS_SIGNING_IDENTITY`.
7. Elimina il secret `SIGNING_BOOTSTRAP_TOKEN`, poi in Developer settings elimina il token.

**D. Prova della build `v0.0.1`**
1. Releases → `v0.0.1` → scarica il .dmg.
2. Installa seguendo i passi Gatekeeper del README.
3. Apri FrameNote → **Info e diagnostica**.
4. Incolla in chat uno screenshot della schermata, oppure lo .zip di "Esporta diagnostica", e scrivi se l'app si è aperta.

## Verifica
- **CI:** `ci.yml` verde su un branch di lavoro; `build.yml` verde su `main` per macOS e Windows.
- **Firma e FFmpeg:** il log di build mostra il requisito designato con l'hash del certificato, identico in due build consecutive, e lo smoke test del sidecar passa.
- **Release:** `v0.0.1` contiene .dmg, .msi e `SHA256SUMS.txt`.
- **Prova manuale (da verificare da Domenico):** l'app si apre e la diagnostica mostra `h264_videotoolbox` e la GPU Apple M1 Pro (Metal). L'esito va annotato in `PROGRESS.md`.
