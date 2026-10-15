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

## 2026-10-13 — Release anche dal pulsante "Run workflow"
- **Decisione:** `build.yml` avviato a mano su `main` con l'opzione **Pubblica** crea da solo tag e pre-release `v<versione di tauri.conf.json>`. Il push di un tag `v*` resta valido.
- **Motivo:** dalle sessioni cloud di Claude il push di tag su GitHub è rifiutato (403): si può fare push solo sul branch di lavoro. Così la release si pubblica senza tag a mano. La firma con il certificato è obbligatoria anche in questo caso, e una release già esistente non viene mai sovrascritta.

## 2026-10-13 — Webcam stile Loom: in M2 e M3
- **Decisione:** su richiesta di Domenico la bolla della fotocamera avrà interruttore prima della registrazione, grandezze, forme, colori del bordo e animazioni leggere a scelta (dettagli in §5.1 e §5.2 della specifica). Arriva in **M2** (interruttore, traccia separata, bolla di base) e **M3** (colori, animazioni, sfondo). M1 resta solo zoom e cursore.
- **Motivo:** M1 è uno spike per verificare la qualità degli zoom; la webcam arriva insieme alla registrazione completa e ai suoi permessi (fotocamera, microfono). Il renderer di M1 è costruito a livelli per accoglierla senza rifacimenti.

## 2026-10-13 — Scelte tecniche di M1 (spike zoom)
- **Solo macOS:** su Windows la build compila e la schermata spiega che la registrazione arriva più avanti; la cattura Windows entra in M2.
- **macOS minimo 13.0** (prima 12.3): lo richiede il crate `screencapturekit` 11.
- **Mouse letto con CoreGraphics** (`CGEventGetLocation`, `CGEventSourceButtonState`) a 120 Hz, senza permessi aggiuntivi. Scartato `device_query`: su macOS pretende il permesso Accessibilità e va in panic se manca.
- **Cattura → FFmpeg via pipe** (NV12, risoluzione nativa, 30 fps a frame rate costante); i tempi dei frame (host time di ScreenCaptureKit) e degli eventi del mouse usano lo stesso orologio di sistema. Se CPU o drop risultano troppo alti, in M2 si passa alla codifica in-process (VideoToolbox/AVAssetWriter).
- **Cursore sempre a freccia** nello spike; le forme reali in M3.

## 2026-10-15 — Zoom: ritorno a schermo intero e riquadro di anteprima (feedback di Domenico su M1)
- **Richiesta:** quando non clicca per un po' la ripresa deve tornare a 1×; in aggiunta, un riquadro tratteggiato visibile solo a lui
  che mostra cosa finirà nel video.
- **Causa nel motore di M1:** lo zoom finisce 1,5 s dopo l'ultimo clic, ma i clic a meno di ~2,9 s l'uno dall'altro si fondono in un
  solo blocco, anche se sono in punti lontani; in quel caso il livello resta al minimo (1,5×) invece di tornare a 1×. Con clic
  frequenti la ripresa resta zoomata a lungo e insegue il cursore.
- **Subito (correzione di M1, `v0.1.1`, da approvare):** regola esplicita "torna a schermo intero dopo N s senza clic"
  (predefinito 2 s, regolabile 1–5 s): nessuna fusione scavalca una pausa più lunga di N s; se i clic ravvicinati sono sparsi
  e a 1,5× non stanno nell'inquadratura, la ripresa resta a schermo intero invece di zoomare e inseguire il cursore.
- **Confermato dalla diagnostica** della prova (2026-10-15): 22 clic in 34 s → un solo zoom di 25 s a 1,5× con 20 clic, più uno
  finale da 2 clic; la ripresa non è mai tornata a 1×. Nella schermata "Prova zoom": impostazione + **Ricalcola zoom** sulla
  stessa registrazione, senza registrare di nuovo.
- **M2:** riquadro tratteggiato in sovrimpressione durante la registrazione, escluso dalla cattura (le finestre di FrameNote lo sono
  già) e trasparente ai clic. Usa la versione "in tempo reale" del motore zoom: l'export può anticipare lo zoom di 0,4 s rispetto
  al riquadro, perché conosce già il clic successivo. Interruttore per nasconderlo.

## 2026-10-15 — Icona di FrameNote nella barra dei menu (richiesta di Domenico)
- Icona sempre presente mentre l'app è attiva, con il **tempo trascorso accanto** durante la registrazione; aspetto diverso in
  registrazione (pallino rosso) e in pausa. Clic → menu di controllo rapido.
- **M2:** avvia/ferma con tempo trascorso, pausa/riprendi, annulla registrazione, microfono on/off, audio di sistema on/off,
  webcam on/off, scelta della sorgente (schermo intero, finestra, area), apri l'ultima registrazione / mostra nella cartella,
  Info e diagnostica, Esci. Sostituisce la parte "tray/menu bar" dell'indicatore di registrazione già in specifica (§5.1).
