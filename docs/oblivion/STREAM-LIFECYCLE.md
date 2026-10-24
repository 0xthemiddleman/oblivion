# Stream attivi nel ciclo delle VM

La [suite locale aggiornata](ephemeral-evidence.json) supera **29 controlli**
sul Mac con host HVF e guest TCG: strumenti web compilati, regole A/B,
cattura esterna, revoca degli stream e due sessioni effimere successive.
Nell'effimero i token sono distinti, i dischi RAM vengono rimossi e il
generale resta byte-identico. Il provider rimane sintetico.

La [suite CI x86](host-socks-stream-ci-evidence.json) supera ora 18 controlli
attraverso profilo SOCKS cifrato, client compilato dal commit corrente e due
VM. Il proxy/provider sintetico vive sul loopback dell'host di laboratorio,
esterno al gateway. L'host non ha una NIC: nessun provider reale viene contattato.

La prova storica Mac verifica la revoca mentre il client compilato riceve
una risposta, attraverso controller LUKS, VM della sessione, supervisore e VM
gateway. Il provider è sintetico e vive nella VM gateway; non viene contattato
Abliteration e Tor non trasporta questa prova.

```text
Controller host → grant privato → supervisore gateway → broker
                         ↓
CLI nella VM sessione → token RAM → rete interna → broker
                                                   ↓
                             SOCKS5 + TLS → provider sintetico
```

## Cosa viene verificato

La CLI usa il launcher di produzione e il binario con il profilo Oblivion.
Il provider controlla hostname SOCKS, credenziale di isolamento, chiave API
sintetica, modello, endpoint e streaming. Invia un frammento SSE e mantiene
aperta la risposta senza finish, `[DONE]`, chunk finale o EOF. Il test aspetta
`step_start` dal client e verifica che il provider abbia uno stream attivo
prima di avviare la revoca.

Nella sessione A il controller riceve `close` mentre QEMU è ancora attivo:
revoca il broker, poi rifiuta la chiusura del volume occupato. Il provider
osserva la chiusura della specifica connessione TLS, il client termina con
errore e il test verifica che QEMU e il mapping LUKS siano ancora presenti.
Questo separa la revoca dalla distruzione della VM e dalla chiusura storage.
Solo in seguito `force-leave` chiude entrambi.

Nella sessione B il test interrompe QEMU con SIGKILL mentre un secondo stream
è aperto. Verifica sia la revoca del grant e la rimozione del pidfd osservato
dal servizio host, sia la fine della connessione TLS del provider. Il volume
rimane aperto e impedisce l'ingresso in A fino alla chiusura esplicita. La
chiusura della connessione, da sola, non dimostrerebbe la revoca del grant:
viene controllato anche lo stato del supervisore.

Restano attivi i controlli della [suite di rete](SESSION-NETWORK.md): token
diversi a ogni apertura, rifiuto dei token precedenti, crash e blocco del
servizio host, uscita QMP, scadenza dei frame privati incompleti e scansione
dei file regolari di runtime dopo la chiusura. Il runner cerca inoltre i
marcatori sintetici e le password nel disco di laboratorio. Queste scansioni
non equivalgono a una nuova analisi di ogni scrittura o richiesta di rete.

## Separazione del laboratorio

Le immagini `stream-*` del Dockerfile sono distinte dalle immagini normali.
La sola modifica al broker compilato per questa prova aggiunge il certificato
del provider sintetico alle CA fidate; il diff esatto viene esportato in
`gateway/trust.diff`. Il sorgente del broker nel repository e il suo binario
di produzione non ricevono un'opzione per cambiare CA, origin o verifica TLS.

`vm-stream-cert.pem` e `vm-stream-key.pem` sono materiali pubblici della
fixture, senza valore come credenziali reali. Il servizio Python sostituisce
il listener SOCKS soltanto nell'immagine di laboratorio. Tor conserva il
proprio canale di controllo ma rimane offline. La console diagnostica del
gateway esiste soltanto nel binario host con `lab-emulation`; il binario
normale mantiene la seriale disattivata. Lo stato diagnostico contiene ID
progressivi, esiti e hash, senza richieste o credenziali.

