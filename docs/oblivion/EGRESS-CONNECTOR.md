# Connettore Tor e SOCKS5

`oblivion-egress` è il nuovo componente di trasporto, compilato e verificato
separatamente. Il collegamento a supervisore e firewall e il nuovo codice host
per [storage, IPC e selezione nel manager](KEYS-AND-EGRESS.md) sono ora compilati
in [Debian ARM64 tramite CI](linux-ci-evidence.json). Supervisore e firewall
superano inoltre la [matrice con due VM](transport-vm-evidence.json): 12 casi
SOCKS e 9 Tor offline. Il [lifecycle Linux dei volumi](host-lifecycle-ci-evidence.json)
supera 28 controlli e il [percorso SOCKS completo](host-socks-stream-ci-evidence.json)
ne supera 18. La [successiva prova Tor↔SOCKS](host-transport-transition-evidence.json)
ne supera 21: Tor senza bootstrap impedisce di aprire il volume; il ritorno
esplicito al profilo salvato ripristina SOCKS con nuovo token e stream.
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
destinazioni, altre porte, BIND e UDP ASSOCIATE vengono rifiutati. Questo
listener non è un SOCKS generale da esporre alla VM della chat.

È ora verificato nei [test di componente Debian ARM64](tools-proxy-component-evidence.json)
il canale `oblivion-tools`. La
configurazione privata opzionale `tools` contiene l'identità della sessione e
la policy versionata; in sua assenza il ruolo viene rifiutato già durante
l'autenticazione. Le regole si applicano prima del contatto con il proxy:
`allow` inoltra, `ask` e `deny` chiudono senza attendere una conferma ancora non
implementata. I nomi sono canonicalizzati; gli IP pubblici autorizzati usano
i rispettivi tipi SOCKS IPv4/IPv6, sempre nel trasporto scelto. Le regole dei
tool non ampliano il ruolo API, che mantiene origine e porta fisse.

Tor riceve username distinti per API e tool e il token per apertura; un
SOCKS esterno riceve soltanto le proprie credenziali configurate. Questa
distinzione non prova l'isolamento effettivo dei circuiti Tor. Il supervisore
ora accetta `tools` come campo opzionale del grant privato e avvia
`oblivion-tools` solo se presente. Il processo usa UID 1301 e il listener
`10.88.0.1:8112`, ammesso dal firewall solo dalla VM di chat. Riceve via FD
privati il token locale e l'indirizzo del connettore, senza chiave API o
credenziali SOCKS. Il connettore riceve la policy. La fine di qualsiasi processo
del grant chiude l'intera autorizzazione. Avvio parziale, EOF, byte sul canale
e segnali devono rimuovere listener e connessioni. La [prova del servizio](tools-service-evidence.json)
supera 56 controlli in una VM ARM senza swap: 28 test di libreria, dieci di
attivazione, hardening dei binari di produzione e due prove dell'adapter.
Il [percorso completo dei tool](tools-vm-evidence.json) supera quattro casi
VM attraverso supervisore e firewall, con client Python. Il controller host
ora invia `tools` quando esiste una policy salvata o scelta in RAM. La
[prova host](host-policy-evidence.json) supera 33 controlli storage/manager e
23 del ciclo completo, inclusi grant reale e rifiuti dal guest. Senza policy
il servizio rimane spento. Il launcher ora configura il proxy per Bun/webfetch;
la prova del client compilato è preparata ma ancora da eseguire.
Le nuove prove di componente usano socket reali per autorizzazione, indirizzi,
separazione delle credenziali e revoca. Passano tutti i 28 test di libreria;
i risultati precedenti non coprono questa aggiunta.
Oltre agli otto test HTTP/CONNECT, sono aggiunti cinque test del connettore
e due di attivazione del processo: un
profilo privato con 128 regole oltre il precedente limite di 4 KiB e il rifiuto
di frame oltre 128 KiB. La CI ARM ora esegue anche il laboratorio gateway
senza swap e le due prove dell'adapter compilato, oltre a costruire i binari.
Le otto prove precedenti di attivazione e le due nuove del servizio tool ora
passano insieme ai controlli di hardening del binario di produzione. La prima
esecuzione CI era stata rifiutata prima del boot per la scelta di HVF sul
runner Linux; la ripetizione con TCG esplicito è passata.

Il tipo condiviso `oblivion-transport::Transport` distingue:

- `tor`: endpoint fisso `127.0.0.1:9050`, username del servizio e token del grant
  per l'isolamento SOCKS. È il valore predefinito del tipo.
- `socks5`: endpoint IPv4 letterale e porta, con autenticazione opzionale.
  Sono esclusi hostname, IPv6, porta zero, `0.0.0.0/8`, `224.0.0.0/3`, loopback e il
  collegamento interno `10.88.0.0/30`. Un IP LAN esplicito è ammesso dal tipo;
  l'eccezione firewall limitata a quell'IP e porta è verificata nella matrice VM.

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

## Integrazione runtime verificata nel laboratorio

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
La matrice VM verifica separatamente revoca, crash dei due figli e del controller,
perdita del controllo e configurazioni non valide, richiedendo l'arresto dei
processi e la rimozione delle autorizzazioni di rete.

