# Connettore Tor e SOCKS5

`oblivion-egress` è il nuovo componente di trasporto, compilato e verificato
separatamente. Il collegamento a supervisore e firewall è ora scritto, ma
**attende compilazione e collaudo Linux**. Le immagini già verificate usano
ancora il percorso precedente. Il nuovo codice host comprende ora
[storage, IPC e selezione nel manager](KEYS-AND-EGRESS.md), compilati soltanto
nei target nativi disponibili sul Mac. Il lifecycle Linux dei volumi e il
cambio effettivo di percorso rimangono da collaudare.
I componenti della chat OpenCode non cambiano.

## Confini implementati

Il connettore riceve configurazione e token su un socket privato ereditato,
insieme a un listener TCP già aperto su loopback. Il binario di produzione
richiede Linux, utente non root, controller root, swap assente e descrittori
validi prima di inizializzare il runtime. Disabilita core dump, dump del processo
e nuovi privilegi. Non legge proxy da ambiente, argomenti o file.

Ogni istanza appartiene a un solo grant. Il broker deve autenticarsi sul
listener locale con il token di quella apertura. Per ora è autorizzato solo
`api.abliteration.ai:443`, passato come nome al SOCKS: autenticazione e verifica
della destinazione precedono qualsiasi connessione al proxy. IP diretti, altre
destinazioni, altre porte, BIND e UDP ASSOCIATE vengono rifiutati. Il futuro
proxy dei tool richiederà un proprio canale autorizzato; questo listener non è
un SOCKS generale da esporre alla VM della chat.

Il tipo condiviso `oblivion-transport::Transport` distingue:

- `tor`: endpoint fisso `127.0.0.1:9050`, username del servizio e token del grant
  per l'isolamento SOCKS. È il valore predefinito del tipo.
- `socks5`: endpoint IPv4 letterale e porta, con autenticazione opzionale.
  Sono esclusi hostname, IPv6, porta zero, `0.0.0.0/8`, `224.0.0.0/3`, loopback e il
  collegamento interno `10.88.0.0/30`. Un IP LAN esplicito è ammesso dal tipo;
  l'eccezione firewall limitata a quell'IP e porta è scritta e attende collaudo.

