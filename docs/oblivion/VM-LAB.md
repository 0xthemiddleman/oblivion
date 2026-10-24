# Laboratorio delle VM di sessione

24 ottobre 2026. Il controller in [oblivion-host](../../packages/oblivion-host/README.md)
avvia e arresta ora una VM con il solo disco della sessione aperta. Il ciclo
A → B → A è stato eseguito con systemd, QEMU e LUKS reali, usando documenti e
password sintetici. Il [riepilogo verificato](vm-evidence.json) collega le prove
agli hash dei sorgenti e delle immagini.

La prova iniziale con fixture shell ha superato **11 controlli**, compresi IPC da UID non
privilegiato e VM non responsiva. Nessuno dei marcatori o delle password
sintetiche cercati è stato trovato in chiaro sul disco spento. Il laboratorio
storage separato ha superato 17 controlli; `bun run check` ha superato i 34 task
upstream. Compilazione e formattazione Rust sono state verificate separatamente.

**Questo laboratorio prova il funzionamento, non certifica l'isolamento KVM.**
Sul Mac ARM disponibile, la VM Debian esterna usa HVF e le VM interne usano TCG.
L'emulazione è abilitata soltanto nel binario compilato con `lab-emulation`.
Il binario normale rifiuta quella configurazione e richiede KVM prima di aprire
un volume di sessione. QEMU non considera TCG una garanzia di isolamento per
guest ostili; il collaudo x86/KVM resta un gate aperto
([modello di sicurezza QEMU](https://www.qemu.org/docs/master/system/security.html)).

## Confini implementati

- Il controller monta il generale e apre il volume LUKS interno; il filesystem
  della sessione viene montato **soltanto nel guest**.
- QEMU gira con UID/GID 1100, senza capability, in un servizio systemd transitorio.
  Ha root e namespace di mount/rete propri, `/dev` privato, cgroup limitato e
  filtro seccomp QEMU. La root contiene solo eseguibili e librerie pubblici.
- Il guest riceve kernel/initramfs, un sistema SquashFS di sola lettura e il disco
  attivo. Nessun 9p, virtiofs, cartella host, clipboard o scheda di rete.
  Il mapping del generale e i file degli altri vault non compaiono nella root
  del processo QEMU.
- Nel guest, UID 1000 lavora in `/vault/workspace`; home, stato e configurazione
  sono nello stesso vault. `/tmp`, `/var` e quindi `/var/tmp` sono in RAM. Niente
  swap, ibernazione, dump della memoria guest o storage journald.
- Il comando di chiusura chiede lo spegnimento attraverso QMP, aspetta l'uscita
  di QEMU e soltanto dopo rimuove il mapping. Se il guest non risponde, restituisce
  un errore e mantiene l'interblocco. La chiusura forzata è un comando distinto.
- L'IPC del manager apre e chiude l'intero ciclo VM; non espone la primitiva root
  che apre soltanto il blocco. Il socket di controllo non viene passato al guest.

La variante iniziale del guest usa una fixture shell. La nuova variante
`opencode-guest` esegue anche il **binario OpenCode reale con profilo privacy**:
la CLI e il server lavorano nel guest con un provider sintetico su loopback.
La console privata è un socket Unix; il bridge e la TUI fidata per l'utente non
sono ancora implementati. Non esiste un'immagine installabile di prodotto.

## OpenCode nel guest sul Mac ARM

La [prova con OpenCode](chat-vm-evidence.json) ha superato **14 controlli in
112,60 secondi**. Collega l'immagine al medesimo hash del binario già verificato
nel [profilo privacy](PRIVACY-PROFILE.md); la build rifiuta un binario che non
corrisponde all'evidenza. Tutte le credenziali e i contenuti sono sintetici.

1. In A, OpenCode riceve un documento come contesto, usa lo streaming nativo
   Abliteration e avvia la shell per creare un documento in `/vault/workspace`
   e temporanei in `/tmp` e `/var/tmp`.
2. Il controller spegne A e chiude il suo mapping. B parte con un nuovo guest:
   non trova il database, il documento, il contesto o i temporanei di A.
3. Riaprendo A, OpenCode riprende lo stesso Session ID. Il provider sintetico
   riceve la cronologia precedente, compresi risposta e risultato del tool;
   il documento persiste e i temporanei non esistono più.
4. Restano verificati il rifiuto di aperture simultanee, i privilegi limitati,
   la VM non responsiva, la chiusura forzata esplicita e il blocco generale.

Nessuno dei marcatori cercati è stato trovato in chiaro nel disco spento. I test
non dimostrano cancellazione fisica della RAM, assenza di ogni rappresentazione
possibile dei dati o resistenza di TCG a guest ostili. Non è ancora un test della
TUI, di prompt interrotti durante una scrittura o della compatibilità Tor reale.
La chiave API del fixture è nell'ambiente del guest: il broker esterno rimane
necessario prima di usare credenziali reali.

## Riproduzione

Prerequisiti attuali: Mac ARM64, Docker e QEMU con HVF. Dal repository:

```sh
docker build --file script/oblivion/Dockerfile.vm-lab --target artifacts \
  --output type=local,dest=artifacts/oblivion-vm/image .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-vm/image \
  --output artifacts/oblivion-vm/my-new-run
```

Per la variante con OpenCode, compilare prima il binario come descritto nel
[profilo privacy](PRIVACY-PROFILE.md), poi:

```sh
docker build --file script/oblivion/Dockerfile.vm-lab \
  --build-arg GUEST=opencode-guest --target artifacts \
  --output type=local,dest=artifacts/oblivion-chat-vm/image .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-chat-vm/image \
  --output artifacts/oblivion-chat-vm/my-new-run
```

L'evidenza include l'hash del binario: per una nuova compilazione che produce un
hash diverso, rieseguire prima le prove privacy e aggiornare il loro riepilogo
solo dopo averne verificato il risultato. La VM esterna usa 4 GiB; il guest
OpenCode 1,5 GiB e 2 vCPU. La variante shell usa 512 MiB e 1 vCPU. Nessun
filesystem è condiviso con il Mac o fra le VM. La rete del guest resta senza NIC;
il provider di prova vive nel guest stesso.

Il runner richiede una directory nuova e crea un disco virtuale vuoto da 1 GiB.
L'init del laboratorio formatta **solo il disco virtuale previsto dal runner**:
non eseguire gli script guest/host sul proprio sistema e non collegare dischi
fisici alla VM. Nessuna rete, cartella o credenziale reale viene passata alle
prove. La build scarica le dipendenze; l'esecuzione è offline.

`serial.log`, `data.raw` e `summary.json` rimangono in `artifacts/`, escluso da Git.
Le diagnostiche dettagliate sono proprie della fixture, contengono solo dati
sintetici e servono a verificare avvio e arresto. Nel controller di produzione
stderr/stdout di QEMU non vengono registrati. La ricerca finale di marcatori
esamina tutto il disco spento, compreso spazio libero e journal esterno; non è
una prova universale di assenza di ogni possibile rappresentazione dei dati.

La build fissa il digest Debian e il lockfile Cargo e registra versioni APT,
hash delle immagini e sorgenti compilati. APT non è ancora fissato a uno
snapshot: non si dichiara una build identica bit per bit o una catena firmata.

## Cosa verificano le prove

Il runner controlla KVM obbligatorio nel binario normale e rifiuto del profilo
TCG; UID/capability/namespace di QEMU; utente limitato e root readonly nel guest;
interblocco di B mentre A è aperta; persistenza separata; scomparsa dei temporanei;
uscita del processo VM e del lavoro guest in background; chiusura forzata e
blocco del generale. Ferma inoltre realmente QEMU con SIGSTOP: la chiusura
ordinata deve fallire mantenendo mapping e stato aperto; la richiesta esplicita
di chiusura forzata passa dal client non privilegiato. Le prove storage e IPC malformato restano nel
[laboratorio LUKS](STORAGE-LAB.md).

Il primo ciclo ha anche rilevato e corretto mountpoint mancanti nella root
immutabile di QEMU e driver input mancanti nell'initramfs minimale ARM: senza
`gpio_keys`/`evdev`, il guest non riceveva la richiesta di spegnimento. Il
controller restituiva un timeout lasciando il volume aperto, come previsto.

Restano: avvio KVM su hardware Linux x86, TUI interattiva e audit esteso di OpenCode nel
guest, console fidata, servizio controller di prodotto, gateway e broker,
trasferimenti, guest effimero, USB fisica, power loss, integrità e revisione
indipendente. La [roadmap](IMPLEMENTATION.md) mantiene separati questi gate.
