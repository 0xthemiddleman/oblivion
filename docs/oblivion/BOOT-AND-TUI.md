# Avvio e interfaccia minimale di Oblivion

24 ottobre 2026. Direzione approvata dall'utente: Debian minimale senza ambiente desktop, logo ASCII Oblivion viola con animazione leggera, password LUKS2 generale e accesso immediato al manager delle sessioni protette. Una chat principale per sessione Oblivion; una sola sessione aperta. Le sottosessioni tecniche restano nello stesso volume. **Il [manager, il bridge e una prima TUI ridotta](MANAGER.md) sono implementati e provati nel laboratorio Mac ARM. La live, l'animazione, il ridimensionamento e la rifinitura completa del prodotto restano da completare.** Questo documento mantiene il comportamento finale richiesto.

## Un prodotto, due contesti di esecuzione

Il manager fidato gira sulla live e controlla volumi, impostazioni, rete e lifecycle. La TUI di conversazione gira nella VM della sessione aperta, insieme al server OpenCode e ai tool. L'aspetto e i comandi devono essere coerenti, così l'utente percepisce un solo prodotto nel terminale. I processi e gli archivi rimangono separati.

L'interfaccia Oblivion sostituisce quella di un coding agent generalista. Si conservano motore di conversazione, streaming, adapter Abliteration e strumenti upstream; si semplificano navigazione, menu, palette e impostazioni. Togliere comandi dalla UI e disabilitare le capacità non previste nel profilo runtime sono attività distinte: nascondere un comando non impedisce da solo un'operazione via API o shell.

Il frontend del manager usa un utente limitato. Le operazioni privilegiate appartengono al controller con API ristrette. Password del generale e delle sessioni si inseriscono nella UI fidata, prima di entrare nella TUI del guest. Il guest non legge password né chiavi API reali.

## Flusso di avvio concordato

```text
Avvio USB
    |
    v
Logo ASCII OBLIVION, viola
    |
    v
Password del volume generale
    |
    v
Manager delle sessioni protette
    |
    +-- Nuova --> password della nuova sessione --> volume + VM
    |
    +-- Apri ---> password della sessione scelta --> volume + VM
                                                       |
                                                       v
                                              TUI della conversazione
                                                       |
                                                       v
                                            Chiudi e blocca la sessione
                                                       |
                                                       v
                                                    Manager
```

Il logo e il programma di sblocco appartengono all'immagine pubblica verificata. Possono apparire prima di aprire il generale, perché il sistema live non è conservato dentro quel volume. Il generale contiene il profilo privato e i contenitori delle sessioni. Non viene aggiunto un login Linux o un account Oblivion dopo la password generale.

L'avvio normale è il percorso predefinito. Nella stessa schermata di sblocco deve essere possibile scegliere l'avvio effimero **prima** di montare il generale: in quel caso dispositivo e filesystem vengono aperti in sola lettura e il lavoro della chat resta in RAM. Non introdurre un'opzione successiva che prometta retroattivamente zero scritture sul supporto.

La prima preparazione della USB crea il generale e la sua password; non viene distribuito un contenitore con chiave predefinita. Al primo sblocco il manager guida alla configurazione di Wi-Fi e API key; agli avvii successivi usa il profilo salvato, mostrando gli errori di connessione senza aprire automaticamente una sessione. Si può raggiungere il manager anche senza rete. Nessuna richiesta AI precede l'apertura della sessione e l'autorizzazione prevista.

## Identità visiva e schermate

Direzione visiva: sfondo scuro, accento viola, testo leggibile e pochi elementi. Logo solo ASCII per funzionare anche senza font particolari; pulsazione o passaggio di luminosità leggero, disattivabile e senza ritardo obbligatorio. L'animazione si ferma durante l'inserimento della password e non interferisce con l'input. La password non viene stampata, registrata o conservata in history.

Bozza testuale del logo; forma e spaziatura finali da verificare sul terminale:

```text
   ___  ___ _    _____   _____ ___  _  _
  / _ \| _ ) |  |_ _\ \ / /_ _/ _ \| \| |
 | (_) | _ \ |__ | | \ V / | | (_) | .` |
  \___/|___/____|___| \_/ |___\___/|_|\_|

                  O B L I V I O N
