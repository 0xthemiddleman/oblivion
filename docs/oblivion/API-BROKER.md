# Primo broker API

24 ottobre 2026. Componente implementato in `packages/oblivion-gateway`, separato
dal controller privilegiato. Provato nativamente sul Mac ARM e nel guest Debian
ARM senza swap, avviato dal Mac con HVF. Il [laboratorio di rete](NETWORK-LAB.md)
ora esegue il broker nella VM gateway e verifica firewall e Tor reale. Il
[supervisore Rust](GATEWAY-CONTROL.md) lo avvia e revoca tramite il controllo
virtio privato dell’host, senza usare il launcher Python della fixture. La VM
avviata dal controller ora usa la [rete privata della sessione](SESSION-NETWORK.md),
con token nuovo a ogni apertura e revoca prima della chiusura. Lo
[stato Tor](TOR-STATE.md) viene ora importato e salvato nel generale.
Restano proxy dei tool e compatibilità completa del provider.

Il binario `oblivion-api` gestisce una sola autorizzazione, senza poterla
riassegnare a un'altra sessione. Il supervisore gli consegna la chiave reale;
OpenCode riceve soltanto un token locale revocabile. Il broker non monta il
profilo generale o i vault e non aggiunge dipendenze HTTP/TLS al controller LUKS.
Il [salvataggio della credenziale](CREDENTIALS.md) nel generale cifrato è ora
implementato nella TUI fidata. Il [servizio host](HOST-GATEWAY.md) ora consegna
la chiave al supervisore e gestisce la VM gateway insieme al generale. Il
controller ora autorizza il broker e consegna il token alla singola sessione.

## Superficie ammessa

| Confine                       | Comportamento implementato                                                                                                                                                            |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Richiesta del guest           | Solo `POST /v1/chat/completions`, senza query o URI assoluto; bearer token locale                                                                                                     |
| Provider                      | Origin compilato `https://api.abliteration.ai/v1/chat/completions`; nessun override nel protocollo di attivazione                                                                     |
| Modelli                       | I tre identificatori del catalogo già fissato nel fork: `abliterated-model`, `abliterated-model-large`, `abliterated-model-large-v2`; disponibilità effettiva non verificata          |
| Corpo                         | JSON con campi ammessi espliciti, riserializzato dopo la validazione; streaming obbligatorio, messaggi e tool calling `function`                                                      |
| Funzioni esterne del provider | Campi sconosciuti, `web_search_options`, altri tipi di tool e gli altri endpoint vengono rifiutati                                                                                    |
| Allegati API                  | Immagini e PDF inline in data URL; URL HTTP, file locali e riferimenti remoti negli schemi dei tool rifiutati. Non è una validazione del formato del documento                        |
| Opzioni                       | `store: true` vietato; opzioni numeriche limitate; nessun passthrough di body o header arbitrari                                                                                      |
| Trasporto                     | SOCKS5 con hostname remoto (`socks5h`) verso un indirizzo loopback dato dal controller; resolver locale che rifiuta, TLS verificato, HTTP/1, redirect e retry automatici disabilitati |
| Credenziali                   | Authorization del guest sostituita; gli altri header del guest non vengono inoltrati. Credenziali SOCKS separate per ruolo API e token della sessione                                 |
| Risposta                      | Solo SSE con status 200; header ridotti. Errori, redirect e diagnostica del provider non vengono restituiti al guest; 429 mantiene un errore generico di limite                       |
| Revoca                        | Interrompe richieste in attesa, stream e connessioni del listener. Il completamento attende la cancellazione dei task HTTP                                                            |

