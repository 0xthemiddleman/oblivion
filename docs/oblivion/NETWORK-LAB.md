# Gateway di rete nel laboratorio Mac

24 ottobre 2026. Il gateway ha una prima immagine Debian ARM e una prova con
**due VM indipendenti su Mac/HVF**. Il broker API di produzione gira nella VM
gateway; il client di prova nella VM chat usa soltanto un token locale.
La VM avviata dal controller delle sessioni mantiene ancora `-nic none`:
il [laboratorio host](HOST-GATEWAY.md) ora collega il gateway al generale LUKS,
ma rete della chat e adapter OpenCode rimangono da integrare.
Il [supervisore Rust](GATEWAY-CONTROL.md) riceve la chiave su una porta virtio
privata e gestisce il broker. La fixture Python non riceve più la chiave e non
avvia il processo API. L’immagine finale viene provata senza modifiche ai
sorgenti caricate in RAM.

## Topologia e confini

```text
VM chat                        VM gateway                         Internet
10.88.0.2/30 -- Unix/Ethernet -- 10.88.0.1/30
una sola NIC                   API :8111, UID 1300
                               SOCKS loopback :9050 -> Tor UID 1200 -> TCP
                               10.0.2.15/24 -> QEMU user networking
```

La rete interna è un socket Unix privato QEMU `netdev stream`, senza bridge
verso la LAN o port forwarding. Le VM leggono una base OS immutabile; non
hanno dischi dati, cartelle condivise o clipboard. Il runner Mac conserva solo
artefatti sintetici. La cattura `filter-dump` è esterna ai guest e inizia prima
del boot. Non è un servizio del prodotto. Il collegamento HTTP interno e i
suoi token sono osservabili dall'host fidato: il laboratorio sul Mac non offre
la protezione della futura live avviata al posto dell'OS installato.

| Controllo nel gateway   | Comportamento                                                                                                                                             |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Avvio                   | Entrambe le interfacce restano giù durante il caricamento atomico delle regole nftables; un errore impedisce l'avvio di Tor                               |
| Ingresso dalla chat     | Soltanto `10.88.0.2 -> 10.88.0.1:8111/TCP`; token verificato dal broker                                                                                   |
| Inoltro IP              | Vietato dal firewall, oltre a `ip_forward=0`                                                                                                              |
| IPv6 e UDP              | Bloccati; nessun DNS, QUIC o fallback diretto                                                                                                             |
| API e futuro proxy tool | UID 1300/1301: solo SOCKS loopback; nessuna uscita Internet diretta                                                                                       |
| Tor                     | UID 1200: TCP verso indirizzi pubblici; indirizzi privati, LAN, loopback, link-local, metadata e altri intervalli riservati bloccati anche per questo UID |
| Controllo Tor           | Solo socket Unix e cookie privati; nessun ControlPort TCP                                                                                                 |
| SOCKS Tor               | Solo loopback; `IsolateSOCKSAuth`, rifiuto degli indirizzi interni e DNS remoto                                                                           |
| File e log              | Root readonly, `/var` e `/tmp` in RAM, swap/core disabilitati; output dei servizi non registrato                                                          |

Queste regole sono applicate **fuori dalla VM chat**. La prova modifica il suo
firewall e aggiunge una route predefinita da root. Nel solo laboratorio abilita
anche il forwarding del kernel del gateway, per verificare che il divieto
nftables continui a fermare i pacchetti. Socket canary realmente in ascolto
sul gateway rendono significative le prove negative sulle porte di servizio.
Il gateway e l'host sono domini fidati; non si dichiara resistenza a una loro
compromissione.

Tor parte con `DisableNetwork 1`. Il futuro controller dovrà consegnare lo stato
operativo delle guardie dal generale cifrato prima di autorizzare la rete.
**Il trasferimento e salvataggio di questo stato non sono implementati.**
Il laboratorio abilita Tor su dati usa e getta solo con `--tor-online`; non è
la politica di persistenza del prodotto e non va usato con dati personali.
Il file resolver dell'immagine è vuoto. Bridge con hostname, portali Wi-Fi,
sincronizzazione dell'ora e IPv6 non sono gestiti da questo prototipo.

## Riproduzione

