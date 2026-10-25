# Prima ISO di sviluppo

Obiettivo: avvio x86-64/UEFI fino al manager fidato, usando i servizi e i binari
di produzione. È il primo artefatto avviabile verso la beta, non la beta
funzionale né una release per dati privati.

La [prima immagine verificata](live-iso-evidence.json) pesa circa 890 MiB.
Avvia il manager in 22,27 s da CD-ROM virtuale e 22,19 s da USB virtuale,
con firmware UEFI e host x86 emulato sul Mac ARM. Entrambe le prove verificano
tastiera, ritorno al manager, spegnimento ACPI e disco estraneo invariato.
Questi tempi e risultati riguardano il laboratorio, non un PC fisico.

## Contenuto e confini

- Debian trixie minimale, senza desktop, SSH, login root o secondo login Linux.
- Controller privilegiato separato dal manager UID 1000, sulla console tty1.
  Il suo namespace privato espone soltanto questo terminale oltre ai dispositivi
  virtuali standard; non viene disabilitato `PrivateDevices`.
- OpenCode compilato con il profilo Oblivion; componenti della chat invariati.
- Immagini della sessione, gateway Tor/SOCKS e import/export già incluse.
  Nessun certificato, provider o binario con emulazione del laboratorio.
- Root SquashFS con overlay volatile, `/tmp` e `/var` in RAM; swap,
  ibernazione, core dump e journal persistente disabilitati.
- Nessuna rete configurata o VM avviata automaticamente in questa consegna.

La preparazione dell'area dati USB è il passo successivo. Il manager reale
interroga il controller ma mostra **supporto dati non predisposto** e non
accetta la creazione di archivi. `/persist` è un mount tmpfs in sola lettura:
il controller non può creare per errore un archivio persistente nell'overlay
RAM. Non ci sono password o volumi preconfezionati.

Il tasto Q blocca ed esce dal manager; systemd lo riavvia. Per spegnere si usa
il pulsante di alimentazione/ACPI. Questa immagine non monta né formatta
automaticamente supporti dati.

## Build dal Mac o da Linux

Servono Docker Buildx con esecuzione Linux amd64, Python 3 e Bun **1.4.4**.
Il filesystem di lavoro deve avere spazio per le immagini Docker e l'ISO.
Non è necessario un PC x86 per costruire l'immagine.

Da root del repository, predisporre le dipendenze native della compilazione
incrociata dal lockfile, poi eseguire:

```sh
bun install --frozen-lockfile --os='*' --cpu='*' --libc='*'
sh script/oblivion/build-live.sh
```

`OBLIVION_BUN` permette di selezionare il percorso assoluto del Bun fissato.
Lo script aggiorna soltanto gli artefatti in `artifacts/oblivion-beta-iso`.
Il target Docker `beta-iso` è in `script/oblivion/Dockerfile.vm-lab` per riusare
le basi e le build esistenti, ma seleziona esplicitamente gli stadi di
produzione e non dipende dalle scelte `HOST`/`GUEST` del laboratorio.

Output: `artifacts/oblivion-beta-iso/image/oblivion-development-amd64.iso`,
`SHA256SUMS`, inventari Debian delle cinque immagini, hash dei binari e
manifest dei sorgenti. Le versioni effettive dei pacchetti sono registrate;
non è ancora una build riproducibile byte per byte o una release firmata.

La composizione usa [Debian live-boot](https://manpages.debian.org/trixie/live-boot-doc/live-boot.7.en.html)
per avviare il filesystem compresso e [GRUB mkrescue](https://manpages.debian.org/trixie/grub-common/grub-mkrescue.1.en.html)
per l'immagine avviabile. La persistenza generica di Debian Live è disabilitata:
la futura persistenza USB resta amministrata dal controller Oblivion.

## Prova dell'artefatto

Il collaudo usa **l'ISO senza modifiche**, avviata da firmware UEFI. Non passa
direttamente kernel/initrd a QEMU, non usa `--preview` e non inserisce hook di
test nel guest. Legge la schermata VGA via QMP/OCR, invia input alla tastiera
e richiede lo spegnimento ACPI. Un disco sintetico estraneo deve restare
byte-identico. Nessun disco fisico o filesystem dell'host viene condiviso.

Sul Mac con QEMU e Tesseract installati:

```sh
python3 script/oblivion/run-live-boot.py \
  --iso artifacts/oblivion-beta-iso/image/oblivion-development-amd64.iso \
  --firmware /opt/homebrew/share/qemu/edk2-x86_64-code.fd \
  --variables /opt/homebrew/share/qemu/edk2-i386-vars.fd \
  --output artifacts/oblivion-beta-iso/boot-01
```

L'output deve essere una directory nuova. Contiene schermate, OCR, comando,
log del solo laboratorio e risultato con hash dell'ISO. TCG è usato per
avviare l'host x86 sul Mac ARM, non come fallback delle VM di produzione.
Queste continuano a richiedere KVM; tale requisito non impedisce il solo
avvio del manager.

Con `--media usb` la stessa ISO viene collegata come dispositivo USB mass-storage
in sola lettura tramite un controller xHCI virtuale, anziché come CD-ROM. Il
collaudo non formatta né collega una chiavetta fisica.

## Limiti da chiudere prima della beta funzionale

Preparazione esplicita della USB e persistenza LUKS, configurazione fisica
della rete, conversazione con Abliteration reale e test KVM su PC x86.
La compatibilità Secure Boot non è implementata: non cambiamo impostazioni
del firmware automaticamente e non dichiariamo una catena di avvio verificata.
Il checksum del download non sostituisce autenticazione o protezione del boot.
OnionShare, copie fra sessioni e nuove funzioni restano fuori da questa tappa.
