# Motore di copia dei documenti

Il pacchetto `packages/oblivion-transfer` implementa il primo componente del
percorso USB → sessione → USB: selezione di file e cartelle, offerta validata,
stream di byte e ricezione in un deposito privato. Il binario `oblivion-copy`
è provato fra due VM con porte virtio-serial dedicate. Il controller e il
manager ora collegano l’importazione alla sessione attiva; export e USB fisica
restano da verificare. Il prototipo non è una live pronta per documenti privati.

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
a 255 byte e profondità 32. Il controller impone per la prima importazione un massimo di 256 MiB e
1.024 voci, senza aumenti richiesti dal manager sul canale. I file aperti durante la
selezione restano aperti: esaurire il limite dei descrittori causa errore,
senza riaprire silenziosamente percorsi diversi.

Percorsi assoluti, `.` e `..`, componenti vuoti, backslash, caratteri di
controllo, controlli bidi, duplicati e conflitti file/directory sono rifiutati.
Le directory genitrici devono comparire prima dei figli. La UI deve
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
il menu di riconciliazione descritto sotto la distingue da una copia pubblicata.
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
il servizio di riconciliazione distingue il deposito privato dalla pubblicazione.

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

## Verifiche

Le [evidenze del manager](manager-import-evidence.json), commit `4f75dfc`,
registrano **nove controlli VM passati in 207,65 secondi** su Mac/HVF, con host
Debian e due guest TCG. Comprendono annullamento prima della copia, ricezione
verificata e modifica come UID della chat, riapertura, morte della sorgente,
destinatario congelato con chiusura forzata, blocco durante la conferma ed
effimero. Sorgente invariata; generale invariato nel percorso effimero;
nessuno dei 14 marcatori cercati sul disco scrivibile. La fixture usa una
shell UID 1000, non la chat compilata né una USB fisica.

Passano 35 test del motore sul Mac e 38 in Debian ARM64/x86-64, 34 test host
sul Mac e 35 in Debian ARM64, Clippy e i 34 task del controllo completo.
Il job x86 dedicato al manager è configurato; il suo esito non fa parte di
queste prove locali. Il laboratorio si riproduce con `HOST=import-host` nel
Dockerfile VM e il normale `run-vm-lab.py`; `--refresh-import-test` sostituisce
solo la fixture nella RAM del laboratorio, verificandone il digest.

## Evoluzione delle prove

Le prime [evidenze di componente](file-copy-evidence.json) registrano **12 test passati**
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

## Importazione nel manager

`Ctrl+^` apre il menu documenti nell’area fidata, mantenendo la console e la
VM della chat. `Ctrl+]` continua a chiudere la sessione. I due tasti incollati
con bracketed paste rimangono dati. Non cambia la TUI upstream della chat.
Il menu seleziona una sorgente e richiede il percorso relativo di un file o
una cartella; non è ancora un browser del filesystem. Mostra il manifest,
quantità e dimensioni prima di **Importa nella chat**. La conferma autorizza
sia la copia verificata sia la disponibilità dei file per l’agente; non invia
automaticamente i documenti al provider.

Il controller elenca solo device USB foglia non montati, escludendo il
filesystem del generale. Il binario di laboratorio ammette inoltre un disco
virtio con seriale fisso; il binario di produzione non lo ammette. Crea una
mappatura device-mapper readonly, assegnata esclusivamente alla VM temporanea
UID/GID 1102. Quella VM non ha NIC né directory condivise; monta il solo
dispositivo autorizzato con `ro,nodev,nosuid,noexec`, aggiungendo `noload` per
ext4. I lettori FAT/exFAT sono configurati; la prova di integrazione usa ext4.
Il controller host non monta né interpreta il filesystem esterno.

`oblivion-transfer-host.service` prepara porte monouso e lega ID casuale,
limiti e scadenza alla sessione già aperta e ai due pidfd QEMU. Le due porte
devono essere pronte prima di iniziare il budget di cinque minuti. Il manager
riceve manifest e ricevuta, mentre i byte passano nel relay a buffer limitato
senza deposito sul filesystem host. Una disconnessione del manager o la morte
di un proprietario interrompono il relay.

Il ricevitore è UID 1001/GID 1000, distinto dalla chat UID 1000. L’inbox
`/vault/.oblivion-inbox` è 0700; dopo hash, fine stream e sincronizzazione,
`publish` rende accessibile `/vault/imports/<id>/files`: file 0660 e directory
0770; il contenitore `<id>` è 0750. La chat può modificarli ma non accedere all’inbox né sostituire gli ID
nel genitore `/vault/imports`, 0750 del ricevitore. Il servizio vede un solo
mount scrivibile `/vault`: due bind separati per inbox/imports impedirebbero
la rename atomica con `EXDEV`. I permessi 0700 delle directory della chat
impediscono al ricevitore di accedervi.

