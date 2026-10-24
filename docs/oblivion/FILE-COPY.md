# Motore di copia dei documenti

Il pacchetto `packages/oblivion-transfer` implementa il primo componente del
percorso USB → sessione → USB: selezione di file e cartelle, offerta validata,
stream di byte e ricezione in un deposito privato. Il binario `oblivion-copy`
è ora provato fra due VM con porte virtio-serial dedicate. Non è ancora
collegato al manager o a una USB fisica e non abilita importazioni nella chat.

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

## Endpoint e scadenza del canale

`oblivion-copy send|receive <radice-locale> <budget-ms>` usa esclusivamente il
descrittore 3 ereditato dal launcher fidato. Non crea listener e non riceve
percorsi di socket dal peer. Nomi selezionati, offerta e ricevuta passano sul
canale privato, senza argv, environment o output di console. Il processo
disabilita i core dump; su Linux applica anche `PR_SET_DUMPABLE=0` e
`PR_SET_NO_NEW_PRIVS=1`. Un errore produce soltanto `transfer_failed` su stderr;
le future unità di prodotto devono disabilitarne la persistenza.

Il mittente riceve identificatore, quote e selezione, prepara l'offerta e
aspetta una conferma con lo stesso identificatore e digest del manifest.
Solo allora invia i byte. Il ricevitore richiede un'offerta corrispondente
alla conferma fornita dal controller prima di creare la transazione. Il
mittente verifica identificatore, hash, byte e numero di voci della ricevuta
prima di rispondere `copied`. Ogni processo gestisce una sola operazione.

L'endpoint **non è l'autorità che concede il permesso**: il launcher deve
vincolare il descrittore e l'offerta alla VM corrente e alla decisione presa
nel manager. Il controllo di questi campi non autentica da solo un peer.
Emissione e revoca della capacità amministrativa restano da integrare.

Il canale abilita I/O non bloccante e usa una sola scadenza monotona per
letture e scritture, anche frammentate. Il progresso non rinnova il budget;
un peer che non legge non può bloccare indefinitamente una scrittura sul
canale. Le prove distinguono timeout, EOF e cancellazione. Questo non impone
una scadenza alle chiamate bloccanti del filesystem: la supervisione deve
poter terminare l'ambiente anche quando un disco o `fsync` non risponde.

Se la copia viene pubblicata ma la ricevuta si perde, il mittente segnala
errore e la copia completa può restare nel deposito privato del destinatario.
Non si ripete automaticamente la copia e non si presenta l'esito come certo;
il recupero del servizio deve riconciliare questi casi prima della consegna.

## Relay host e revoca

`oblivion-copy-relay <budget-ms>` riceve dal controller tre descrittori:
sorgente FD3, destinazione FD4 e controllo fidato FD5. Ogni processo gestisce
una sola copia, senza listener, riapertura di socket o tentativi automatici.
Il controller deve scegliere le porte QEMU della coppia autorizzata e mantenere
privato il controllo. Il relay non decide quali sessioni o dispositivi ammettere.

Su Linux, `SO_PEERPIDFD` fornisce il riferimento ai processi che hanno creato
le due connessioni Unix. Questi handle restano legati a quelle istanze, senza
cercare un PID numerico o riaprire il percorso del socket. Servono Linux 6.5+
e l'ABI x86-64/ARM64 supportata; l'assenza del meccanismo provoca errore. Il
binario rifiuta l'esecuzione su Mac; i test della libreria sul Mac usano canali
espliciti per simulare la durata dei proprietari e non attestano i pidfd Linux.

Il controllo invia selezione e quote, riceve il manifest validato e deve
approvare identificatore e digest esatti. Il relay inoltra l'autorizzazione
solo dopo questa conferma. Ricontrolla i file durante l'inoltro e trasmette il
marcatore finale solo con tutti gli hash corretti. Verifica ricevuta e risposta
del mittente; il controllo riceve metadati, non il contenuto dei documenti.
Il buffer del contenuto rimane limitato a 64 KiB e non viene scritto su disco host.

