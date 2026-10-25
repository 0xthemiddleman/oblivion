# Primo collaudo su PC fisico

Il 25 ottobre 2026 la ISO diagnostica è stata avviata da una USB su un PC
Intel x86-64 con 32 GB di RAM. Il collegamento SSH è stato autorizzato dal
pannello locale e l'impronta del server confrontata con la foto dell'utente.
Indirizzo LAN, chiavi del collaudo e inventario dettagliato dei dischi restano
negli artefatti locali, non nel report pubblico.
Il [report verificabile](physical-hardware-evidence.json) registra sei controlli
superati in 23,52 secondi, oltre all'inventario iniziale e alla verifica finale.

## Verificato

- Avvio UEFI fino al manager UID 1000, senza servizi falliti.
- Ethernet Realtek RTL8125 con driver Debian `r8169` e DHCP funzionante.
  La scheda Wi-Fi Intel è riconosciuta da `iwlwifi`; il solo riconoscimento
  non dimostra associazione alla rete, trasferimento dati o riconnessione.
- API KVM utilizzabile e avvio effettivo del kernel/root filesystem Debian
  inclusi nell'ISO, con QEMU UID 1100 e accelerazione KVM confermata da QMP.
  Il guest usa un `init=/bin/sh` diagnostico, senza NIC o cartelle condivise,
  e termina con uno spegnimento pulito.
- Controller di produzione: generale LUKS2 e due sessioni LUKS2 in file RAM,
  Argon2id, password errate rifiutate, apertura contemporanea impedita,
  file presente in A e assente in B, recupero del file riaprendo A.
- Dopo il blocco entrambi i mapping sono chiusi; il marcatore sintetico non
  compare nel contenitore cifrato esterno. I file di prova vengono eliminati
  dalla RAM e il manager originale viene ripristinato.
- I contatori kernel di scritture e discard dei dischi fisici, inclusa la USB,
  non aumentano durante il test. È una misura delle operazioni osservate in
  quella finestra, non un confronto byte per byte dei dischi né un audit di
  tutte le possibili forme di persistenza.

Lo script è `script/oblivion/hardware-probe.py`. Richiede la live, root, swap
disabilitato, `/run` in tmpfs e assenza di un archivio utente inizializzato.
Non accetta dispositivi fisici come destinazione. I mount delle sessioni
sintetiche sono eseguiti dal solo harness per verificarne i file; non cambiano
l'architettura di produzione, in cui l'host non monta il filesystem delle chat.
Durante la prova il controller normale viene fermato e poi ripristinato.

## Riscontro: log di crash nella memoria del firmware

Il PC ha mostrato `efi_pstore` registrato come backend persistente, con
`pstore_disable=N`. Il kernel poteva quindi conservare log di crash in
variabili UEFI, fuori dai volumi LUKS. Non è stata osservata o provocata una
fuga di chat; è una funzione di persistenza incompatibile con l'obiettivo
del progetto, emersa su hardware reale.

Nella live in esecuzione è stata impostata la disabilitazione del backend EFI
e mascherato `systemd-pstore.service` in RAM. La verifica successiva mostra
`pstore_disable=Y` e nessun backend registrato. Non sono stati letti i contenuti
di eventuali crash record preesistenti, né ne è stata richiesta la cancellazione.
Il setter del [kernel Linux](https://github.com/torvalds/linux/blob/v6.12/drivers/firmware/efi/efi-pstore.c)
disabilita il backend anche a runtime.

Le ricette di avvio ora aggiungono `pstore.backend=none`,
`efi_pstore.pstore_disable=1` e `systemd.mask=systemd-pstore.service`.
La variante diagnostica aggiornata è costruita separatamente in
`artifacts/oblivion-hardware-iso/image-pstore/oblivion-hardware-test-amd64.iso`.
Il test dell'ISO controlla questi parametri e il servizio mascherato.
La [nuova immagine verificata](hardware-pstore-evidence.json) supera entrambi
gli avvii UEFI sul Mac, da CD-ROM e USB virtuali, con tutti i controlli di rete
e SSH. Il suo avvio a freddo sul PC fisico resta da provare.
La vecchia USB non viene modificata via SSH: la correzione a runtime si perde
al riavvio; un nuovo avvio fisico deve usare l'immagine aggiornata.

## Limiti

Questa prova verifica KVM e storage sul PC reale separatamente. Non verifica
ancora l'intero avvio di una chat attraverso il controller, le quattro VM
insieme, il provider reale, i trasferimenti USB, la persistenza del profilo
sulla chiavetta o Secure Boot. I dati e le password del test sono sintetici;
il contenitore generale è in RAM, non persistente sulla USB.

Il primo tentativo del harness aveva anticipato un job di recupero accodato
da systemd: il controller ha rifiutato l'apertura con `recovery_required`.
Il test attende ora anche i job pendenti e il ripristino effettivo del manager;
non aggira il blocco di sicurezza.
