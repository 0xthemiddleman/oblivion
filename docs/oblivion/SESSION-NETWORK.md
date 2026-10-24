# Rete e token della singola sessione

Il controller collega la VM della sessione al gateway tramite un socket Ethernet
punto a punto. Genera 32 byte casuali e avvia QEMU con il guest ancora sospeso.
Collega l’autorizzazione a quel processo, avvia il broker e consegna al guest
soltanto il token locale esadecimale.
La chiave Abliteration rimane nel profilo generale e nella VM gateway.

## Apertura e chiusura

1. `enter` richiede un gateway attivo e una sola sessione aperta. Con rete
   abilitata verifica prima bootstrap Tor e acquisizione delle guardie;
   durante l’attesa il volume rimane chiuso. Dopo lo sblocco
   del volume genera un token nuovo, anche quando si riapre lo stesso archivio.
2. Un socket host root-only in `/run/oblivion/gateway-control.sock` accetta solo
   `check`, `grant`, `revoke` e `stop`. Il servizio controlla UID 0 tramite `SO_PEERCRED`, valida
   frame e richieste e attende la risposta del supervisore nel gateway.
3. QEMU della sessione parte sospeso. Il controller collega il canale virtio
   `oblivion.session` e verifica UID 1100. Il servizio gateway acquisisce il
   pidfd del processo tramite un breve collegamento alla console e autorizza
   il broker. Solo dopo la conferma il controller avvia il guest, aspetta che
   apra la porta, invia il token e attende la conferma della configurazione.
4. Il guest salva il token in `/run/oblivion-api/token`, root:1000, 0640,
   directory 0750 in RAM. Configura soltanto `10.88.0.2/30`, senza default route,
   DHCP o resolver esterno. IPv6 viene disabilitato.
5. `leave`, `force-leave` e `close` revocano il broker prima di chiudere il disco.
   La risposta arriva dopo la fine del processo API. Se il proprietario host
   non risponde, il controller distrugge il gateway e ne verifica l'arresto.
   `lock` e `recover` chiedono anche il checkpoint Tor finale e fermano
   l’intera VM gateway prima di chiudere il generale.

Un avvio fallito revoca il grant; il volume rimane interbloccato fino alla
chiusura esplicita. Un gateway fermo impedisce una nuova apertura con rete.
Non viene avviato automaticamente un collegamento alternativo.

La consegna del token ha una [scadenza complessiva di 60 secondi](CHANNEL-DEADLINES.md),
che comprende ripresa del guest, richiesta della porta, invio e conferma.
I frammenti non rinnovano il tempo disponibile. QMP ha un proprio limite di
5 secondi per greeting, capabilities e comando, compresi gli eventi intermedi.

## Uscita imprevista della VM

Il servizio host conserva un pidfd del processo QEMU durante l'autorizzazione.
Lo ottiene con `SO_PEERPIDFD` dal socket Unix della console, dopo il controllo
UID 1100, mentre il guest è sospeso. Chiude subito il collegamento alla console
per lasciarla al manager. Non accetta un PID numerico fornito dal guest e non
aggiunge dispositivi virtuali, file di stato, privilegi o processi di servizio.