La documentazione del provider colloca la ricerca in `web_search_options` su
Chat Completions; il fetch nativo usa invece Messages. Entrambi i percorsi sono
esclusi dalla superficie del broker. Questo non controlla l'infrastruttura del
provider né cambia le sue dichiarazioni sulla conservazione dei dati.
Fonti verificate il 24 ottobre 2026: [ricerca](https://docs.abliteration.ai/capabilities/web-search/openai-chat-completions),
[fetch](https://docs.abliteration.ai/capabilities/web-fetch),
[data handling](https://abliteration.ai/data-handling).

L'adapter upstream rimane quello OpenAI-compatible già usato da Abliteration.
Lo schema del broker deriva da `packages/ai/src/protocols/openai-chat.ts`.
Le prove iniziali del componente costruiscono richieste di quel formato. La
[suite della sessione](SESSION-NETWORK.md) ora avvia anche il vero OpenCode
attraverso il broker, verificando autenticazione e ammissione del payload con
Tor disabilitato; non verifica ancora una risposta del provider. Reasoning strutturato, cache control e
altre estensioni non incluse vengono rifiutati: la compatibilità completa, inclusi
titoli e compaction, rimane da verificare con risposte complete e streaming.

## Attivazione e chiusura

Il processo non accetta argomenti. Riceve due descrittori già aperti:

- FD 3: stream Unix privato dal controller, con una riga JSON iniziale contenente
  `api_key`, `session_token` e `socks`; nessun altro campo è accettato.
- FD 4: listener TCP già aperto dal componente fidato. Il broker non sceglie o
  apre autonomamente una porta di ascolto.

La riga iniziale è limitata a 8 KiB e cinque secondi. Il token deve essere una
stringa esadecimale di 64 caratteri; il controller lo genera con 32 byte
casuali da CSPRNG a ogni apertura, senza riuso. Il broker restituisce
`{"ready":true}` sul canale privato. EOF, errore o qualunque byte successivo su
quel canale revocano l'autorizzazione. Anche SIGTERM/SIGINT chiudono il servizio.
Il supervisore attende l'uscita del processo prima di confermare la revoca al
controller; la chiusura del volume dipende ora da questa conferma o dalla
distruzione verificata della VM gateway.
La revoca non può richiamare byte già consegnati alla rete o dati già ricevuti
dal provider.

Il binario di produzione richiede Linux, UID diverso da root, swap assente e
peer UID 0 sul canale di controllo. Imposta core limit zero, dumpability zero,
no-new-privileges e umask 077. La feature `lab-emulation` consente le prove sul
Mac e il controller di test non privilegiato; non disabilita il controllo dello
swap su Linux. Le protezioni del laboratorio nativo Mac non equivalgono a quelle
della live. Non usare credenziali reali nel laboratorio.

Le credenziali non passano per argv, variabili d'ambiente o file. Il processo
non installa logger, non emette access log e usa messaggi di errore fissi.
I buffer di ingresso delle credenziali sono azzerati al rilascio; non viene
promesso l'azzeramento di tutte le copie interne delle librerie HTTP/TLS o della
RAM. L'assenza di swap e dump e il confinamento del processo restano necessari.
Il servizio confinato è implementato nel [supervisore](GATEWAY-CONTROL.md);
il tracing completo delle scritture delle dipendenze rimane aperto.

I limiti attuali sono 64 connessioni HTTP, quattro richieste attive, 16 MiB per
richiesta, 64 MiB per risposta, 4.096 messaggi e 256 tool/call per array. Gli
header hanno dieci secondi; il body trenta; il provider quindici per la
connessione, 120 per una lettura e 600 totali. Una connessione al guest ha un
limite complessivo di 650 secondi, anche se il client smette di leggere.
Nessuna coda illimitata o riapertura automatica dopo revoca.

## Prove riproducibili

Dal package, sul Mac:

```sh
cargo test --locked --features lab-emulation
cargo clippy --locked --all-targets --features lab-emulation -- -D warnings
cargo fmt --check
```

Dal repository, usando l'immagine Debian del laboratorio già costruita:

```sh
docker build --platform linux/arm64 -f script/oblivion/Dockerfile.vm-lab \
  --target gateway-artifacts \
  --output type=local,dest=artifacts/oblivion-gateway/bundle .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --gateway-bundle artifacts/oblivion-gateway/bundle \
  --output artifacts/oblivion-gateway/run-new
```

Il bundle contiene binario normale, binario di laboratorio e test Rust compilati
con Rust 1.85.1. Il runner ne verifica gli hash e lo inserisce in un mount RAM
readonly, senza filesystem condiviso. Questo esperimento usa solo loopback,
nessun disco dati, nessuna API reale e nessun Tor. I certificati TLS sintetici
sono fidati soltanto nel test interno Rust; non esiste un'opzione del binario per
disattivare TLS o scegliere un'altra CA/origin.

Le [evidenze](gateway-evidence.json) riportano 14 test sul Mac, ripetuti dieci
volte sotto carico, e 19 controlli nel guest Debian. Coprono payload e header,
SOCKS con hostname remoto e credenziale di isolamento, streaming, errori TLS,
redirect, opzioni vietate, limite del body e concorrenza, revoca, disconnessione
del client, cambio token, attivazione tramite descrittori e avvio Linux senza
privilegi. Il provider sintetico usa socket/TLS reali, senza mock del client HTTP.

Docker Desktop aveva swap attivo e il processo ha rifiutato l'avvio: le prove
positive Linux sono state eseguite nella VM senza modificare lo swap di Docker.
Il primo tentativo VM ha passato i test Rust ma fallito un controllo del formato
di `/proc/PID/limits`; la fixture ora interpreta i campi anziché gli spazi finali.

## Prossimi gate

1. Verificare indirizzi interni e rebinding via Tor nel percorso completo;
   [persistenza e bootstrap](TOR-STATE.md) sono integrati nel ciclo del generale.
2. Estendere la revoca nel controller reale a stream attivi, crash spontaneo
   della VM, shutdown globale e guasti I/O.
3. Proxy dei tool con allow/deny/ask, quote e revoca dei tunnel; nessun traffico
   o DNS prima dell'autorizzazione.
4. Adapter reale, catture PCAP/tracing e guasti end-to-end, inclusi risposta
   eccessiva, TLS con hostname errato, client lento, cancellazione e cambio A→B.
5. Abliteration via Tor con una credenziale autorizzata e dati sintetici, poi
   collaudo della live e dell'isolamento su hardware di destinazione.

Le prove del componente non attestano anonimato, assenza universale di leak,
sicurezza della VM gateway o comportamento dei server Abliteration.