Il runner verifica manifest e immagini prima dell'avvio e il guest ne
ricontrolla gli hash. Il test verifica anche gli hash del broker e del
provider effettivamente avviati. L'host Debian usa HVF sul Mac ARM; le due
VM interne usano TCG. Nessuna cartella del Mac viene montata e la VM esterna
non ha una scheda di rete.

## Riproduzione

La build corrente usa la CLI indicata da `OPENCODE_BINARY` e la provenienza
`OPENCODE_BUILD_EVIDENCE`, verificandone l'hash. Non sostituisce più il client
con il binario ARM storico della prova adapter. Il guest comunica anche l'hash
del client realmente presente prima di accettare i comandi del laboratorio.

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-stream-lifecycle/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
docker build --platform linux/arm64 --target stream-artifacts \
  --build-arg OPENCODE_BUILD_EVIDENCE=artifacts/oblivion-build/build-evidence.json \
  --output type=local,dest=artifacts/oblivion-stream-lifecycle/images-new \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-stream-lifecycle/bundle-new \
  --gateway-image artifacts/oblivion-stream-lifecycle/images-new/gateway \
  --session-image artifacts/oblivion-stream-lifecycle/images-new/session \
  --stream --output artifacts/oblivion-stream-lifecycle/run-new
```

Il [report](stream-lifecycle-evidence.json) conserva hash, controlli e limiti.
Si riferisce alla prova storica; le modifiche al runner non aggiornano da sole
la sua copertura o la versione del binario verificato.
Il campo `opencode_sha256` del riepilogo grezzo del runner descrive la vecchia
immagine host; il report distingue il binario realmente contenuto nella nuova
immagine della sessione, coperto dal suo manifest e dall'hash della build CLI.

## Percorso SOCKS salvato: 18 controlli in CI

La suite x86 aggiunge `--stream --stream-socks`, un host `stream-host` e un
gateway `STREAM_GATEWAY=stream-gateway-base`. Questa variante conserva Tor
offline e non avvia il provider dentro il gateway. Il provider/proxy sintetico
vive nel loopback dell'host Debian di laboratorio, raggiunto dal gateway
tramite l'alias QEMU `10.0.2.2:9050`. L'host esterno non ha una scheda di rete;
la prova non contatta Internet e non usa credenziali reali.

Il controller salva endpoint e credenziali SOCKS nel generale prima di
aggiungere la chiave API. La suite attraversa il client compilato dal commit
CI, la VM di sessione, il broker, il connettore e il proxy esterno. Verifica
stream interrotti dalla revoca e dalla morte della VM, cambio esplicito della
chiave, token nuovi per ogni apertura, rifiuto del cambio trasporto a stream
aperto e ripristino del profilo dopo lock/unlock. Uno stato Tor volutamente
invalido deve rimanere ignorato e invariato durante il percorso SOCKS.

Gli artefatti `stream-results` registrano **18 controlli passati in 439,36
secondi**; anche i 28 controlli host e gli altri job sono verdi. Il
[report](host-socks-stream-ci-evidence.json) verifica hash, provenienza e
copertura e conserva il primo fallimento dello script, corretto prima
dell'esecuzione riuscita. Le suite host e trasporto rimangono evidenze
distinte. KVM accelera l'host di
laboratorio; gateway e sessione continuano a usare TCG, senza attestazione
dell'isolamento KVM di produzione. La variante esterna verifica il manifest
dell'immagine gateway e l'hash della fixture avviata sull'host; non legge un
hash del broker in esecuzione attraverso la console gateway.

Restano aperti provider remoto con Tor reale, shutdown globale durante uno
stream, guasti I/O, limiti della risposta e client lento nel ciclo completo,
proxy dei tool e collaudo su KVM/hardware. La prova non revoca dati già ricevuti
dal provider e non dimostra isolamento di produzione tramite TCG.
