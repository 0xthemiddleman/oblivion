# Più chiavi API e scelta del percorso di rete

Requisiti aggiunti il 24 ottobre 2026: salvare più API key Abliteration e
scegliere tra Tor, predefinito, e un proxy SOCKS5 globale per Oblivion.
Le [chiavi multiple](CREDENTIALS.md) sono implementate nel manager e nel
controller. Il [connettore](EGRESS-CONNECTOR.md), supervisore e firewall ora
superano anche la [matrice con due VM](transport-vm-evidence.json): 12 casi
SOCKS e 9 Tor offline. Storage e manager sono collegati e compilati in
[Debian ARM64 tramite CI](linux-ci-evidence.json), in attesa del collaudo
completo dei nuovi profili con LUKS. Il Docker del Mac restituisce ancora
errori I/O; la CI usa un runner indipendente.

## Chiavi salvate

Il manager fidato gestisce un elenco di chiavi con nome e identificatore opaco:
aggiunta con campo nascosto, rinomina, rimozione confermata e scelta di una
predefinita. Prima dell'apertura, con più chiavi, si sceglie quale usare per quella
sessione. La selezione vale per tutte le chiamate AI, inclusi titoli e
compaction, fino alla chiusura. Non vengono cambiate le password LUKS.

I segreti rimangono nel volume generale, in file root 0600 e directory 0700.
L'IPC del manager espone nomi e stato dopo lo sblocco, mai valori delle chiavi.
Al gateway viene consegnata solo quella selezionata; alla chat resta il token
locale revocabile. L'agente non può elencare o scegliere chiavi. Non si salva
automaticamente un'associazione fra archivio e account: un'eventuale
preferenza per archivio richiederà una decisione esplicita di prodotto.

Cambio di chiave, modifica e rimozione richiedono tutte le sessioni chiuse.
Prima del cambio viene arrestato il gateway che possiede la vecchia chiave.
Se la chiave manca, è rifiutata o esaurisce il credito, il sistema restituisce
un errore: nessuna rotazione automatica, selezione alternativa o nuovo invio
su un altro account. Una chiave diversa non garantisce un'identità diversa
se le chiavi appartengono allo stesso account del provider.

La migrazione da `profile/abliteration.key` avviene alla prima modifica:
il vecchio segreto si elimina solo dopo la pubblicazione durevole del nuovo
archivio. In lettura sola viene presentato come unica chiave disponibile senza
migrazione. Nel profilo in sola lettura si possono scegliere chiavi salvate in RAM;
aggiunte, rinomine, rimozioni e modifica della predefinita non vengono salvate.

## Un percorso globale scelto dall'utente

| Modalità              | Uscita applicativa                                     | Comportamento                                  |
| --------------------- | ------------------------------------------------------ | ---------------------------------------------- |
| Tor, predefinita      | Gateway → Tor → destinazioni autorizzate               | Isolamento dei flussi e stato Tor già previsti |
| SOCKS5 personalizzato | Gateway → proxy configurato → destinazioni autorizzate | Tor non viene avviato per questa modalità      |

Per «globale» si intende il traffico Internet applicativo di Oblivion:
richieste AI, strumenti e futuri servizi di rete e manutenzione. Non si
ereditano le impostazioni proxy del Mac usato per sviluppare. Wi-Fi, DHCP e
raggiungibilità del trasporto sono infrastruttura distinta con eccezioni
ristrette; UDP/QUIC e client incompatibili devono fallire, senza uscita diretta.

Indirizzo, porta, eventuali username/password e modalità scelta si salvano nel
generale cifrato. L'assenza di una scelta usa Tor; configurazione invalida o
proxy indisponibile producono un errore, mai un cambio automatico. Nel
profilo readonly una scelta temporanea resta soltanto in RAM.

La modalità è visibile nel manager. Il cambio richiede chat e trasferimenti
chiusi: revoca, fine verificata delle connessioni e del gateway precedente,
poi avvio del nuovo percorso. Una richiesta interrotta non viene reinviata.
Non sono previste modalità diretta, fallback Tor↔SOCKS o concatenazione
automatica Tor-su-SOCKS.