- **M3:** scorciatoie nel menu per forma e colore della bolla webcam (insieme a colori e animazioni della bolla).
- **M4:** trascrizione on/off.
- Tecnica: icona di sistema di Tauri 2 (`tray-icon`). Su Windows (M9) il testo accanto all'icona non esiste: il tempo va nel
  suggerimento e nel menu.

## 2026-10-15 — Trascrizione on/off e trascrizione in diretta (richiesta di Domenico)
- **On/off** dal menu della barra dei menu per trascrivere la registrazione in corso: **M4**.
- **Trascrizione in diretta** (durante registrazione o call, locale con whisper.cpp): fuori specifica, **in valutazione**. Stima
  dell'impatto sul MacBook M1 Pro 16 GB data a Domenico il 2026-10-15; la decisione arriva dopo una misura reale sul suo Mac
  all'inizio di M4 (test automatico nella diagnostica). Non è ancora nel piano.

## 2026-10-15 — Sicurezza e privacy, fase A (`PIANO_SICUREZZA.md` approvato da Domenico, D1 e D2 sì)
- **D1 — Progetti in `~/FrameNote/Progetti`.** Spostamento automatico dei progetti da `Documenti/FrameNote/Progetti`
  una sola volta e solo se FrameNote era già installata (il database esiste): una nuova installazione non tocca mai
  Documenti, quindi macOS non chiede più quel permesso. Nessuna sovrascrittura: un progetto con lo stesso nome resta
  dov'era e la home lo segnala.
- **D2 — Esclusi dalla cattura** con il filtro di ScreenCaptureKit: notifiche (`com.apple.notificationcenterui`) e
  password manager (Password, Accesso Portachiavi, 1Password, Bitwarden, Dashlane, LastPass, KeePassXC, Proton Pass,
  Enpass). ScreenCaptureKit esclude solo app già aperte: un thread controlla ogni 2 s e aggiorna il filtro
  (`update_content_filter`). Da verificare sul Mac con una notifica vera durante la registrazione.
- **Rete:** nessun client di rete compilato (controllo `scripts/check-no-network.sh` in CI sulle dipendenze di macOS,
  Windows e Linux); FFmpeg con `-protocol_whitelist file,pipe,fd` su ogni ingresso e uscita e percorsi `file:`
  espliciti. `fd` serve perché FFmpeg 6+ apre `-` con quel protocollo: scoperto dal test con FFmpeg reale, senza
  avrebbe bloccato registrazione ed export. Navigazione della webview limitata alle pagine dell'app, CSP con
  `object-src`/`frame-src`/`base-uri`/`form-action` a `'none'`. «Apri il video» apre solo `.mp4/.mov/.m4v` dentro
  la cartella progetti.
- **File:** `umask 077` per tutto il processo (anche FFmpeg): file `0600`, cartelle `0700`; le cartelle già esistenti
  vengono ristrette all'avvio. `.metadata_never_index` per Spotlight (efficacia da verificare in M4, quando ci saranno
  trascrizioni da cercare).
- **Diagnostica:** nomi delle cartelle dei progetti sostituiti da `progetto-N` in log e JSON, titolo tolto dal
  `project.json`, `privacy.json` aggiunto, elenco dei file mostrato prima della finestra di salvataggio.
- **Build:** `cargo audit` e `npm audit --omit=dev` in CI; test Rust su macOS con il sidecar FFmpeg vero; avvio di prova
  dell'app firmata con verifica dell'Hardened Runtime (che la firma aveva già da M0); hash dei sidecar FFmpeg fissati in
  `scripts/ffmpeg.env` oltre allo `SHA256SUMS.txt` della release.
- **Non fatto:** fissare le GitHub Actions per hash del commit. Servirebbe leggere i repository delle azioni
  (`actions/*`, `dtolnay/rust-toolchain`, `Swatinem/rust-cache`), fuori dall'accesso di questa sessione. Rischio
  limitato: i secret di firma sono visibili solo allo step di firma. Da fare quando Domenico aggiunge quei repository
  alla sessione o con Dependabot.

## 2026-10-15 — Zoom: nuove regole (correzione di M1, `v0.1.1`)
- `hold_ms` predefinito 2 s ("torna a schermo intero dopo"), regolabile nella schermata Prova zoom (1–5 s) con
  **Ricalcola zoom** sulla stessa registrazione e **Riapri l'ultima registrazione** dopo un riavvio.
- `merge_gap_ms` (1 s) ora è il tempo minimo a schermo intero tra due zoom. Se manca: stessa zona → zoom unico; zone
  lontane → si accorcia il primo zoom se c'è spazio, altrimenti cade quello con meno clic (entrambi se pari) e la
  ripresa resta a 1×; uno zoom caduto continua a "occupare" il suo intervallo, così una raffica di clic sparsi non
  genera zoom. Con la registrazione di prova (22 clic sparsi in 34 s) il risultato atteso è schermo intero.
- `ZoomSettings` con `#[serde(default)]`: i `project.json` di v0.1.0 restano leggibili.
