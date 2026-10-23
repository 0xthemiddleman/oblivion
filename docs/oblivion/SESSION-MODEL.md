# Sessioni OpenCode e ambienti Oblivion

23 ottobre 2026. Analisi sul commit upstream `68bc26b7da1990ce10ffecfbc1e09eb13aa5d824`. **La proposta di integrazione descritta qui è da discutere con l'utente; non è implementata.** Rimangono confermati un volume e una password indipendenti per sessione Oblivion, una sola aperta e nessun filesystem condiviso.

## Il comportamento attuale di OpenCode

Un client TUI può creare, elencare e selezionare più conversazioni servite dalla stessa istanza OpenCode. Non si tratta di altrettante VM o identità di storage:

- [ClientProvider](../../packages/tui/src/context/client.tsx) mantiene un client API e una connessione eventi al server.
- Il comando [session.new](../../packages/tui/src/app.tsx) prepara una nuova conversazione; il [prompt](../../packages/tui/src/component/prompt/index.tsx):1237 crea la sessione applicativa.
- [DialogSessionList](../../packages/tui/src/component/dialog-session-list.tsx):63 chiama `session.list` e alla riga 249 cambia la route visualizzata.
- Il [server](../../packages/server/src/handlers/session.ts):59,93 espone list/create; il [SessionStore](../../packages/core/src/session/store.ts) usa il servizio Database, e la [tabella session_v2](../../packages/core/src/session/sql.ts) contiene ID, parent e fork di molte conversazioni.
- Le sessioni possono avere lavoro in background, figli e fork. Cambiare scheda non equivale a fermare i processi, smontare un filesystem o chiudere una chiave LUKS.

Questo separa la cronologia applicativa, ma non crea il confine di riservatezza per ogni chat richiesto da Oblivion. Cifrare il DB comune protegge tutte quelle chat insieme; quando lo si apre, non mantiene le altre indipendentemente bloccate.

## Proposta per mantenere una password per chat dell'utente

Chiamare **sessione Oblivion** l'ambiente protetto: contenitore LUKS2, password, filesystem, VM e processi. Gli ID di sessione OpenCode al suo interno sono dati applicativi, non autorità che possono sbloccare altri ambienti.

Per l'MVP, una sessione Oblivion espone una conversazione principale dell'utente. Eventuali sottosessioni tecniche degli agenti sono parte della stessa attività e dello stesso volume; non ricevono password distinte. Questa distinzione va mostrata chiaramente nella UX e confermata prima dell'implementazione. Se il guest crea altri ID o viene compromesso, essi rimangono nel medesimo confine: l'isolamento dagli altri vault non deve dipendere dal contare le righe del DB.

La finestra terminale può restare unica. Un selettore fidato sulla live gestisce gli ambienti; per quello aperto mostra la TUI eseguita dentro la relativa VM. Client e server della VM vengono ricreati a ogni apertura. Non si riutilizza un unico client OpenCode che mantenga in memoria dati di più VM.

| Azione visibile                             | Comportamento proposto                                                                                                                                                             |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Nuova sessione Oblivion                     | Chiudere l'attuale se aperta; UI fidata per nuova password; nuovo contenitore e nuova VM; nuova conversazione principale                                                           |
| Apri una sessione esistente                 | Selettore fidato con identificatori neutri; password di quel volume; avvio del solo ambiente selezionato                                                                           |
| Cambia A → B                                | Interrompere/concludere il lavoro di A, revocare rete e trasferimenti, salvare e chiudere filesystem/VM/mapping, svuotare il terminale; poi sbloccare B                            |
| Nuova chat o elenco chat della TUI upstream | Adattare i comandi dell'utente al selettore Oblivion; non presentarli come creazione o apertura di vault se operano soltanto sul DB locale                                         |
| Sessioni figlie dei tool                    | Restano nell'ambiente corrente, con medesimo filesystem e medesima password; nessuna apertura automatica di altri vault                                                            |
| Fork di una conversazione                   | Per l'MVP disabilitare il fork utente upstream finché non è definita la semantica; un duplicato protetto indipendente richiede copia esplicita, nuova password e nuovo contenitore |
| Chiudi finestra/riavvia solo OpenCode       | Non dichiarare la sessione bloccata finché il controller non completa la chiusura dell'intero ambiente                                                                             |

La UI dell'agente può esprimere una richiesta di cambio ambiente, ma non può sostituire l'approvazione sulla UI fidata o fornire password a un comando arbitrario. Nascondere un pulsante non è un controllo di sicurezza: API locali, plugin e processi restano confinati grazie al volume e alla VM, non grazie alla UI.

Nel generale rimane soltanto un indice minimo di identificatori opachi, senza titoli, anteprime o indici dei messaggi. Dopo lo sblocco si può mostrare il titolo interno della sessione aperta. Il requisito di titoli visibili a sessioni chiuse richiederebbe una scelta esplicita diversa sulla riservatezza dei metadati.

L'alternativa più vicina a OpenCode invariato è un **vault con molte chat**, tutte con la stessa password. È più semplice da integrare ma cambia il requisito: le chat di quel vault si sbloccano insieme. Non è assunta come scelta dell'utente.

## `/tmp` e il momento della pulizia

Con `/tmp` su `tmpfs`, i file spariscono quando quell'istanza di filesystem viene smontata o la VM termina e viene ricreata. Non serve attendere il riavvio del computer: il cambio sessione di Oblivion distrugge già la VM precedente. Riavviare soltanto il processo OpenCode nella stessa VM, invece, non svuota automaticamente `/tmp`.

Il [kernel documenta](https://docs.kernel.org/filesystems/tmpfs.html) che tmpfs può usare swap. Il piano esclude swap su disco, ibernazione e snapshot della RAM sia sul guest sia sulla live, e richiede quote. La scomparsa logica del filesystem non è una dichiarazione di cancellazione forense immediata di ogni byte in RAM.

Se `/tmp` è una directory su disco, la cancellazione dipende dalle regole della distribuzione e dall'applicazione. [systemd-tmpfiles](https://www.freedesktop.org/software/systemd/man/latest/tmpfiles.d.html) può pulire al boot o per età; il nome `/tmp` e la cifratura LUKS non cancellano i contenuti da soli. Riavviare e riaprire lo stesso volume non garantisce che siano spariti. Per Oblivion si mantiene quindi `/tmp` in RAM; anche `/var/tmp` e le altre cache indicate dal piano hanno una destinazione esplicita.

## Verifiche da aggiungere al prototipo

- A chiusa e B aperta: nessun processo/server/descrittore della VM A ancora vivo; nessun titolo, cronologia o output A nel client B.
- Creare molti ID, figli o fork tramite le API del guest A non permette di elencare, leggere o sbloccare i vault B/C.
- Nuova chat, cambio, uscita, riavvio client e arresto forzato non vengono confusi con lo stato LUKS.
- Un canary in `/tmp` sopravvive al solo riavvio dell'applicazione, ma non alla ricreazione della VM; un documento salvato nel vault persiste dopo riapertura con password.
- Errore di chiusura di A: B rimane bloccata. Nessuna VM sospesa usata come scorciatoia per il cambio.