Il nuovo `script/oblivion/run-transport-lab.py` prepara due VM indipendenti e un
proxy SOCKS/TLS sintetico sul loopback dell'host di laboratorio, raggiunto attraverso l'alias
QEMU. Nessuna richiesta viene inoltrata a un provider. L'immagine di laboratorio
aggiunge esclusivamente il certificato pubblico della fixture al broker e
conserva il diff; le immagini di produzione non ricevono quella CA. I test
coprono credenziali e metodi rifiutati, proxy fermo, ripresa, rotazione
del token, stream revocato, crash e tentativo di riconfigurazione. Le catture
esterne controllano anche le coppie IP/porta, oltre ai protocolli.

Il [report precedente](transport-runtime-status.json) distingue:

- controlli statici Rust, Python e shell;
- smoke test del solo server sintetico sul Mac, con curl e verifica TLS;
- regressione del parser su quattro PCAP già conservati;
- build Linux e `bun run check` non completati per errori I/O Docker, con
  circa 340 MiB liberi sul Mac; nessuna nuova esecuzione delle due VM.

La [CI successiva](linux-ci-evidence.json) ha completato build e controlli
del repository su un runner Linux ARM64 indipendente. Il workflow
`.github/workflows/oblivion-check.yml` esegue anche le matrici di guasto SOCKS e
Tor offline con `--accelerator tcg`, conservando report, console e PCAP sintetici.
Il backend resta esplicito: `hvf` sul Mac, `tcg` su Linux, senza fallback.
L'emulazione software non verifica l'isolamento hardware. I bundle hanno
manifest SHA256 e una retention CI di sette giorni; non sono una release firmata.

La [matrice completata](transport-vm-evidence.json), sul commit `30495ba`,
ha 21 casi positivi. Tutti i 12 casi SOCKS verificano stream TLS, autenticazione,
assenza di fallback, revoca/crash e una cattura esterna contenente solo TCP verso
il proxy scelto, senza DNS/UDP/IPv6 né chiavi API o token locali in chiaro.
I 9 casi Tor offline hanno zero pacchetti sull'interfaccia esterna, compresi
startup e tentativi di aggiramento del firewall. Sono verificati UID separati,
capability assenti nei figli, argv/ambiente senza segreti e core dump disabilitati.
Il report conserva anche i due fallimenti iniziali del laboratorio e le correzioni:
compatibilità QEMU e attesa dell'arresto del supervisore prima di verificare
l'assenza del listener dopo un crash. Nessuna correzione al codice di produzione
è stata necessaria per far passare questa matrice.

Prima di eseguire questi comandi **localmente** occorre liberare spazio e verificare che
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
Per il prodotto rimangono regressione Tor online, persistenza delle guardie,
lifecycle host e volumi e proxy dei tool. Finché questi gate restano aperti
non è dichiarato il supporto SOCKS globale.

Il runner aggiunge ora `--tools` per attivare esplicitamente il terzo processo
con una policy sintetica. Passano quattro casi CI: revoca con SOCKS
autenticato e senza autenticazione, crash del proxy tool e crash del connettore.
Le [evidenze](tools-vm-evidence.json), sul commit `1190fcf`, registrano 13
controlli per caso e tutti i 29 job CI verdi. HTTP e HTTPS verificato attraversano
i binari Rust e il firewall; revoca e crash chiudono insieme gli stream API e tool.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 script/oblivion/run-transport-lab.py \
  --image artifacts/oblivion-transport-runtime/image-tools \
  --output artifacts/oblivion-transport-runtime/tools-revoke \
  --authentication required --fault revoke --tools
```

L'immagine deve essere ricostruita dai sorgenti correnti. Il caso verifica
UID/capability/ambiente dei tre processi, `deny`/`ask`/destinazioni sconosciute,
porte e indirizzi privati, token precedente rifiutato, HTTP e CONNECT/TLS
autorizzati e chiusura contemporanea degli stream API e tool. Il certificato
pubblico della fixture viene passato solo al client Python di laboratorio;
il proxy tool non termina TLS. Le catture esterne devono mostrare soltanto il
proxy selezionato, senza DNS/UDP/IPv6 o token locali. La prova usa client Python,
non verifica ancora Bun/OpenCode né la selezione di policy nel manager.

## Integrazione successiva

1. Collaudare il salvataggio nel generale cifrato, i soli metadati nel manager
   e la scelta temporanea in readonly, ora implementati nel sorgente host.
2. Collaudare il passaggio del profilo dall'host e l'avvio Tor condizionale.
   Verificare che SOCKS non importi, acquisisca o salvi stato Tor.
3. Ripetere revoca e arresto attraverso il lifecycle host/LUKS completo;
   richiedere chat e trasferimenti chiusi prima di cambiare modalità.
4. Esercitare provider e Tor reali, preservando lo stato selettivo di Tor.
5. Estendere la stessa scelta al proxy autorizzato dei tool e provarne le
   destinazioni, la policy e i tentativi di aggiramento.
