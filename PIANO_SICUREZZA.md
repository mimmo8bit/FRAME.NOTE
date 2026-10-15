# FrameNote — Piano "Sicurezza e privacy"

> **Stato (2026-10-15):** approvato da Domenico (D1 sì, D2 sì). Fase A implementata per `v0.1.1`, tranne il blocco
> delle GitHub Actions per hash (motivo in `DECISIONS.md`). Hardened Runtime: era già attivo da M0, ora verificato in CI
> e mostrato nella schermata. Da verificare sul Mac: esclusione delle notifiche, FileVault, spostamento dei progetti.

## Obiettivo
I dati di Domenico (registrazioni, eventi del mouse, in futuro trascrizioni, note e nomi dei clienti) **non escono dal
Mac se non per un'azione esplicita e necessaria**. La protezione non deve basarsi sulla buona volontà del codice, ma
su blocchi verificabili: in CI a ogni build, nel sistema operativo, e visibili a Domenico in una schermata.

Questo lavoro non è una milestone della specifica: è un layer trasversale. La **fase A** si fa subito ed esce insieme
alle eventuali correzioni di M1 (versione `0.1.1`); la **fase B** sono regole vincolanti per le milestone successive.

---

## Da dove possono uscire i dati (analisi di oggi)

| Via d'uscita | Oggi | Rischio |
|---|---|---|
| Connessioni di rete dell'app (Rust) | Nessuna: nessun client HTTP è compilato | Basso, ma niente impedisce che una dipendenza futura ne porti uno |
| Interfaccia (webview) | CSP: solo IPC verso il backend | Basso; mancano blocco della navigazione e alcune direttive |
| FFmpeg | Mac compilato con `--disable-network`; **Windows (BtbN) ha la rete attiva** | Medio su Windows |
| Cartella progetti `Documenti/FrameNote` | **Se iCloud "Scrivania e Documenti" o OneDrive (Windows) sono attivi, le registrazioni vanno nel cloud** senza che FrameNote faccia nulla | **Alto** |
| Altri utenti dello stesso computer | Cartelle con permessi standard | Basso |
| Ricerca Spotlight / Siri | Indicizzano i file dei progetti (in futuro le trascrizioni) | Basso, locale |
| Contenuto della registrazione | Banner di notifiche (anteprime di messaggi/email) e password manager finiscono nel video | **Medio**: il video poi si invia ai clienti |
| Diagnostica (l'unica uscita voluta) | Home anonimizzata, mai video | Medio in futuro: i nomi delle cartelle e il `project.json` conterranno nomi di clienti |
| Altri programmi sul Mac | FrameNote ha il permesso di registrare lo schermo; la firma non ha l'Hardened Runtime | Medio: un malware potrebbe iniettare codice in FrameNote e usarne il permesso |
| Catena di build | Dipendenze bloccate (`--locked`); azioni GitHub per tag; **FFmpeg Windows preso da "latest" senza checksum** | Medio |

---

## Fase A — da fare ora

### A1. Rete chiusa per costruzione
- **Guardiano della rete in CI:** un controllo fallisce la build se nell'albero delle dipendenze *compilate* (macOS,
  Windows, Linux) compare un client di rete (`reqwest`, `hyper`, `ureq`, `curl`, `tungstenite`, `rustls`, `native-tls`,
  `openssl`, …). Oggi sono presenti solo come opzioni non attive di Tauri. Unica eccezione futura: il modulo `net/`
  della fase B, con approvazione esplicita.
- **Webview:** CSP più stretta (`object-src`, `frame-src`, `base-uri`, `form-action` a `'none'`), **blocco di qualsiasi
  navigazione** fuori dall'app e di nuove finestre, strumenti per sviluppatori spenti nella build di release. Test.
- **FFmpeg:** ogni comando parte con `-protocol_whitelist file,pipe` (FFmpeg rifiuta qualsiasi indirizzo di rete anche
  nella build Windows) e test che lo verifica su tutti i comandi generati.
- **Comandi dell'app:** "Apri il video" e "Mostra nella cartella" accettano solo file dentro la cartella progetti, anche
  con collegamenti simbolici o `..` nel percorso. Test.

### A2. Dati a riposo
- **Cartella progetti fuori dalla portata dei servizi di sincronizzazione** (decisione D1 sotto).
- **Permessi dei file:** cartelle `0700`, file `0600` su macOS: gli altri utenti del Mac non possono leggerli.
- **Spotlight:** file `.metadata_never_index` nella cartella progetti e nella cartella dati: le future trascrizioni non
  compaiono nella ricerca del Mac né nei suggerimenti di Siri.
- **Schermata "Privacy e sicurezza"** (dentro Info e diagnostica), con semaforo e istruzioni passo passo:
  - FileVault attivo? (la cifratura del disco del Mac: è la vera protezione dei file se il Mac viene perso o rubato);
  - cartella progetti sincronizzata con iCloud/Dropbox/Google Drive/OneDrive?
  - connessioni di rete dell'app: *nessuna*; FFmpeg senza rete;
  - firma dell'app stabile e Hardened Runtime attivo;
  - app escluse dalla registrazione (A3).