```

Il colore è applicato dal renderer, non incorporato nel testo qui. Il renderer deve avere una variante a colori limitati e statica. Il viola va espresso attraverso token di tema dedicati; focus, selezione ed errori continuano a usare i token semantici, senza trasformare ogni elemento in viola.

Bozza del manager dopo lo sblocco generale:

```text
 OBLIVION                                 Rete: pronta

 Sessioni protette

 > Sessione 7C2A                       Bloccata
   Sessione 91F4                       Bloccata

 [N] Nuova   [Invio] Apri   [I] Impostazioni
 [B] Blocca Oblivion        [S] Spegni
```

L'indice delle sessioni chiuse usa identificatori neutri. Titoli delle conversazioni, anteprime e nomi dei documenti non vengono copiati nel generale per abbellire la lista. Il titolo interno può essere mostrato dopo lo sblocco della sessione. Le lettere dei comandi sono una bozza, da verificare rispetto a tastiera e scorciatoie riservate.

Il manager distingue generale aperto, sessione chiusa, apertura, sessione aperta e chiusura. Una chiusura incompleta resta visibile e impedisce l'apertura della successiva. Nella modalità effimera mostra chiaramente il lavoro temporaneo e non crea contenitori persistenti.

La schermata di conversazione ha soltanto: identificazione della sessione aperta, messaggi, campo di composizione, file di contesto, stato del lavoro e accesso ai comandi essenziali. Output tool e sottosessioni restano consultabili senza una barra di schede per chat indipendenti. Un'interruzione del modello e il blocco del volume sono azioni distinte.

## Comandi: mantenere, sostituire, togliere

La riduzione generale dell'interfaccia è concordata. La seguente è la lista iniziale di implementazione: non elimina strumenti dell'agente né dichiara già disabilitate le funzioni upstream.

| Funzione                                                                                        | Comportamento Oblivion                                                                                                                  |
| ----------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Nuova sessione, `/new`, `session.new`                                                           | Richiedere il passaggio al manager fidato; completare la chiusura dell'attuale, quindi creare volume, password e VM nuovi               |
| Elenco/cambio, `/sessions`, `session.list`                                                      | Selettore Oblivion, con chiusura completa prima di sbloccare un altro volume                                                            |
| Uscita, blocco, spegnimento                                                                     | Azioni esplicite del controller; uscire dalla sola TUI non basta a dichiarare bloccata la sessione                                      |
| Chat e contesto                                                                                 | Conservare invio, allegati, consultazione dei file importati, ricerca nella chat aperta, navigazione dei messaggi                       |
| Esecuzione agente                                                                               | Conservare stop, ripresa, stato, autorizzazioni dei tool e consultazione delle sottosessioni tecniche                                   |
| Importa / esporta                                                                               | Entrata e uscita esplicite tramite il servizio di trasferimento; sostituire l'export upstream che crea semplicemente un file temporaneo |
| OnionShare                                                                                      | Invia/ricevi attraverso il servizio dedicato; collegamento dalla TUI coerente con il piano dei trasferimenti                            |
| Modello                                                                                         | Solo modelli Abliteration ammessi, senza selezione o autenticazione di altri provider                                                   |
| Impostazioni globali                                                                            | Wi-Fi, credenziale API, policy rete e preferenze del terminale nel manager fidato                                                       |
| Nuova scheda, switch rapido, riapri scheda                                                      | Togliere la navigazione tra chat principali dello stesso server dalla UX ordinaria                                                      |
| `session.clear`, `/clear`                                                                       | Togliere l'azione upstream che porta a una nuova chat nello stesso ambiente; nessuna ambiguità con cancellazione o blocco               |
| Progetti, Git, worktree, selezione coding agent                                                 | Togliere le sezioni di navigazione orientate allo sviluppo dalla UI principale; i tool restano disponibili nel guest secondo policy     |
| Fork utente, condivisione chat, pairing/web, accesso remoto                                     | Escludere dal profilo MVP; eventuali funzioni future richiedono semantica esplicita per isolamento ed export                            |
| Connessione di provider, aggiornamento OpenCode in chat, installazione automatica di estensioni | Escludere dalla UI e dal profilo ordinario; manutenzione separata a sessioni chiuse                                                     |
| Devtools, profiling, trace, pannelli di debug                                                   | Escludere dalla distribuzione normale; strumenti di audit soltanto nel laboratorio                                                      |

La palette rimane piccola e contiene solo le azioni disponibili nel contesto corrente. Le richieste di apertura, importazione o modifica della rete provenienti dal guest non sono autorizzazioni: il controller le presenta sulla UI fidata e applica il flusso già previsto. Si usa il medesimo confine anche per API e processi, senza affidarsi al nome di un comando.

## Debian minimale e collocazione dei pacchetti

La base scelta è Debian Live senza desktop environment e senza display manager. Il [manuale Debian Live](https://live-team.pages.debian.net/live-manual/html/live-manual/the-basics.en.html) documenta la costruzione di una live di base senza X.org; le [liste pacchetti di live-build](https://live-team.pages.debian.net/live-manual/html/live-manual/customizing-package-installation.en.html) permettono di selezionare il contenuto. Versione, repository e pacchetti effettivi saranno fissati nella build; non si assume che i default di live-build siano quelli desiderati.

| Ambiente                | Software da includere                                                                                                                                                         |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Live fidata             | Kernel e firmware necessari all'hardware scelto, console/input, servizi di avvio, cryptsetup/LUKS2, controller e manager, QEMU/KVM, gestione dispositivi e connessione fisica |
| VM della sessione       | Sistema guest minimo, client/server Oblivion, dipendenze native richieste, shell e utilità necessarie ai tool, parser e convertitori documentali selezionati                  |
| Gateway / broker        | Tor, enforcement di rete e servizio API con sole dipendenze necessarie; nessun mount dei volumi utente                                                                        |
| Ambiente trasferimenti  | OnionShare CLI e componenti di import/export; avvio soltanto quando richiesto                                                                                                 |
| Laboratorio di sviluppo | Build toolchain, strace, tcpdump/tshark e fixture; esclusi dall'immagine utente salvo dipendenze di prodotto motivate separatamente                                           |

Gli strumenti per lavorare sui documenti appartengono al guest: non occorre installarli anche nel dominio fidato. Non si distribuisce l'intero checkout di sviluppo con tutte le dipendenze del monorepo. Nessuna installazione automatica al primo avvio per far funzionare chat, parser o strumenti base. Le immagini necessarie sono già presenti sulla USB.

Il primo prototipo prova la console senza stack grafico: avvio, colori, Unicode/ASCII, input multilinea, tastiera, ridimensionamento dove disponibile e gestione delle escape sequence del guest. L'assenza di un desktop è decisa; la compatibilità di OpenTUI su quella console è ancora un gate. Un eventuale backend terminale aggiuntivo richiede una scelta documentata in base alle prove, non l'introduzione silenziosa di un desktop.

Non sono ancora dichiarati dimensione dell'immagine, consumo RAM o tempo di boot: includono anche le VM, i firmware e le dipendenze dei tool. Si misurano sul target x86-64/UEFI previsto dal piano.

## Ordine di realizzazione

1. Prototipo del rendering console con logo, sblocco simulato e manager su dati sintetici; ogni simulazione deve essere indicata come tale e non chiedere password reali.
2. Collegamento del manager al controller LUKS/VM e verifica delle transizioni reali, incluso errore di chiusura. La UI dei segreti resta esterna al guest.
3. Profilo TUI Oblivion: palette ridotta, niente schede di chat indipendenti, comandi di sessione collegati al controller e funzioni runtime non previste disabilitate.
4. Immagine Debian riproducibile con i pacchetti distinti per ambiente; prove di boot e della TUI su hardware reale, oltre alle prove nella VM di sviluppo.

Il motore di inferenza e gli strumenti non vengono riscritti per ottenere questo flusso. L'implementazione si concentra sul manager, sull'integrazione del controller, sulla superficie della TUI e sul profilo di distribuzione.