I tre canali condividono una scadenza assoluta. Morte di una VM, chiusura del
controllo o dati di annullamento interrompono l'attesa e chiudono entrambe le
porte, anche se l'operazione era bloccata sull'altra VM. La revoca ha precedenza
sui dati pronti nello stesso `poll`. Il controller deve attendere la fine del
worker prima di smontare o cambiare sessione: questo collegamento al ciclo
`close`/`lock` non è ancora implementato. Non si annulla retroattivamente una
copia già pubblicata nel deposito privato; valgono i limiti della ricevuta persa.

Le [evidenze del relay](file-copy-relay-evidence.json) registrano **31 test sul
Mac e 34 in Debian ARM64/x86-64**, compresi tre test Linux con il binario reale
e processi reali. Il laboratorio Linux ARM64/TCG sul Mac supera **nove controlli
in 164,73 secondi**: copia con relay compilato, controllo separato dai byte,
annullamento prima dell'approvazione e `SIGKILL` della VM destinataria mentre
il mittente è ancora attivo. Il marcatore sintetico è assente dai quattro
dischi destinatari dopo l'uscita delle VM. Nel caso `SIGKILL` il guest non
smonta il volume; gli altri ricevitori verificano la normale chiusura LUKS.

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

Le [prove degli endpoint](file-copy-endpoints-evidence.json) portano la suite
a **23 test passati** sul Mac e in Debian ARM64/x86-64: cinque sul canale,
sei con i binari reali e dodici del core. La prova successiva avvia due VM
Debian ARM64/HVF senza rete, con sistema readonly, sorgente virtuale readonly
e destinazione LUKS2. Passano sei controlli in 6,45 secondi: selezione, ricevuta,
nessun mount condiviso, sorgente byte-identica, chiusura del volume e
cancellazione di una copia interrotta su una porta virtio-serial reale.
Il marcatore del documento non compare nei due dischi destinatari chiusi.

La sorgente del laboratorio è intenzionalmente un disco virtuale in chiaro;
la copia originale vi rimane. Non è una scansione di tutti i file della live.
Il KDF economico della fixture LUKS serve solo al test e non modifica la
configurazione crittografica del controller di produzione. Su Mac QEMU non
supporta il flag Linux `dump-guest-core=off`: il runner imposta `RLIMIT_CORE=0`,
senza presentare HVF come prova dell'isolamento Linux/KVM previsto dal prodotto.

Queste prove **non** completano USB → sessione di chat → USB. Servono ancora:

1. Identità e unità isolate del ricevitore, con deposito LUKS/tmpfs e recupero.
2. Ambiente di importazione senza rete e device autorizzato readonly; export
   con autorizzazione di scrittura distinta e smontaggio verificato.
3. Collegamento del relay al controller, con emissione della capacità monouso
   per la coppia autorizzata e aggancio della revoca a chiusura/blocco;
   gestione dei guasti del filesystem e recupero delle copie orfane.
4. Selezione e conferma nel manager fidato, consegna alla chat e revoca su
   blocco/cambio sessione; una sola sessione aperta resta obbligatoria.
5. Prove VM con ENOSPC, USB rimossa, crash, nomi ostili, flush fallito e scansione
   dei residui, poi prova sul supporto fisico. A→B e OnionShare seguono.

Il core accetta un `Read`/`Write` limitato dal chiamante e non apre socket o
device di trasporto. L'endpoint dispone ora del canale con deadline; le porte
provate in laboratorio restano separate da console, token API e controllo
amministrativo. La prima CI (`7ba0f97`) supera i 23 test su entrambe le
architetture ma fallisce entrambe le prove VM: il mittente scade mentre il
destinatario sta ancora facendo il boot in emulazione. Il laboratorio ora
aspetta che entrambe le porte siano pronte e solo allora avvia gli endpoint.
Il budget di copia rimane 30 secondi, separato dall'attesa di avvio. La
correzione supera nuovamente i sei controlli su Mac, con HVF (6,43 secondi)
e TCG (59,55 secondi complessivi). La nuova esecuzione CI resta da verificare.

Riferimenti tecnici: [I/O e sincronizzazione Rust](https://doc.rust-lang.org/std/fs/struct.File.html),
[SHA-256 incrementale RustCrypto](https://docs.rs/sha2/0.10.9/sha2/),
[gestione delle porte seriali QEMU](https://github.com/qemu/qemu/blob/master/hw/char/virtio-console.c).