Il riferimento del kernel rimane legato al processo originale anche quando
quel numero di PID può essere riutilizzato. `poll` segnala la fine del processo;
il servizio revoca il broker, attende la conferma e chiude il pidfd. Una revoca
normale elimina il riferimento prima dell'arresto di QEMU. Se il canale di
controllo fallisce, l'uscita del proprietario distrugge l'intera VM gateway.
Il generale e le guardie Tor possono rimanere aperti dopo una revoca riuscita.
Vedere l'[introduzione di SO_PEERPIDFD nel kernel Linux](https://github.com/torvalds/linux/commit/7b26952a91cf65ff1cc867a2382a8964d8c0ee7d)
e il [comportamento di polling dei pidfd](https://man7.org/linux/man-pages/man2/pidfd_open.2.html).

Il controllo avviene nel ciclo del servizio host, con intervallo ordinario di
250 ms e tempi aggiuntivi per le operazioni IPC in corso. Non è una promessa
di revoca istantanea. Il requisito è Linux 6.5 o successivo; il laboratorio
Debian usa 6.12. Se non può ottenere il pidfd, l'avvio fallisce prima della
consegna del token. Non c'è un ripiego sul solo numero di PID.

L'osservatore non dichiara il volume chiuso e non opera su cryptsetup. Il manager
esegue la normale chiusura quando termina la console; usando direttamente il
controller, occorre ancora `close`/`lock`. Finché il mapping esiste, una nuova
sessione resta interbloccata. L'uscita di QEMU non dimostra un flush ordinato
del filesystem del guest. Un guest bloccato con QEMU ancora vivo richiede il
percorso separato di arresto e recupero.

## Confini

La VM della chat mantiene `PrivateNetwork=yes` e `AF_UNIX` per il processo QEMU.
Il solo socket Ethernet del gateway viene montato nella root di QEMU; i socket
amministrativi restano assenti. La directory del gateway rimane 0700 e il
socket Ethernet concede accesso al gruppo della VM della sessione. Nessun
filesystem di documenti, profilo o vault viene condiviso.

Il guest usa una NIC con MAC fisso `52:54:00:6f:00:03`. La VM gateway applica il
firewall già verificato nel [laboratorio di rete](NETWORK-LAB.md): il guest può
raggiungere il broker `10.88.0.1:8111`, senza accesso diretto al SOCKS Tor o
forwarding verso Internet. Il collegamento Ethernet non si riconnette da solo
quando la VM gateway scompare.

Il token locale è intenzionalmente leggibile dai tool della sessione: è una
capacità revocabile, non la credenziale del provider. Il launcher
`guest/conversation.sh` lo passa come `ABLIT_KEY` e imposta l'endpoint del broker
per l'adapter OpenAI-compatible già presente. Usa l'istanza standalone e i
percorsi HOME/XDG del volume della sessione. Nessun token è negli argomenti o
nell'ambiente dei processi host/QEMU; dentro il guest è nell'ambiente OpenCode.

Con `gateway_network: false`, il default per il laboratorio offline, una
richiesta corretta raggiunge il broker e riceve `provider_unavailable`.
L’[avvio online](TOR-STATE.md) importa lo stato selettivo di Tor prima di
abilitare la rete e ammette nuove sessioni solo dopo il bootstrap. Restano
proxy dei tool e compatibilità reale con Abliteration. Il launcher è disponibile nello stage `conversation-guest`;
la live completa non esiste ancora.

## Laboratorio Mac

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-session-network/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
docker build --platform linux/arm64 --target session-network-artifacts \
  --output type=local,dest=artifacts/oblivion-session-network/guest-new \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-session-network/bundle-new \
  --gateway-image artifacts/oblivion-tor-state/image-03 \
  --session-image artifacts/oblivion-session-network/guest-new \
  --output artifacts/oblivion-session-network/run-new
```

Le prove usano LUKS, systemd e due guest TCG dentro l'host Debian avviato con HVF
sul Mac. L'host esterno non ha NIC. La root del gateway è quella di produzione;
nel guest della sessione una fixture UID 1000 sostituisce l'entrypoint interattivo
per interrogare il broker e avviare il vero OpenCode tramite il launcher.
Token e chiavi sono sintetici; i token scambiati sulla console di laboratorio
rimangono nella RAM della fixture e non vengono stampati nel log esterno.

Il [report iniziale delle prove](session-network-evidence.json) registra 8 controlli
della nuova suite, 10 del lifecycle del gateway e 19 della regressione
storage/manager, tutti passati con lo stesso bundle host. Il vero adapter
raggiunge il broker e riceve l'errore atteso `provider_unavailable`; il token
non compare nell'output del client o nei file regolari del vault ispezionati.
Il caso con proprietario host sospeso verifica che la mancata conferma di
revoca provochi la distruzione del gateway prima della chiusura del volume.

Le verifiche di questa suite riguardano autenticazione, rotazione A→B→A,
revoca, percorsi di rete negati e chiusura. Non provano cancellazione fisica della
RAM, anonimato, compatibilità completa di streaming/tool calling, shutdown su
hardware, resistenza a guasti I/O o assenza universale di scritture. Il
laboratorio di lifecycle mantiene anche una prova del vecchio guest privo del
ricevitore: l'avvio deve fallire e richiedere chiusura esplicita.

La [prova della revoca all'uscita](session-exit-evidence.json) passa nove controlli
con il nuovo bundle, più dieci di regressione del ciclo del gateway. Ispeziona il vero
pidfd nel processo host, ne confronta il PID con QEMU, termina QEMU con SIGKILL
e verifica la revoca senza invocare prima `close`. Verifica anche l'uscita
normale del processo tramite QMP `quit`, la chiusura del pidfd e il mantenimento
dell'interblocco del volume. Sono prove offline con lo stesso broker reale;
streaming del provider e shutdown hardware restano separati.

La [prova degli stream attivi](STREAM-LIFECYCLE.md) estende questa suite con
un provider SOCKS/TLS sintetico nella VM gateway e il vero client compilato.
Verifica la fine della connessione prima della chiusura LUKS e dopo SIGKILL
della VM della chat, insieme alla revoca del grant e al blocco del volume.
