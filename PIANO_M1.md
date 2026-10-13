# FrameNote — Piano di M1 (spike zoom)

## Obiettivo
Verificare il cuore di FrameNote prima di costruire il resto: registrare lo schermo con i movimenti e i clic del mouse e
produrre un MP4 con **zoom automatici fluidi** sui clic e **cursore smussato**, renderizzato con `wgpu`.

**Criterio di uscita (§7):** un video di 30 s con 5 clic, zoom fluidi e cursore smussato.
Se la qualità non è buona mi fermo e propongo soluzioni prima di andare avanti.

È uno *spike*: interfaccia minima, solo il necessario per provare il risultato. Barra REC, bolla, anteprima live,
recovery dopo crash, scorciatoie e audio arrivano in M2; look (sfondo, padding, ombre) ed editor in M3 e M7.

---

## 1. Flusso per Domenico
1. Home → **Prova zoom (M1)**.
2. Se manca il permesso di registrazione schermo: spiegazione + pulsante che apre il pannello giusto di Impostazioni di Sistema.
3. **Avvia registrazione** (schermo principale, senza audio) → lavori normalmente e fai qualche clic → **Ferma**.
4. FrameNote calcola gli zoom e mostra l'elenco ("3 zoom: 0:04–0:07 a 2,0×, …").
5. **Esporta con zoom** → barra di avanzamento → **Mostra nella cartella** / **Apri il video**.

I file finiscono in `Documenti/FrameNote/Progetti/AAAA-MM-GG_HHMM_prova-zoom/`, con la struttura di §4:
`project.json`, `screen/capture.mp4`, `events.jsonl`, `exports/zoom.mp4`.

## 2. Cattura dello schermo (macOS)
- **Crate `screencapturekit` 11** (binding Rust di ScreenCaptureKit, mantenuto, richiede macOS 13+).
- Schermo principale a **risoluzione nativa Retina** (sul MacBook 14": 3024×1964), 30 fps, **cursore escluso** dalla cattura (lo ridisegniamo noi, smussato).
- Formato dei frame **NV12** (YUV 4:2:0): 1,5 byte per pixel invece di 4, quindi meno dati da spostare.
- **Codifica:** i frame vanno via pipe al **sidecar FFmpeg** con `h264_videotoolbox` (encoder hardware verificato in M0), alta qualità perché è la sorgente del montaggio. File **MP4 frammentato**: se l'app si chiude di colpo, il video registrato fin lì resta leggibile. La segmentazione da 2 s con recovery completa è il lavoro di M2.
- Gli istanti dei frame vengono dai timestamp di ScreenCaptureKit, così video ed eventi del mouse restano allineati.
- **Permesso:** prima di registrare controllo `CGPreflightScreenCaptureAccess`; se manca lo chiedo con `CGRequestScreenCaptureAccess` e mostro come abilitarlo. Grazie alla firma stabile di M0, il permesso concesso resta valido anche con le versioni successive.

## 3. Mouse: posizione e clic (`events.jsonl`)
- **CoreGraphics** (`CGEventGetLocation`, `CGEventSourceButtonState`): lettura della posizione e dei pulsanti del mouse 120 volte al secondo, su un thread dedicato. (Il crate `device_query` è stato scartato: su macOS pretende il permesso Accessibilità.)
  Su macOS **non serve nessun permesso aggiuntivo** (niente "Monitoraggio input" o "Accessibilità"). La tastiera, che quel permesso lo richiede, arriva in M2.
- Formato §4: `{"t_ms":…, "type":"move"|"click", "x":…, "y":…, "button":"left", "display":…}`.
  Coordinate convertite da punti di macOS a **pixel del video** (fattore Retina e origine dello schermo).
  Le registro sempre con lo stesso orologio dei frame.
- Un clic dura di solito 80–150 ms; a 120 Hz lo vedo sempre. Registro sia la pressione sia il rilascio.

## 4. Motore degli zoom (logica pura, ben testata)
Modulo `render/zoom.rs`, senza dipendenze di sistema, quindi testabile ovunque.
- **Raggruppa i clic** vicini nel tempo e nello spazio: entro 2,5 s e 350 px, regolabili.
- **Per ogni gruppo un blocco zoom:** inizio 0,4 s prima del primo clic, fine 1,5 s dopo l'ultimo, durata minima 1,2 s.
  Due blocchi separati da meno di 1 s si fondono: **isteresi contro gli zoom nervosi**.
- **Livello 1,5–2,5×,** scelto per far stare l'area dei clic con un margine; il centro è il baricentro dei clic,
  spostato quanto basta perché l'inquadratura non esca mai dal video.
- **Dentro lo zoom** la telecamera segue il cursore solo quando esce da una zona morta centrale, così non trema.
- **Transizioni con molla smorzata criticamente** (niente rimbalzi, niente scatti), calcolata a passo fisso: lo
  stesso istante dà sempre la stessa inquadratura, in anteprima come in export.
- **Ogni zoom è un blocco** `{inizio, fine, livello, centro}` salvato in `project.json`. In M7 si modificheranno a mano;
  in M1 si leggono e basta.

**Cursore** (modulo `render/cursor.rs`):
- smussatura del percorso (filtro "One Euro": liscio quando si muove piano, reattivo quando si muove veloce);
- ingrandimento 1,5×;
- anello animato sul clic;
- dissolvenza dopo 2 s di immobilità.

Nello spike il cursore è sempre una **freccia standard** disegnata da noi. Le altre forme (I testuale, manina) sono un
limite noto, da affrontare in M3.

## 5. Renderer `wgpu` (offline, lo stesso che farà l'anteprima)
Modulo `render/`:
1. FFmpeg decodifica `capture.mp4` in frame grezzi verso il renderer.
2. Ogni frame va in una texture GPU. Lo shader applica l'inquadratura della telecamera (ritaglio + scala con filtro
   **bicubico**, più nitido del bilineare quando si ingrandisce) e disegna sopra il cursore smussato e l'anello del clic.