Le regole consenti/blocca/chiedi restano davanti a entrambi i trasporti.
La VM della chat non accede al proxy grezzo, alle sue credenziali o alla LAN.
Per SOCKS il firewall del gateway consentirà solo all'apposito servizio
l'endpoint configurato, senza l'uscita generale oggi attribuita a Tor.
Un proxy su LAN richiede un'eccezione limitata al suo IP e porta, non l'apertura
della rete locale all'agente.

## Profilo host e manager: codice disponibile, collaudo Linux pendente

Il nuovo profilo versionato `profile/transport.json` contiene modalità,
endpoint IPv4 letterale e autenticazione opzionale, nel generale cifrato.
File root 0600, staging nella stessa directory, `fsync`, rename e sincronizzazione
della directory precedono il riavvio del gateway. Il default è Tor solo quando
il file manca: formato, versione, campi o valori invalidi producono un errore.

`/run/oblivion/transport-selection.json` contiene un'eventuale scelta temporanea
nella tmpfs privata root 0600. Il profilo salvato resta invariato. La scelta
vale fino al blocco del generale e viene rimossa anche allo sblocco successivo;
non è una preferenza associata a una singola chat. Un profilo salvato corrotto
non viene nascosto da una selezione temporanea. File e staging usano gli stessi
controlli anti-symlink/hardlink del controller.

L'operazione IPC privata `egress` accetta un JSON di massimo 4096 byte nel
frame segreto; non usa argomenti o ambiente per le credenziali:

- `save` con `transport`: salva e seleziona il profilo, solo con generale scrivibile.
- `select` con `transport`: selezione temporanea, ammessa anche in readonly.
- `saved`: rimuove la selezione temporanea e ripristina il profilo salvato.

Il controller richiede generale aperto, assenza di recupero, volume della chat
chiuso e processo VM fermo. Arresta il gateway precedente prima di scrivere;
un errore di scrittura non lo riavvia. La configurazione root di laboratorio
`gateway_network: false` rifiuta SOCKS esterni: la scelta utente non può
scavalcare un'immagine configurata offline.

Nel manager fidato, **R** apre la rete: **T** sceglie Tor, **S** configura SOCKS,
**P** ripristina il profilo salvato. Il salvataggio permanente o temporaneo si
sceglie prima dell'applicazione; in readonly si usa soltanto RAM. Username e
password sono campi nascosti; lo stato espone soltanto modalità, endpoint,
presenza di autenticazione e origine temporanea/salvata. La schermata spiega
che l'autenticazione SOCKS standard viaggia in chiaro fino al proxy. La chat
OpenCode non viene modificata.

Il proprietario host invia il trasporto sul canale privato del supervisore.
In modalità SOCKS non legge né importa né salva lo stato Tor. La chiusura usa
`shutdown`; in Tor continua a richiedere il checkpoint prima dell'arresto.
Una chiusura non confermata restituisce ora `gateway_shutdown_unconfirmed`
dopo aver distrutto la VM. Il generale non viene dichiarato bloccato finché
il controller non completa la chiusura del volume.

Il [report host](host-transport-status.json) registra 19 test nativi passati,
compresi sei casi del profilo, più Clippy su tutti i target nativi. Non sono
prove dei rami Linux esclusi da `cfg`. La suite `vm-services-test.py` include
ora menu reale, LUKS, permessi, staging, ENOSPC, blocco a chat aperta e
readonly con confronto byte per byte del volume. La
[verifica Linux](host-transport-ci-evidence.json) supera 15 controlli,
compresi menu, credenziali nascoste, selezione RAM e disco pieno, poi fallisce
aspettando la chat nella VM. I casi readonly e a chat aperta non sono ancora
raggiunti. Restano inoltre da provare il cambio effettivo di gateway
Tor↔SOCKS, stream, persistenza delle guardie e tutte le catture esterne. Il
proxy dei tool rimane da realizzare prima di dichiarare il trasporto globale.

