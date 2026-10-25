# ISO per collaudo sul PC

Questa variante della live aggiunge pacchetti Debian già esistenti:
NetworkManager/nmtui, firmware Wi-Fi Intel, Realtek, Atheros, Broadcom e
MediaTek, OpenSSH e strumenti per identificare l'hardware. Non include un
desktop. Il supporto a periferiche e filesystem viene dal kernel Debian;
non manteniamo un fork dei driver. Alcuni dispositivi potrebbero comunque
richiedere firmware o un kernel diversi: serve il collaudo sul PC reale.

La [verifica dell'artefatto](hardware-iso-evidence.json) supera 19 controlli
complessivi in due avvii UEFI, da CD-ROM e USB virtuali sul Mac ARM. Include
DHCP, nmtui, autenticazione e revoca SSH, firewall, cattura del traffico e
scomparsa dello stato al nuovo avvio. Le immagini VM di produzione e i binari
del manager/controller sono identici a quelli della prima ISO verificata.

La base è Debian 13 trixie, con repository Debian. La live usa una radice
SquashFS con modifiche in RAM: gli aggiornamenti di sistema devono essere
incorporati in nuove immagini. Un aggiornamento manuale nella live non
sopravvive al riavvio. Un aggiornamento persistente e autenticato della USB
non è ancora implementato.

## Uso

Artefatto: `artifacts/oblivion-hardware-iso/image/oblivion-hardware-test-amd64.iso`.
Il file `SHA256SUMS` nella stessa cartella permette di verificare la copia.
La scrittura su una chiavetta richiede di scegliere esplicitamente il supporto;
gli script di build e test non scrivono su dischi fisici.

1. Avviare il PC x86-64 dalla USB in modalità UEFI. Secure Boot non è supportato
   da questa immagine. Windows non viene avviato.
2. Il manager appare su **Alt+F1**. Con **Alt+F2** si apre il pannello di collaudo.
3. Ethernet usa DHCP. Per il Wi-Fi scegliere **1**, selezionare la rete nel
   normale menu Debian e inserire la password; tornare con Esc.
4. Premere Invio per aggiornare l'indirizzo IP. Scegliere **2** per abilitare
   l'accesso dal Mac, che deve trovarsi sulla stessa rete locale. Il pannello
   mostra l'impronta della chiave SSH del PC: verificarla sul Mac prima del
   primo collegamento. Non serve aprire porte sul router.
5. **3** chiude l'accesso remoto e le sessioni amministrative; **5** spegne.

Il client SSH deve usare la chiave dedicata generata sul Mac in
`~/.ssh/oblivion-hardware-test` (oppure `OBLIVION_TEST_KEY` in fase di build),
con utente `oblivion-test`. Solo la chiave pubblica entra nella ISO.
Quell'utente dispone di `sudo` senza password per il collaudo. L'accesso
richiede la chiave; login root, autenticazione a password e inoltri SSH sono
disabilitati. Il servizio è spento ad ogni avvio e il firewall consente la
porta 22 soltanto da sottoreti IPv4 private direttamente connesse.

La chiave del server viene generata in RAM: cambia ad ogni nuovo avvio.
Una chiave cambiata va verificata di nuovo sul pannello del PC; non disabilitare
globalmente la verifica del server. La modalità diagnostica permette
amministrazione completa dell'host: usare questa immagine per i test, senza
documenti o credenziali personali. I log diagnostici sono limitati a 16 MiB
in RAM; profili Wi-Fi, chiavi del server e modifiche della live scompaiono allo
spegnimento. Il salvataggio del Wi-Fi nel volume generale LUKS resta un passo
successivo della beta.

## Build e verifica

```sh
sh script/oblivion/build-hardware.sh
```

Richiede la prima ISO verificata, descritta in [LIVE-ISO.md](LIVE-ISO.md).
Lo script ne controlla l'hash, riusa le quattro immagini VM senza modificarle
e ricostruisce solo l'host con i pacchetti diagnostici. La prima ISO resta
separata; il target `beta-iso` non include SSH né la chiave del collaudatore.
Gli inventari dei pacchetti e i manifest nell'output registrano la provenienza.

`script/oblivion/run-hardware-boot.py` avvia l'ISO senza modificarla tramite
UEFI/QEMU, con DHCP virtuale e accesso SSH da una porta loopback. QEMU blocca
le connessioni verso l'esterno. Il collaudo esercita manager, nmtui, chiavi,
firewall e disattivazione dell'accesso remoto; un secondo avvio da USB virtuale
verifica la scomparsa dello stato temporaneo. Non è una prova della radio
Wi-Fi, di KVM fisico o di una conversazione con il provider.

L'host diagnostico disabilita IPv6 dal kernel. La cattura comprende anche
l'avvio UEFI: il firmware può emettere pacchetti IPv6 di neighbor discovery
prima che Debian inizi. La configurazione del sistema operativo non governa
quella fase. Il test distingue questi pacchetti iniziali e verifica che il
traffico TCP/UDP successivo sia limitato a DHCP e SSH del collaudo.

Restano i limiti della prima immagine: creazione archivi disabilitata fino al
provisioning USB, nessuna sessione di produzione avviata automaticamente e
nessuna dichiarazione di beta privacy completa.
