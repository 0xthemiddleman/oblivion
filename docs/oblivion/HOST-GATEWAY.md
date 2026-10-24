# Gateway collegato al volume generale

Il controller Linux ora può avviare una VM gateway dopo lo sblocco del generale
e consegnarle la chiave salvata nel profilo. Il collegamento usa il
[protocollo privato del supervisore](GATEWAY-CONTROL.md). La chat rimane ancora
senza rete: assegnazione del token alla sessione, adapter e stato persistente
Tor devono essere collegati prima di abilitare le richieste al provider.

## Ciclo di vita

- Generale chiuso: il gateway non viene avviato.
- Sblocco senza chiave salvata: il manager rimane disponibile, gateway fermo.
- Sblocco con chiave, anche readonly: parte `oblivion-gateway-host.service`,
  che legge il file root 0600 nel generale e avvia la VM gateway.
- Salvataggio o sostituzione della chiave: richiede tutte le sessioni chiuse;
  ferma il vecchio gateway prima dell'aggiornamento atomico e avvia una VM nuova.
- Rimozione della chiave: ferma prima il gateway, poi cancella il file.
- Chiusura di una sessione: il gateway resta disponibile per il profilo.
- Blocco e recovery: fermano il proprietario del canale e verificano la fine
  della VM gateway prima di chiudere sessione e generale.

Il profilo readonly fornisce la credenziale già salvata senza aggiornarla.
Questo è il riuso del profilo per il futuro avvio effimero, non ancora una
sessione effimera completa. Un errore di avvio non cancella una chiave appena
salvata e non dichiara chiuso il generale. La TUI mostra lo stato del gateway
e permette di bloccare e riprovare.

## Processi e dati

Le operazioni storage sono processi brevi. Il servizio host dedicato conserva
invece il canale virtio, controlla lo stato ogni 250 ms e termina se la VM non
risponde entro il timeout. Notifica `READY=1` a systemd solo dopo la configurazione
privata confermata dal guest. Non espone una porta di amministrazione alla chat.

Il servizio host gira come root con la sola `CAP_DAC_OVERRIDE`, necessaria per
connettersi ai socket QEMU di UID 1101. Il generale è readonly anche nel suo
namespace. Dopo l'invio, i buffer espliciti della chiave vengono rilasciati con
`Zeroizing`; la copia operativa rimane nel gateway. Non si promette cancellazione
completa di ogni copia interna alla RAM.

La VM gateway usa **UID/GID 1101**, mentre la sessione usa **1100**. Il suo
`RootDirectory` contiene solo QEMU e librerie pubbliche; root guest, kernel e
initrd sono bind readonly. Non riceve il dispositivo della sessione, il generale
o una cartella condivisa. Il runtime in `/run` e `/tmp` sono in RAM;
core dump, swap del cgroup e capability sono disabilitati. Il processo QEMU
usa sandbox, limiti di memoria/processi e un runtime privato 0700.

QEMU parte sospeso con `-S`. L'host verifica il peer Unix UID 1101, collega la
porta privata e solo allora riprende il guest via QMP. Le credenziali non
compaiono in argv o ambiente. Le NIC hanno MAC fissi e nessuna ROM di boot PXE;
il kernel viene caricato dall'immagine locale verificata nel laboratorio.

Il servizio ora riceve `grant` e `revoke` dai worker storage tramite un socket
host riservato a UID 0; il [collegamento della sessione](SESSION-NETWORK.md)
consegna il token locale al guest. Tor resta disabilitato. Lo stato restituito
dal supervisore deve corrispondere a quello atteso dal proprietario host.
`BindsTo` ed `ExecStopPost` fanno terminare la VM alla morte del proprietario;
`lock` verifica anche la VM direttamente prima di chiudere i mapping.
Non c'è riavvio automatico del gateway con credenziali ancora in memoria.

## Configurazione e laboratorio Mac

Il campo opzionale `gateway` del file root del controller usa lo stesso schema
di immagini/risorse di `vm`, ma seleziona il ruolo gateway. In produzione
l'acceleratore è KVM; TCG è accettato solo dal binario `lab-emulation`.
L'immagine host deve includere l'account 1101, l'unità host e il mount target
`run/oblivion-gateway-vm` nella root pubblica di QEMU.

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-gateway-host/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-gateway-host/bundle-new \
  --gateway-image artifacts/oblivion-supervisor/image-final \
  --output artifacts/oblivion-gateway-host/run-new
```

La fixture usa il vero controller, cryptsetup, systemd e la root di produzione
del gateway, con segreti sintetici. Adatta in RAM la vecchia immagine host:
aggiunge l'account dedicato e il mount target, copia i soli binari pubblici di
QEMU in un tmpfs eseguibile readonly e carica kernel/initrd/root del gateway
verificandone gli hash. Nel laboratorio abilita stderr volatile per diagnostica;
la configurazione di produzione rimane silenziosa. Non è una build live finale.

Il report iniziale registra dieci controlli, inclusi sostituzione della VM, errore di avvio,
morte del proprietario, guest non responsivo, arresto del vero controller,
assenza di marcatori sul disco esterno e profilo readonly invariato byte per byte.
Il report iniziale include inoltre i 19 controlli della suite precedente con
lo stesso bundle host. La fixture attuale verifica il fallimento controllato
del vecchio guest, che non dispone del nuovo ricevitore del token; il ciclo
con rete è verificato dalla nuova suite dedicata.
Durante lo sviluppo è
emersa un’attesa circolare: `ExecStopPost` aspettava una VM ordinata dopo
l’arresto del proprietario. Il gateway ora usa `BindsTo` senza quell’ordine,
mentre la VM della sessione conserva la sequenza di shutdown precedente.
Per il test del controller la fixture disabilita soltanto l'avvio del manager
dipendente, così mantiene il possesso della console; la suite precedente copre
il manager reale. La scomparsa del processo QEMU prova la fine della VM,
non la cancellazione fisica di ogni copia dalla RAM del computer Mac.

I risultati e i tentativi falliti sono in [host-gateway-evidence.json](host-gateway-evidence.json).
Rete della sessione, token locali e revoca nel passaggio A→B sono descritti in
[SESSION-NETWORK.md](SESSION-NETWORK.md). Restano stato Tor, attivazione della
rete e compatibilità completa dell’adapter. Le prove su KVM e hardware, shutdown
globale, guasti I/O e revisione del confine privilegiato rimangono gate aperti.
