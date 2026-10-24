# Creazione interrotta e spazio esaurito

Le [evidenze del laboratorio](creation-evidence.json) registrano 14 controlli
superati in 188,15 secondi con i binari attuali, includendo il ciclo di crash e recupero
dei servizi. Hash di sorgenti, binari, fixture, initramfs e disco sono conservati
nel report. Le due interruzioni finali avvengono dopo una scrittura riuscita sul
dispositivo cifrato, durante la creazione del generale e della sessione.

Il controller distingue un archivio pubblicato da un tentativo di creazione.
Il generale nasce come `<general_image>.pending`; una sessione nasce come
`sessions/<id>.pending`, dentro il generale cifrato. Il nome definitivo viene
pubblicato dopo LUKS2, formattazione ext4, chiusura del mapping e sincronizzazione
del file. `renameat2(RENAME_NOREPLACE)` impedisce di sostituire una destinazione
esistente; prima di rispondere con successo viene sincronizzata anche la
directory. La semantica è quella di [rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html).

Un'interruzione può lasciare un `.pending`, anche se la formattazione era quasi
finita. Non viene ripreso o riformattato automaticamente. Il controller apre le
sessioni soltanto dal nome definitivo `.luks`, quindi questi tentativi non sono
mai stati consegnati a una chat e non contengono documenti scritti dall'agente.
Un generale incompleto non è ancora stato aperto per salvare il profilo.

Il recupero dei processi e dei mapping rimane separato dal recupero dello spazio:
prima si verifica che non ci siano worker o volumi ancora aperti, poi si può
eliminare un tentativo. La morte del controller non rilascia il lock mentre il
figlio di formattazione è ancora vivo. Il [recupero dei servizi](HOST-SERVICES.md)
mantiene inoltre il suo blocco finché la chiusura non è verificata.

## Interfaccia fidata

| Stato o comando           | Comportamento                                                                                                             |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Nessun generale esistente | Invio propone la prima creazione, con password nascosta di almeno 16 byte e conferma. Non formatta dispositivi fisici.    |
| Generale incompleto       | La schermata segnala la preparazione interrotta. D apre una conferma; una seconda D elimina solo il tentativo incompleto. |
| Lista sessioni            | Mostra spazio disponibile nel generale e dimensione prevista della prossima sessione.                                     |
| I                         | Passa alla lista separata delle creazioni interrotte e torna alle sessioni.                                               |
| D nella lista incompleta  | Chiede conferma per il solo identificatore selezionato; annullare conserva il file.                                       |
| Spazio esaurito           | Errore esplicito, nessuna apertura della VM; l'eventuale residuo compare fra le creazioni interrotte.                     |

Le operazioni IPC `discard-general-pending` e `discard-session-pending` ricevono
soltanto un'azione e, per le sessioni, un identificatore esadecimale. Non accettano
percorsi. Il controller rifiuta symlink, hard link, nomi manipolati, un mapping di
sessione ancora aperto e lo stato di recupero non risolto. Il generale esistente
non è eliminabile tramite l'operazione per la sua preparazione. Un ID pubblicato
non seleziona il relativo `.luks` attraverso l'operazione per gli incompleti.

Il profilo in sola lettura non espone questa lista e rifiuta le cancellazioni.
Questa è una funzione di pulizia dei tentativi mai pubblicati, non una funzione
di eliminazione delle chat o di riparazione degli archivi corrotti.

## Spazio e persistenza

Ogni immagine riserva la propria dimensione prima della formattazione. Il
controller distingue `ENOSPC`/`EDQUOT` dagli altri errori di preallocazione,
leggendo il codice restituito direttamente da
[posix_fallocate(3)](https://www.man7.org/linux/man-pages/man3/posix_fallocate.3.html).
Un errore può lasciare un file parzialmente allocato: viene conservato e mostrato.
Lo spazio indicato nella UI è una misura del filesystem, non una prenotazione;
la preallocazione rimane il controllo effettivo della nuova creazione.

L'archivio già pubblicato non viene sovrascritto o troncato per fare spazio. La
cancellazione del tentativo libera le sue allocazioni dopo la conferma; non è
una promessa di cancellazione fisica sicura delle celle flash. Le passphrase non
vengono salvate in questi file, nell'ambiente o nei log.

## Prove riproducibili sul Mac

Il laboratorio usa i binari Linux compilati sul Mac ARM, cryptsetup/ext4 reali,
systemd e la TUI su una PTY. Si esegue con il bundle e il runner descritti in
[HOST-SERVICES.md](HOST-SERVICES.md). `vm-services-test.py` include le prove di
creazione e ripete quelle dei servizi.

Per interrompere anche un `mkfs` molto rapido, la fixture usa eventi Linux
di exec e syscall attraverso [ptrace(2)](https://www.man7.org/linux/man-pages/man2/ptrace.2.html).
Attende il ritorno riuscito della prima `write`/`pwrite64` sul dispositivo mappato,
ferma il vero processo di formattazione, uccide il controller,
verifica che il figlio mantenga il lock, poi interrompe il figlio e invoca il
recupero. Non sostituisce cryptsetup o mkfs con simulazioni e non modifica i
timeout del prodotto. Tutte le credenziali e i dati sono sintetici.
Il tracer usa esplicitamente l'ABI ARM64 del laboratorio Mac; le due interruzioni
sono registrate come `OBLIVION_FAULT_AFTER_BLOCK_WRITE` nell'evidenza seriale.

La matrice comprende preparazione generale interrotta, creazione di sessione
interrotta, annullamento e conferma dalla TUI, errore di spazio sul filesystem
esterno e nel generale, rifiuto di alias e percorsi ostili, conservazione degli
archivi pubblicati, riapertura e profilo in sola lettura identico byte per byte.
Il runner verifica anche i marcatori nel disco virtuale spento.

## Limiti ancora aperti

Queste prove non equivalgono a togliere alimentazione o scollegare una USB fisica.
Restano errori I/O/fsync, crash in ogni fase della pubblicazione, filesystem
danneggiati, perdita dell'header, disco pieno dentro il guest, OOM, backup e cambio
password. La persistenza dopo power loss dipende anche dal dispositivo che
rispetta i flush; non viene dedotta dal solo successo di una syscall.

Un vecchio prototipo che abbia lasciato un generale incompleto con il nome
definitivo non viene cancellato o riconvertito automaticamente. Non rinominare
un archivio contenente dati in `.pending` per usare la pulizia: richiede invece
una procedura distinta di diagnosi e recupero.