```sh
docker build --platform linux/arm64 \
  --target gateway-network-artifacts \
  --output type=local,dest=artifacts/oblivion-supervisor/image-final \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-network-lab.py \
  --image artifacts/oblivion-supervisor/image-final \
  --output artifacts/oblivion-supervisor/offline-new
python3 script/oblivion/run-network-lab.py \
  --image artifacts/oblivion-supervisor/image-final \
  --output artifacts/oblivion-supervisor/online-new --tor-online
```

Per ripetere i guasti usare `--control-fault` con `disconnect`, `shutdown`,
`malformed`, `oversized`, `partial`, `duplicate-grant`, `supervisor-crash` o
`broker-crash`, scegliendo ogni volta una nuova cartella di output.

Richiede Mac ARM, QEMU con HVF, Python 3 e `bsdtar`. Ogni cartella di output deve
essere nuova. L'immagine `gateway.squashfs` esclude gli strumenti diagnostici;
`laboratory.squashfs` aggiunge Python, curl e il servizio della fixture.
Solo quest'ultima è usata nelle prove descritte qui. I due guest usano 1.5 GiB
di RAM complessivi. I controlli del laboratorio non equivalgono al confinamento
del processo QEMU nell'host Linux di produzione.

Il runner verifica i manifest e la corrispondenza di sorgenti Rust, script,
firewall, Tor e unità di servizio. Per default avvia l’immagine costruita senza
modificarla; un file non allineato richiede una nuova build. L’opzione esplicita
`--overlay`, riservata allo sviluppo, inserisce nell’initrd la fixture e lo script
di rete correnti e una diagnostica degli errori di boot. Il report distingue
questa modalità dalle prove dell’immagine immutata.

La variante offline vieta anche l'uscita nel backend QEMU. La variante online
rimuove quella seconda barriera: è il firewall del gateway a dover bloccare
l'uscita prima di Tor. Quindi verifica il bootstrap, una richiesta HTTPS a
`check.torproject.org/api/ip` con risposta `IsTor: true`, il crash forzato di Tor,
la revoca del broker, il rifiuto del vecchio token e la perdita del gateway.
Non contatta Abliteration e non usa API key reali. Le credenziali sintetiche
vengono create dopo il boot; la chiave del provider è consegnata solo al gateway.
I PCAP interni contengono token di laboratorio: restano in `artifacts/`, non
vengono pubblicati nel repository.

## Evidenze e limiti

Le [evidenze iniziali](network-evidence.json) conservano la prima prova con
launcher Python: 10 controlli offline e 11 con Tor reale. Il nuovo
[report del supervisore](gateway-control-evidence.json) registra le prove con
il binario Rust, il protocollo virtio e gli otto scenari di arresto. Prima di Tor la cattura
esterna è vuota anche nella variante online. Dopo l'attivazione contiene TCP,
senza UDP o IPv6. Questo verifica la matrice sintetica eseguita; non dimostra
assenza universale di leak né rivela da solo le richieste dentro Tor/TLS.

Durante lo sviluppo l'immagine minimale non aveva il comando standalone
`sysctl`: il servizio si è fermato senza aprire le interfacce. Lo script ora usa
l'applet BusyBox già installata. Il primo arresto esplicito di Tor fermava anche
il target di laboratorio tramite le dipendenze systemd; la prova di guasto ora
termina il processo Tor con SIGKILL e verifica l'errore API senza fallback.

Restano da completare:

1. Collegare la rete della chat, assegnare il token locale alla sessione e
   verificare revoca e attesa nel passaggio A→B reale.
2. Persistenza selettiva dello stato Tor e comportamento dell'avvio effimero.
3. Proxy dei tool con consenti/blocca/chiedi e revoca dei tunnel.
4. Adapter OpenCode, streaming e tool calling attraverso questo broker;
   Abliteration reale con credenziale di prova autorizzata.
5. Tracing per processo e filesystem, rebinding, indirizzi privati via Tor,
   guasti sotto carico e blocco globale, integrazione nell'immagine live.
6. Boot e isolamento su KVM/hardware x86 e revisione indipendente.

Riferimenti di implementazione: [QEMU, network backends](https://www.qemu.org/docs/master/system/invocation.html),
[manuale Tor Debian](https://manpages.debian.org/trixie/tor/tor.1.en.html),
[nftables, metadati dei pacchetti](https://wiki.nftables.org/wiki-nftables/index.php/Matching_packet_metainformation).