### A3. Cosa entra nella registrazione
- **Esclusi dalla cattura**, con il filtro di ScreenCaptureKit: i **banner delle notifiche** (anteprime di WhatsApp,
  Mail, Messaggi…) e i **password manager** (Portachiavi, Password di Apple, 1Password, Bitwarden, …). Le finestre
  di FrameNote sono già escluse. La lista diventa modificabile nelle impostazioni in M2.

### A4. Diagnostica (l'unica uscita voluta oggi)
- Nei log e nello zip i nomi delle cartelle dei progetti diventano `progetto-1`, `progetto-2`…; dal `project.json`
  incluso si tolgono titolo e percorsi. In futuro (M4) i log non conterranno mai testo di trascrizioni o note: regola
  verificata da test.
- Prima di salvare lo zip FrameNote mostra **l'elenco esatto dei file** che contiene.

### A5. Integrità dell'app e della build
- **Hardened Runtime** nella firma macOS: impedisce ad altri programmi di iniettare codice in FrameNote e sfruttarne il
  permesso di registrazione schermo. Il requisito designato non cambia, quindi **i permessi già dati restano**. In CI
  l'app firmata viene avviata davvero sul runner macOS (avvio di prova) per escludere blocchi all'apertura.
- **Catena di build:** azioni GitHub fissate per hash del commit; `cargo audit` (vulnerabilità note delle librerie
  Rust) e `npm audit` in CI; FFmpeg Windows fissato su una build datata di BtbN con SHA256 verificato; controllo che
  nel repository non finiscano mai video, audio, `events.jsonl`, database o chiavi.

---

## Fase B — regole per le prossime milestone (in `DECISIONS.md`)
- **Un solo modulo di rete (`net/`)** quando servirà (M4 modelli, M5 calendario ICS, M6 Notion, M10 aggiornamenti):
  solo HTTPS, solo host in lista, tutto **spento di default**, ogni connessione registrata e visibile nella schermata
  Privacy, interruttore **"Offline totale"**, modelli verificati con SHA256. Nessun dato di Domenico viene mai inviato
  tranne a Notion, se attivato.
- **macOS, M4:** valuto l'App Sandbox con **un piccolo programma separato per i download**: l'app che tocca le
  registrazioni resterebbe senza permesso di rete *a livello di sistema operativo*. Si decide con un piano dedicato.
- **Trascrizioni e note (M4):** database cifrato (SQLCipher) con la chiave nel Portachiavi: protegge anche i backup
  Time Machine non cifrati.
- **Webcam e microfono (M2):** permessi dichiarati in modo esplicito; indicatore sempre visibile quando sono attivi.
- **Windows (M9):** regola del firewall in uscita installata dal `.msi`, controllo di BitLocker.

## Cosa non faccio e perché
- **Cifrare i video con una chiave di FrameNote:** QuickTime e le anteprime non li aprirebbero più, l'export sarebbe più
  lento e perdere la chiave vorrebbe dire perdere tutto. FileVault cifra già tutto il disco, gratis e senza costi di
  prestazioni sui chip Apple: FrameNote controlla che sia attivo e spiega come attivarlo.
- **Password all'apertura dell'app:** non proteggerebbe i file sul disco (li protegge FileVault) e aggiungerebbe
  attrito a ogni registrazione. Il Mac è già protetto dalla password di accesso.

---

## Decisioni per Domenico
- **D1 — Cartella progetti.** *Consigliato:* spostarla in **`~/FrameNote/Progetti`** (la tua cartella Inizio, accanto a
  Documenti). iCloud "Scrivania e Documenti" e OneDrive non la sincronizzano, e macOS non chiede più l'accesso a
  Documenti. I progetti di prova già fatti vengono spostati in automatico. *Alternativa:* restare in Documenti con un
  avviso se la cartella è sincronizzata.
- **D2 — Esclusioni dalla registrazione.** *Consigliato:* notifiche e password manager esclusi di default.

## Verifica
- **CI (a ogni build):** guardiano della rete, test di CSP, navigazione bloccata, `-protocol_whitelist` su tutti i comandi
  FFmpeg, percorsi fuori dalla cartella progetti rifiutati, permessi `0700/0600`, anonimizzazione della diagnostica,
  `cargo audit`/`npm audit`, avvio di prova dell'app firmata con Hardened Runtime, requisito designato invariato.
- **Domenico, sul Mac (5 minuti):** apri **Privacy e sicurezza** e mandami uno screenshot; fai una registrazione breve
  mentre arriva una notifica (per esempio un messaggio a te stesso su WhatsApp) e controlla che nel video il banner non
  ci sia; esporta la diagnostica e controlla l'elenco dei file mostrato.

## Stima
Una sessione di lavoro. Esce come `v0.1.1` insieme alle correzioni di M1 che emergeranno dalla tua prova.