Le credenziali del SOCKS esterno sono distinte dal token locale: al proxy
vengono inviate soltanto quelle configurate, oppure nessuna autenticazione.
Un proxy autenticato che seleziona un metodo senza password viene rifiutato;
non si riprova senza autenticazione e non si cambia percorso. Il formato usa
[SOCKS5](https://www.rfc-editor.org/rfc/rfc1928) e
[username/password RFC 1929](https://www.rfc-editor.org/rfc/rfc1929).
Quest'ultimo trasmette la password del proxy in chiaro sul collegamento SOCKS;
il broker conserva la responsabilità di HTTPS e della verifica TLS del provider.

Nome e password del proxy accettano da 1 a 255 byte UTF-8 senza caratteri di
controllo. Il tipo dei metadati pubblici espone modalità, endpoint e presenza
di autenticazione, mai username o password. I tipi con segreti non implementano
`Debug`; input, token, credenziali e messaggio di autenticazione usano `Zeroizing`.
Questo non promette l'azzeramento di ogni copia interna della RAM.

L'handshake completo ha un unico budget di dieci secondi, anche con input
frammentato. Il servizio limita a 64 le connessioni concorrenti. EOF o un byte
sul canale privato, SIGTERM e SIGINT revocano il listener e tutti i task, inclusi
i flussi già aperti. Gli errori delle singole connessioni non vengono stampati;
la diagnostica di avvio è un codice generico senza valori di configurazione.

## Evidenze e limiti

La [suite verificata](egress-connector-evidence.json) usa socket TCP reali e
proxy sintetici. Sul Mac passano 15 test di libreria e 6 di attivazione; i due
casi dell'adapter ignorati nella suite nativa vengono eseguiti nella VM.
In Debian ARM/HVF passano 35 controlli in 13,80 secondi, compresi:

- Proxy con e senza password, dominio remoto, flusso bidirezionale e revoca.
- Rifiuto prima del contatto del proxy per token, destinazione, IP o comando
  non autorizzati; rifiuto del downgrade e degli errori di autenticazione/CONNECT.
- Handshake frammentato con progressi che non rinnovano la scadenza e successivo
  riuso del listener.
- Processo di produzione senza capability, `NoNewPrivs`, core limit zero,
  controller root, rifiuto di root e descrittori mancanti.
- Binario di produzione con ingresso Tor SOCKS sintetico: identità del grant,
  dominio remoto e inoltro dei byte. Non è una connessione a Tor reale.
- Regressioni del broker e dell'adapter compilato, inclusa revoca di uno stream.

I proxy esterni dei test di libreria usano porte loopback dinamiche tramite
costruzione privata nel modulo dei test; il costruttore di produzione rifiuta
quegli endpoint. Non esiste una deroga nel binario distribuito.

La cattura pacchetti di questa suite riguarda **i due test dell'adapter**, non
l'intero nuovo connettore: 770 pacchetti TCP IPv4 loopback, nessun UDP, IPv6,
pacchetto troncato o perso. Sono ancora necessarie catture e tracing sul percorso
SOCKS completo, con firewall e sottoprocessi, prima di dichiarare il supporto
globale. Il laboratorio non ha NIC esterna, non usa provider reale e non
verifica l'isolamento KVM.

## Integrazione runtime scritta, verifica pendente

Il controller privato accetta un trasporto nel messaggio `configure`; se
omesso usa Tor, preservando il protocollo dell'host esistente. La configurazione
resta immutabile per quella VM. Per ogni grant avvia due figli senza capability:
broker API con UID 1300 e connettore con UID 1201. Il broker raggiunge soltanto
il connettore loopback su porta 9051. Password SOCKS e token passano su
descrittori privati, mai su argv, ambiente o file.

Il ruleset parte con tre insiemi vuoti. La configurazione li aggiorna in una
transazione `nft`: Tor abilita UID 1200 verso Internet e UID 1201 verso il Tor
locale; SOCKS abilita UID 1201 soltanto verso IP e porta configurati. Il tipo
`uid` e l'associazione al socket sono descritti nel
[manuale nftables](https://netfilter.org/projects/nftables/manpage.html).
L'eccezione SOCKS precede il blocco LAN, ma riguarda soltanto quel processo e
quell'endpoint. Forwarding, DNS di sistema e IPv6 rimangono bloccati.

Tor parte solo dopo `tor-initialize`, che è rifiutato in modalità SOCKS. Un
file tmpfiles crea la directory RAM vuota necessaria al mount del controller,
senza avviare Tor. L'avvio attende il canale di controllo prima di fermare Tor
per l'importazione dello stato. Revoca e guasti del controller chiudono entrambi
i figli; `ExecStopPost` ferma Tor e ricarica la policy con selettori vuoti.
Questi sono comportamenti previsti dal sorgente, non nuovi risultati di test.

Il nuovo `script/oblivion/run-transport-lab.py` prepara due VM indipendenti e un
proxy SOCKS/TLS sintetico sul loopback del Mac, raggiunto attraverso l'alias
QEMU. Nessuna richiesta viene inoltrata a un provider. L'immagine di laboratorio
aggiunge esclusivamente il certificato pubblico della fixture al broker e
conserva il diff; le immagini di produzione non ricevono quella CA. I test
previsti coprono credenziali e metodi rifiutati, proxy fermo, ripresa, rotazione
del token, stream revocato, crash e tentativo di riconfigurazione. Le catture
esterne controllano anche le coppie IP/porta, oltre ai protocolli.

Il [report corrente](transport-runtime-status.json) distingue:

- controlli statici Rust, Python e shell;
- smoke test del solo server sintetico sul Mac, con curl e verifica TLS;
- regressione del parser su quattro PCAP già conservati;
- build Linux e `bun run check` non completati per errori I/O Docker, con
  circa 340 MiB liberi sul Mac; nessuna nuova esecuzione delle due VM.

Prima di eseguire questi comandi occorre liberare spazio e verificare che
Docker abbia ripreso a leggere e scrivere normalmente. Non è stato eseguito
un reset globale di Docker e non sono stati rimossi dati di altri progetti.
Da root del repository, usare directory di output nuove:

```sh
docker buildx build -f script/oblivion/Dockerfile.vm-lab --platform linux/arm64 \
  --target transport-network-artifacts \
  --output type=local,dest=artifacts/oblivion-transport-runtime/image-02 .
PYTHONDONTWRITEBYTECODE=1 python3 script/oblivion/run-network-lab.py \
  --image artifacts/oblivion-transport-runtime/image-02 \
  --output artifacts/oblivion-transport-runtime/tor-01 --control-fault egress-crash
PYTHONDONTWRITEBYTECODE=1 python3 script/oblivion/run-transport-lab.py \
  --image artifacts/oblivion-transport-runtime/image-02 \
  --output artifacts/oblivion-transport-runtime/socks-auth-01 \
  --authentication required --fault egress-crash
```

Ripetere la suite SOCKS con autenticazione `none` e per i guasti `revoke`,
`broker-crash`, `supervisor-crash`, `control-disconnect` e `reconfigure`.
Eseguire poi regressione Tor online, persistenza delle guardie, lifecycle host
e volumi, controlli Rust Linux e comando canonico del repository. Finché
questi gate restano aperti non è dichiarato il supporto SOCKS globale.

## Integrazione successiva

1. Collaudare il salvataggio nel generale cifrato, i soli metadati nel manager
   e la scelta temporanea in readonly, ora implementati nel sorgente host.
2. Collaudare il passaggio del profilo dall'host e l'avvio Tor condizionale.
   Verificare che SOCKS non importi, acquisisca o salvi stato Tor.
3. Collaudare l'avvio del connettore con UID separato per grant e i soli segreti
   del trasporto. Il broker continua a ricevere chiave API e token locale.
4. Collaudare il firewall per modalità: API e tool soltanto al connettore locale,
   connettore soltanto al Tor locale oppure all'IP/porta del proxy scelto.
   Nessun accesso grezzo da guest e nessuna uscita diretta o DNS di sistema.
5. Collegare revoca e arresto di entrambi i processi al lifecycle verificato
   delle VM; richiedere chat e trasferimenti chiusi prima di cambiare modalità.
6. Esercitare il percorso completo con proxy disponibile, caduto, autenticato,
   credenziali errate e stream attivi, preservando il percorso Tor e la sua
   persistenza. Poi estendere la stessa scelta al futuro proxy dei tool.
