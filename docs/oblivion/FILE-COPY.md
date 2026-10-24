# Motore di copia dei documenti

Il pacchetto `packages/oblivion-transfer` implementa il primo componente del
percorso USB → sessione → USB: selezione di file e cartelle, offerta validata,
stream di byte e ricezione in un deposito privato. Non è ancora collegato al
manager, a una USB o a porte delle VM. Non abilita importazioni nella chat.

Il confine resta quello di [NETWORK-AND-FILES.md](NETWORK-AND-FILES.md): la VM
di importazione vede il solo dispositivo autorizzato; quella della chat
riceve copie, senza mount condivisi. Il controller non esegue questo lettore
sul filesystem esterno né monta il vault della chat.

## Selezione e offerta

`Selection::prepare` apre una directory sorgente autorizzata dal chiamante e
i soli percorsi selezionati, includendo ricorsivamente le cartelle scelte.
Ogni componente viene risolto con `openat` e `O_NOFOLLOW`, da descrittori di
directory mantenuti aperti. File regolari e directory sono gli unici tipi
accettati; symlink, hard link, FIFO, socket e dispositivi sono rifiutati.
L'ambiente sorgente deve comunque usare mount `nodev,nosuid,noexec` e, per
l'importazione, readonly: la libreria non sostituisce il confinamento del VM.

Il manifest contiene tipo, percorso relativo, dimensione e SHA-256 dei file.
Non trasporta proprietari, permessi eseguibili, timestamp o attributi estesi.
Gli archivi compressi sono file ordinari; non vengono estratti. La libreria
non carica configurazioni, plugin o interpreti presenti nella selezione.

Il chiamante fornisce i limiti di byte e voci. Il protocollo ammette al massimo
4.096 voci e 1 MiB di header, percorsi UTF-8 fino a 4.096 byte, componenti fino
a 255 byte e profondità 32. Il controller dovrà scegliere quote operative
compatibili con RAM e spazio del destinatario. I file aperti durante la
selezione restano aperti: esaurire il limite dei descrittori causa errore,
senza riaprire silenziosamente percorsi diversi.

Percorsi assoluti, `.` e `..`, componenti vuoti, backslash, caratteri di
controllo, controlli bidi, duplicati e conflitti file/directory sono rifiutati.
Le directory genitrici devono comparire prima dei figli. La futura UI deve
comunque trattare tutti i nomi come dati non fidati e applicare il renderer
sicuro del manager.

L'offerta viene letta e validata **prima** di creare la destinazione. Il suo
digest identifica i metadati e gli hash offerti; l'autorizzazione dell'utente
e la capacità monouso del controller devono vincolare precisamente questa
offerta. Un hash non certifica che un documento sia innocuo.

## Stream e ricezione

Il formato v1 usa magic `OBLVCP01`, lunghezza big-endian a 32 bit e manifest
JSON. Seguono, nell'ordine del manifest, esattamente i byte dichiarati di ogni
file e il marcatore finale `OBLVEND1`. Il contenuto viene copiato e verificato
con un buffer da 64 KiB, senza caricare l'intero file in RAM.

Il mittente ricontrolla dimensione e hash dai descrittori già selezionati.
Sostituire il percorso dopo la selezione non sostituisce il file aperto;
modificare il suo contenuto provoca errore senza inviare il marcatore finale.
Non è uno snapshot atomico di un filesystem che cambia: il destinatario
accetta soltanto i byte corrispondenti all'offerta. Per l'export, la copia
completata nel servizio di trasferimento sarà quella da confermare prima
dell'invio, come stabilito nel piano.

`receive` richiede una directory privata appartenente all'identità del
ricevitore e un identificatore locale di 32 cifre esadecimali. Crea una
transazione nuova, rifiutando identificatori già presenti. Il peer non può
scegliere la directory radice. Percorsi figli, creazione e pubblicazione
restano ancorati ai descrittori di directory; nessun percorso assoluto viene
ricavato dai nomi ricevuti.

I file parziali sono in `<id>/pending`, con directory 0700 e file 0600. Solo
dopo hash corretti, marcatore finale e sincronizzazione viene pubblicato
`<id>/files`. La ricevuta riporta identificatore, hash del manifest, numero
di voci e byte. Il chiamante deve verificarla prima di dichiarare completata
la copia. Nessun originale viene rimosso e nessun file preesistente viene
sovrascritto. La pubblicazione rimane nel deposito privato del servizio:
renderla disponibile alla chat richiede un'operazione esplicita successiva.

Errori di lettura, corruzione e interruzione provocano la rimozione della
transazione parziale. Un errore della pulizia viene restituito, non nascosto.
Arresto forzato del processo o power loss possono lasciare una transazione:
il recupero del servizio dovrà gestirla senza presentarla come copia riuscita.
La destinazione deve essere nel volume della sessione o in RAM, secondo la
modalità; la libreria non sceglie né verifica il tipo di storage.

## Verifiche e integrazione ancora necessaria

Le [evidenze di componente](file-copy-evidence.json) registrano **12 test passati**
sul Mac, in Debian ARM64 e in Debian x86-64 sotto Docker sul Mac. Passano
Clippy sul Mac e il controllo completo del repository (34 task). I nuovi job
CI sono predisposti; queste evidenze non ne dichiarano ancora l'esito.

I test esercitano la libreria reale con file, directory e socket Unix, senza
mock del filesystem. Coprono copia selettiva, Unicode, file vuoti, stream
oltre il buffer, ricevuta, permessi, sorgenti modificate, alias, tipi speciali,
header malformati, quote, overflow, interruzione e rifiuto di sovrascritture.
Il Dockerfile dedicato e i job CI ARM64/x86 eseguono gli stessi test in Debian.

Queste prove di componente **non** dimostrano USB → VM → USB o assenza di
tracce sull'intera live. Per completare il percorso servono:

1. Identità e unità isolate del ricevitore, con deposito LUKS/tmpfs e recupero.
2. Ambiente di importazione senza rete e device autorizzato readonly; export
   con autorizzazione di scrittura distinta e smontaggio verificato.
3. Canale locale dedicato, capacità monouso vincolata a VM, manifest, quota e
   scadenza; timeout assoluti e cancellazione anche durante letture bloccate.
4. Selezione e conferma nel manager fidato, consegna alla chat e revoca su
   blocco/cambio sessione; una sola sessione aperta resta obbligatoria.
5. Prove VM con ENOSPC, USB rimossa, crash, nomi ostili, flush fallito e scansione
   dei residui, poi prova sul supporto fisico. A→B e OnionShare seguono.

Il core accetta un `Read`/`Write` limitato dal chiamante e non apre socket o
device di trasporto. Una lettura bloccante senza deadline non è adatta al
servizio. La prima integrazione proverà una porta virtio-serial dedicata,
separata da console, token API e controllo amministrativo.

Riferimenti tecnici: [I/O e sincronizzazione Rust](https://doc.rust-lang.org/std/fs/struct.File.html),
[SHA-256 incrementale RustCrypto](https://docs.rs/sha2/0.10.9/sha2/),
[gestione delle porte seriali QEMU](https://github.com/qemu/qemu/blob/master/hw/char/virtio-console.c).
