# Servizi del sistema live e recupero

Lo sviluppo e il collaudo avvengono sul Mac ARM, con un host Debian in QEMU/HVF
e VM di sessione in TCG. Le unità in `packages/oblivion-host/host` sono componenti
del futuro sistema live; non installano servizi sul Mac e non preparano una USB.

Il ciclo finale ha superato **8 controlli in 141,78 secondi**, dopo una prima
esecuzione riuscita con 7 controlli. Le [evidenze](services-evidence.json) includono
hash di sorgenti, binari, unità, initramfs e fixture effettivamente eseguiti.

## Responsabilità

| Servizio              | Identità e funzione                                                                                                                               |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `oblivion-controller` | Root, socket amministrativo, cryptsetup, mount e ciclo VM. Annuncia la disponibilità solo dopo aver preparato il socket.                          |
| `oblivion-manager`    | UID/GID 1000 dedicato, console fidata, password nascoste e richieste IPC. Nessuna capability o rete IP, filesystem in sola lettura e home in RAM. |
| `oblivion-recover`    | Root, tentativo di spegnimento ordinato e chiusura verificata di entrambi i mapping.                                                              |
| `oblivion-session`    | UID/GID 1100, servizio QEMU transitorio con il solo dispositivo della sessione corrente.                                                          |

Il manager dipende dal controller e attende il tentativo di recupero prima di
partire. Un'uscita, anche anomala, accoda il recupero; il riavvio torna alla
schermata bloccata solo se la chiusura è stata verificata. `Q` blocca tutto e il
servizio riapre il manager, senza lasciare una shell Linux. Il controller esegue
il recupero anche prima dell'avvio e dopo l'arresto dei suoi processi figli.

La coda di recupero usa `systemctl --no-block` nel post-arresto del manager:
attendere lì un servizio ordinato prima del manager creerebbe una dipendenza
circolare. Il riavvio attende invece il servizio separato. Le semantiche di
`ExecStopPost` e ordinamento sono quelle di
[systemd.service](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html).

Il controller mantiene il namespace dei mount dell'host: un mount privato
renderebbe invisibile il generale agli altri processi di recupero. È un confine
privilegiato esplicito, non un processo completamente confinato. Le restrizioni
del manager e di QEMU non vengono estese alla cieca a cryptsetup e mount.

## Una chiusura fallita rimane visibile

L'operazione interna root `recover` crea `/run/oblivion/recovery-required` prima
di tentare il lock delle operazioni. Il file, senza contenuti privati, appartiene
a root su tmpfs e sopravvive al riavvio dei servizi nello stesso boot. Anche se
un worker sta completando un'operazione, una successiva apertura viene rifiutata.

Il controller prova lo spegnimento ordinato, attende QEMU, chiude il mapping della
sessione, smonta il generale e chiude il suo mapping. Solo dopo queste verifiche
rimuove il marcatore. Un errore, timeout o dispositivo occupato lo conserva.
`status` espone `recovery_required`; i nuovi sblocchi e le creazioni sono vietati
anche chiamando direttamente il controller. Il manager mostra **RECUPERO
NECESSARIO**, consente di riprovare il blocco e richiede due pressioni distinte
di F per fermare forzatamente la VM e bloccare entrambi i volumi.

Non ci sono chiusure deferred, unmount lazy o arresti forzati automatici nel
percorso di recupero. Lo spegnimento finale dell'intero sistema rimane soggetto
alla politica di arresto di systemd: la sua verifica end-to-end è un gate distinto.
Il marcatore scompare al reboot perché allora non esistono più i mapping del
kernel precedente; non è una prova di integrità dei filesystem dopo power loss.

## Integrazione nell'immagine

I binari vanno in `/usr/bin`, le unità nella directory dei servizi di sistema,
la configurazione root `0600` in `/etc/oblivion/controller.json`. L'immagine deve
riservare UID/GID 1000 al manager, senza login interattivo, e 1100 a QEMU. Non
installare queste unità su un computer multiutente riutilizzando UID esistenti.

L'esempio configura `/persist/oblivion/general.luks`: quel percorso deve risiedere
sulla partizione persistente prevista dal provisioning. `/var` della live è in
RAM e non è una destinazione corretta per l'archivio da conservare. La preparazione
di partizioni, avvio e primo volume non è implementata da queste unità.

La console prevista è `/dev/tty1`, esclusa da getty. Il manager usa reset e pulizia
della console fra avvii, secondo
[systemd.exec](https://www.freedesktop.org/software/systemd/man/258/systemd.exec.html).
La verifica sulla console fisica, inclusi scrollback e cambio VT, resta aperta.

## Laboratorio sul Mac

Preparare l'immagine TUI descritta in [MANAGER.md](MANAGER.md), poi compilare il
piccolo bundle dei componenti host:

```sh
docker build --platform linux/arm64 -f script/oblivion/Dockerfile.vm-lab \
  --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-services/bundle .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-services/bundle \
  --output artifacts/oblivion-services/my-new-run
```

Il runner aggiunge binari, unità e test all'initramfs; il bundle eseguibile occupa
un tmpfs dedicato, reso in sola lettura prima dell'avvio. Non condivide directory
del Mac. Le evidenze distinguono il manifest dell'immagine base dal bundle
effettivamente eseguito e ne verificano gli hash dentro la VM.

Il test carica le unità reali in `/run/systemd/system` e cambia soltanto percorso
di configurazione, binario TCG esplicito e console PTY. Esegue SIGKILL sui processi
di manager/controller, SIGSTOP su QEMU, annullamento e conferma del recupero, e
arresto del controller. Tutte le password sono sintetiche e pubbliche.
Verifica anche il marcatore quando il lock delle operazioni è occupato e il
rifiuto via IPC di una nuova creazione durante il recupero. Nessuna password è
stata osservata nell'output della PTY e nessun marcatore sintetico è stato trovato
nel disco spento. Nell'immagine precedente l'account del manager è aggiunto in
RAM con gli stessi dati pubblici ora presenti nel Dockerfile del laboratorio.

Rimangono da collaudare interruzione durante creazione/format, esaurimento disco,
OOM, spegnimento completo, power loss, console fisica e Linux/KVM. Una prova con
TCG non certifica l'isolamento del prodotto.
