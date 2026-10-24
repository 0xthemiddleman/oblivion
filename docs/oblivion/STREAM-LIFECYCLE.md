# Stream attivi nel ciclo delle VM

Il laboratorio Mac ora verifica la revoca mentre il client compilato riceve
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

Serve la CLI Linux ARM della [prova adapter](ADAPTER-BROKER.md), nella posizione
indicata dal Dockerfile e con l'hash del relativo report.

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-stream-lifecycle/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
docker build --platform linux/arm64 --target stream-artifacts \
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
Il campo `opencode_sha256` del riepilogo grezzo del runner descrive la vecchia
immagine host; il report distingue il binario realmente contenuto nella nuova
immagine della sessione, coperto dal suo manifest e dall'hash della build CLI.

Restano aperti provider remoto con Tor reale, shutdown globale durante uno
stream, guasti I/O, limiti della risposta e client lento nel ciclo completo,
proxy dei tool e collaudo su KVM/hardware. La prova non revoca dati già ricevuti
dal provider e non dimostra isolamento di produzione tramite TCG.