Annullamento, chiusura, blocco e recupero fermano il worker, distruggono la VM
sorgente e rimuovono porta e mapping readonly prima della chiusura LUKS.
La rimozione QMP della porta è asincrona: il controller attende, entro un
limite, che il backend sia effettivamente rimovibile. Una VM destinataria
bloccata resta interdetta fino alla chiusura forzata esplicita; dopo averla
fermata il controller verifica nuovamente la pulizia prima di chiudere LUKS.
Anche l’apertura del socket QMP ha un timeout: la VM congelata può riempire
la coda delle connessioni prima che inizi lo scambio dei messaggi. Il test
Linux riproduce una coda piena con socket reali; il limite usa
[`SO_SNDTIMEO`](https://man7.org/linux/man-pages/man7/socket.7.html).

Una ricevuta persa dopo la pubblicazione non significa che la copia sia
assente: il manager segnala esito incerto e non riprova automaticamente.
Un errore durante la pubblicazione può lasciare un ID privato riservato;
non viene sovrascritto. La riconciliazione ora segue il percorso descritto sotto.
Restano export con flush/smontaggio, copia A→B e OnionShare. Rimozione USB,
ENOSPC del destinatario, guasti I/O, FAT/exFAT e hardware fisico restano gate
di verifica. Nessuna delle prove trasforma l’originale esterno in un file cifrato.

### Riconciliazione nel manager

Nel menu Documenti, **U — Copie della sessione** apre un elenco paginato.
Il controller crea una porta monouso verso il ricevitore della sola VM attiva;
non avvia una VM sorgente, non monta il vault sull’host e non collega una USB.
Il relay valida richieste e risposte, osserva il pidfd della VM e la connessione
del manager e conserva la stessa scadenza dell’operazione. Importazione e
manutenzione prendono un lock sul descrittore dell’inbox: una copia ancora
in corso non può essere rimossa da una seconda operazione.

Dopo la verifica dei byte, il ricevitore scrive una ricevuta 0600 nella propria
inbox: ID, digest del manifest, numero di voci e byte originali. Non contiene
nomi né contenuti dei documenti. La scrittura usa file temporaneo, fsync e
rename dentro lo stesso volume cifrato. Dopo la pubblicazione e la relativa
sincronizzazione registra un marcatore privato; la ricevuta inviata al manager
viene dopo questi passi. In modalità effimera anche questi metadati sono in RAM.

| Stato | Significato e azione |
| --- | --- |
| Pubblicata | Pubblicazione registrata; la chat può aver modificato i documenti. La ricevuta descrive l’importazione originale, non certifica i byte attuali. |
| Disponibile, conferma interrotta | Contenitore già esposto senza marcatore finale, oppure importazione precedente al registro. Nessun completamento dedotto e nessuna rimozione. |
| Privata, non consegnata | Tentativo nell’inbox o contenitore ancora 0700, mai esposto. **D**, seguito da una seconda conferma **D**, lo elimina; ogni altra copia resta intatta. |
| Da verificare | Marcatore di pubblicazione senza contenitore accessibile coerente. Si conserva per diagnosi. |

L’elenco non percorre né ricalcola hash del contenuto modificabile dalla chat.
La cancellazione ricontrolla lo stato sotto lock e rifiuta copie pubblicate,
link e metadati incoerenti. Non esistono ripresa, nuovo invio o sovrascrittura
automatici. Se una cancellazione perde la risposta, rileggere l’elenco: il
silenzio non è una conferma. Le pagine contengono al massimo 64 record; ogni
radice è limitata a 4.096 ID durante l’ispezione. Oltre tale limite o con un
registro danneggiato l’operazione fallisce, senza pulizia automatica.
Le prove di processo interrotto non sostituiscono quelle di power loss, USB
rimossa o guasti I/O del supporto, che restano aperte.

Il core accetta un `Read`/`Write` limitato dal chiamante e non apre socket o
device di trasporto. L'endpoint dispone ora del canale con deadline; le porte
provate in laboratorio restano separate da console, token API e controllo
amministrativo. La prima CI (`7ba0f97`) supera i 23 test su entrambe le
architetture ma fallisce entrambe le prove VM: il mittente scade mentre il
destinatario sta ancora facendo il boot in emulazione. Il laboratorio ora
aspetta che entrambe le porte siano pronte e solo allora avvia gli endpoint.
Il budget di copia rimane 30 secondi, separato dall'attesa di avvio. La
correzione supera nuovamente i sei controlli su Mac, con HVF (6,43 secondi)
e TCG (59,55 secondi complessivi). La successiva [CI `2e5c2f4`](https://github.com/0xthemiddleman/oblivion/actions/runs/40137315057)
termina con tutti i 31 job passati, inclusi entrambi i laboratori di copia.
È precedente all’integrazione del manager descritta sopra.

Riferimenti tecnici: [I/O e sincronizzazione Rust](https://doc.rust-lang.org/std/fs/struct.File.html),
[SHA-256 incrementale RustCrypto](https://docs.rs/sha2/0.10.9/sha2/),
[gestione delle porte seriali QEMU](https://github.com/qemu/qemu/blob/master/hw/char/virtio-console.c).