I nomi delle destinazioni vengono passati al proxy dopo l'autorizzazione,
usando l'indirizzamento per dominio di [SOCKS5, RFC 1928](https://www.rfc-editor.org/rfc/rfc1928#section-5).
Il bootstrap dell'endpoint stesso non deve usare DNS di sistema implicito:
la prima implementazione potrà ricevere un IP esplicito; l'accettazione di
hostname richiede prima una procedura di risoluzione amministrata e verificata.

## Differenze da rendere visibili

SOCKS5 non sostituisce TLS applicativo: l'API conserva HTTPS verificato fino
ad Abliteration. L'autenticazione username/password standard invia la password
del proxy in chiaro sul collegamento SOCKS, come specifica
[RFC 1929](https://www.rfc-editor.org/rfc/rfc1929#section-3).
La UI deve descrivere questa proprietà nella configurazione; il salvataggio
cifrato non rende cifrata la connessione al proxy.

Il proxy scelto vede i nomi richiesti e controlla la risoluzione remota. Il
campo `BND.ADDR` di SOCKS5 non attesta l'IP finale della destinazione
([RFC 1928](https://www.rfc-editor.org/rfc/rfc1928#section-6)). La policy locale
può rifiutare IP privati espliciti, ma non promette di verificare tutti gli IP
risolti da un proxy arbitrario. La protezione equivalente contro destinazioni
interne remote richiede controlli sul proxy stesso, da verificare. Le credenziali
del proxy non offrono automaticamente l'isolamento dei circuiti di Tor.

OnionShare usa servizi onion e Tor
([documentazione](https://docs.onionshare.org/2.6/en/security.html)). Nella prima
versione sarà disponibile solo con modalità Tor selezionata; in modalità
SOCKS verrà mostrato come non disponibile, senza avviare Tor di nascosto.

## Passi di implementazione e verifica

1. Estendere storage e IPC host da una chiave a un archivio versionato;
   preservare aggiornamenti atomici, readonly, permessi e recupero. Aggiungere
   selezione e gestione nella TUI fidata, quindi migrare il formato precedente.
   **Implementato**; le evidenze e i limiti sono nella pagina delle credenziali.
2. Consegnare al supervisore la sola chiave scelta e un profilo di trasporto
   tipizzato. Mantenere immutabile la coppia chiave/percorso durante l'apertura.
   La consegna della chiave selezionata è implementata; il profilo di trasporto
   configurabile resta da realizzare.
3. Inserire un connettore locale del gateway per Tor o SOCKS esterno, riusabile
   dal broker e dal futuro proxy dei tool. Separare il token locale e le
   credenziali di isolamento Tor dall'autenticazione del SOCKS esterno.
   Le credenziali esterne non devono essere trasmesse al guest o in argv/log.
   **Componente implementato e verificato separatamente** per il solo origin API;
   integrazione operativa e canale del futuro proxy dei tool ancora aperti.
4. Applicare firewall e lifecycle per modalità: nessuna uscita Tor in SOCKS,
   nessun accesso al SOCKS esterno in Tor, revoca delle connessioni già aperte,
   recupero dopo crash e salvataggio dello stato Tor solo dove pertinente.
5. Provare sul Mac con Debian ARM, LUKS reale e fixture: due chiavi distinguibili,
   selezione e riapertura, errori e assenza di rotazione, migrazione interrotta,
   ENOSPC, readonly, proxy autenticato e non autenticato, auth errata, risoluzione
   per dominio, proxy caduto, cambio percorso e revoca durante lo stream.
   Combinare catture e tracing per dimostrare assenza di fallback, DNS diretto
   e bypass dai sottoprocessi. Confermare poi su KVM e con provider reale.

Questi passi precedono l'estensione del proxy dei tool: quest'ultimo deve
applicare la stessa policy indipendentemente dal trasporto selezionato.