3. Il frame torna in memoria e va via pipe a FFmpeg (`h264_videotoolbox`) → `exports/zoom.mp4`.

- **Uscita:** stesse proporzioni dello schermo, lato lungo 1920 px (sul MacBook 14": 1920×1246), 30 fps.
- **Avanzamento** mostrato nell'interfaccia; nel report salvo il tempo di export.
- Lo stesso codice girerà poi per l'anteprima in tempo reale (M3/M7). In M1 non c'è anteprima: solo export.

## 6. Report automatico della prova
Spec §1, "modalità auto-test". Alla fine di registrazione ed export salvo in `project.json` dati che arrivano anche nello
zip di "Esporta diagnostica":
- frame catturati e attesi, quindi drop;
- fps reali;
- CPU media durante la registrazione;
- eventi del mouse e clic rilevati;
- zoom generati;
- tempo di export.

Così vedo i numeri senza chiederti di misurare nulla.

## 7. Windows
In M1 il codice di cattura è **solo macOS** (OS primario). Su Windows la build continua a compilarsi e il pulsante
"Prova zoom" spiega che la registrazione su Windows arriva più avanti. Non è una funzione finta: dice chiaramente
cosa manca. Il motore degli zoom e il renderer sono già multipiattaforma; la cattura Windows
(`windows-capture`) entra in M2 e si rifinisce in M9.

## 8. Test automatici (in CI, senza Mac)
- **Zoom:** raggruppamento dei clic, isteresi, durata minima, inquadratura sempre dentro il video, molla continua e
  senza rimbalzi, determinismo (stesso input → stesso output).
- **Cursore:** filtro di smussatura, dissolvenza, anello del clic.
- **Eventi:** lettura/scrittura di `events.jsonl`, conversione da punti a pixel (Retina, schermo con origine diversa da 0,0).
- **Renderer su GPU software:** in CI Linux installo il driver Vulkan software (lavapipe), che è solo nella CI e non
  sul Mac. Renderizzo frame di prova con un motivo noto e verifico i pixel: che lo zoom 2× ingrandisca davvero la zona
  giusta e che il cursore sia nel punto atteso.
- **Pipeline completa:** video sintetico generato con FFmpeg + `events.jsonl` con 5 clic → export → controllo durata,
  risoluzione e numero di frame dell'MP4 finale.
- Sul runner macOS: compilazione e test del codice di cattura che non richiede permessi. La cattura vera si può
  provare solo sul tuo Mac.

## 9. File e dipendenze
- **Rust nuovi:**
  - `capture/{mod.rs, macos.rs}`
  - `input_events/{mod.rs, poller.rs}`
  - `render/{mod.rs, zoom.rs, cursor.rs, gpu.rs, shaders/zoom.wgsl}`
  - `encoder/pipe.rs` (FFmpeg in ingresso e in uscita)
  - `project/{mod.rs, schema.rs}` (cartella progetto + `project.json` v1)
  - comandi nuovi in `commands.rs`
- **Frontend:** `src/windows/ProvaZoom.tsx` e un pulsante nella Home.
- **Crate nuovi:** `screencapturekit` (solo macOS), `bytemuck`; `wgpu` c'è già. Mouse con CoreGraphics, senza crate aggiuntivi.
- **Configurazione:** macOS minimo da 12.3 a **13.0**, richiesto da `screencapturekit`; il tuo Mac ha 26.6.
  Lo annoto in `DECISIONS.md`.

## 10. Rischi
| Rischio | Cosa faccio |
|---|---|
| **Copia dei frame verso FFmpeg pesante** a risoluzione Retina (≈ 270 MB/s) | NV12 + 30 fps. Il report misura CPU e drop. Se è troppo, in M2 passo alla codifica in-process con VideoToolbox/AVAssetWriter (zero copie), già valutata. |
| **Il codice di cattura si prova solo sul tuo Mac** (in CI non c'è il permesso) | Log dettagliati in ogni passaggio, errori in italiano, e nella diagnostica tutto quello che serve per correggere senza tentativi alla cieca. |
| **Coordinate mouse ↔ video** (Retina, origini) | Funzione di conversione pura e testata. Nel report salvo dimensioni e fattore di scala rilevati. |
| **Export lento** (decodifica e rilettura dalla GPU) | Per uno spike va bene; misuro il tempo. L'ottimizzazione (zero copie su Metal) serve per la regola dei 15 s di M3. |
| **Forma del cursore** sempre a freccia | Limite noto dello spike, risolto in M3. |
| **Qualità dello zoom insoddisfacente** | Come da spec mi fermo, ti mostro il problema e propongo alternative: parametri, filtro di scala, risoluzione di uscita. |

---

## Cosa dovrai fare tu (quando la build `v0.1.0` è pronta)
1. Scarica e installa la nuova versione **sopra** la vecchia: trascina in Applications e scegli **Sostituisci**.
   - Se macOS mostra di nuovo "Not Opened", ripeti **Open Anyway** come oggi.
2. Apri FrameNote → **Prova zoom (M1)**.
3. Alla prima registrazione concedi **Registrazione schermo**: System Settings → Privacy & Security →
   **Screen & System Audio Recording** → attiva FrameNote. Riapri l'app se te lo chiede.
4. **Registra circa 30 s** di qualcosa di non riservato (per esempio una pagina web) facendo **5 clic** in punti diversi,
   poi premi **Ferma**.
5. **Esporta con zoom** e guarda il video. Dimmi:
   - se gli zoom sono fluidi o nervosi;
   - se il cursore è morbido;
   - se l'immagine è nitida.
6. Mandami in chat lo zip di **Esporta diagnostica** e, se te la senti, anche il file `exports/zoom.mp4`: posso
   estrarne i fotogrammi e controllare la qualità.

## Verifica
- **CI:** test verdi su Linux e macOS, build `.dmg`/`.msi` verde, release `v0.1.0` firmata con lo stesso requisito
  designato di `v0.0.1`.
- **Prova manuale (da verificare da Domenico):** video di 30 s con 5 clic, zoom fluidi, cursore smussato. Esito annotato in `PROGRESS.md`.
