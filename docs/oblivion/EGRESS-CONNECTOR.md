# Connettore Tor e SOCKS5

`oblivion-egress` è il nuovo componente di trasporto, compilato e verificato
separatamente. **Non è ancora collegato al supervisore o alle sessioni**:
il broker distribuito continua a usare Tor locale direttamente. La scelta
SOCKS globale nella TUI richiede ancora storage host, IPC, firewall e lifecycle.
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
  l'eccezione firewall limitata a quell'IP e porta resta da integrare.

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

## Integrazione successiva

1. Salvare modalità, endpoint e autenticazione nel generale cifrato; mostrare
   soltanto metadati nel manager e permettere la scelta temporanea in readonly.
2. Trasmettere il profilo al supervisore; avviare Tor solo in modalità Tor.
   In SOCKS non importare, acquisire o salvare stato Tor.
3. Avviare il connettore con UID separato per grant e consegnargli i soli segreti
   del trasporto. Il broker continua a ricevere chiave API e token locale.
4. Applicare firewall per modalità: API e tool soltanto al connettore locale,
   connettore soltanto al Tor locale oppure all'IP/porta del proxy scelto.
   Nessun accesso grezzo da guest e nessuna uscita diretta o DNS di sistema.
5. Collegare revoca e arresto di entrambi i processi al lifecycle verificato
   delle VM; richiedere chat e trasferimenti chiusi prima di cambiare modalità.
6. Esercitare il percorso completo con proxy disponibile, caduto, autenticato,
   credenziali errate e stream attivi, preservando il percorso Tor e la sua
   persistenza. Poi estendere la stessa scelta al futuro proxy dei tool.
