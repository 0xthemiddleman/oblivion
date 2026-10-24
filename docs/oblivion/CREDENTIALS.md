# Chiavi Abliteration permanenti e temporanee

Il manager fidato permette di salvare più API key nominate, scegliere una
predefinita e selezionare quella da usare prima dell'apertura di una sessione.
Le operazioni amministrative restano separate dalla chat: i componenti TUI
upstream di OpenCode non cambiano.
Il comando **T** nel menu delle chiavi o nella scelta prima dell'apertura
aggiunge e seleziona una chiave solo in RAM, anche con generale in sola lettura.
L'estensione supera [40 controlli del manager e 31 di rete](ram-credentials-evidence.json)
in Debian ARM64 sul Mac, inclusi profilo readonly, esaurimento della quota RAM,
crash e selezione osservata dal provider TLS sintetico.

Il [controller LUKS](HOST-GATEWAY.md) consegna al supervisore della VM gateway
soltanto la chiave selezionata. La chat riceve un [token locale revocabile](SESSION-NETWORK.md),
mai le credenziali del provider, permanenti o temporanee. Il percorso
predefinito è Tor, con [SOCKS5 globale alternativo](KEYS-AND-EGRESS.md)
selezionato esplicitamente.

## Comportamento

- **G — Chiavi:** elenco con nomi, indicatore della predefinita e della selezionata.
  J/K o frecce spostano la selezione; Invio sceglie la chiave, Esc torna indietro.
- Nell'elenco: **A** aggiunge, **R** rinomina, **S** sostituisce il segreto,
  **D** salva la predefinita, **X** rimuove dopo una seconda X di conferma.
- **T — Usa nuova chiave in RAM:** chiede nome e segreto nascosto e attiva
  la nuova chiave. Le voci temporanee mostrano **[RAM]**. Rinomina, sostituzione
  e rimozione di queste voci sono ammesse anche in readonly; non possono
  diventare la predefinita persistente. Chiavi salvate e temporanee restano
  selezionabili dallo stesso elenco.
- I campi delle API key sono nascosti. Sono accettati da 16 a 1024 byte ASCII
  stampabili senza spazi. Il nome visibile è da 1 a 64 byte UTF-8, senza controlli
  o spazi iniziali/finali; ciascuno dei due archivi contiene al massimo 64 chiavi.
- Con più chiavi il manager propone la scelta prima di aprire o creare la
  sessione. La selezione resta in RAM fino al blocco del generale; dopo uno
  sblocco si parte dalla predefinita. Non si salva un legame chat/account.
- **A — Imposta** e **X — Rimuovi** nella schermata principale agiscono sulla
  chiave selezionata. Imposta può creare la prima chiave nell'archivio vuoto;
  con un archivio non vuoto e nessuna selezione occorre prima scegliere da G.
- Rimuovere la chiave selezionata lascia la selezione vuota. Salvare una nuova
  predefinita non scavalca una scelta esplicita in RAM. Non avviene alcuna
  rotazione automatica dopo rimozione, errore del provider o credito esaurito.

L'IPC di stato espone solo nomi e identificatori opachi; a generale chiuso
l'intero elenco e il booleano della credenziale disponibile sono `null`.
Non esiste un'operazione per rileggere i segreti dal manager. Il client passa
il comando JSON `api-keys` su stdin e sul canale locale autenticato, mai in
argv o ambiente. Restano compatibili `set-api-key` e `clear-api-key`.

Una sessione aperta, una VM ancora attiva o il recupero richiesto impediscono
anche via IPC la modifica e il cambio di chiave. Il profilo in sola lettura
permette la selezione e la gestione delle sole chiavi temporanee in RAM.
Il gateway precedente viene fermato e distrutto prima del cambio di credenziale;
un cambio fallito lo lascia fermo.
Scegliere di nuovo la stessa chiave, con gateway già attivo, conserva il suo
processo e il bootstrap Tor in corso. Il blocco globale verifica la fine della
VM gateway prima di chiudere il generale.

## Persistenza e migrazione

L'archivio versionato è `profile/abliteration.keys.json`, nel generale LUKS2,
root 0600 dentro directory 0700. Contiene versione, predefinita e voci con
identificatore casuale, nome e segreto. Il profilo non viene montato nelle VM.
Il file `/run/oblivion/api-key-selection`, root 0600 in RAM, contiene solo
l'identificatore selezionato: assenza significa predefinita, file vuoto significa
nessuna chiave. Può indicare una voce salvata oppure temporanea. Viene
pubblicato con un file `.pending` e rename, conservando la selezione precedente
se la RAM disponibile non basta. File finale e staging vengono eliminati al
blocco e allo sblocco del generale.

L'archivio temporaneo è `/run/oblivion/api-keys.json`, con staging `.pending`,
entrambi root 0600 nella tmpfs privata. Usa lo stesso formato validato del
salvato, ma i due archivi non vengono mai serializzati insieme. La predefinita
visibile appartiene sempre all'archivio persistente; **T** sceglie esplicitamente
la nuova voce temporanea. Il controller continua a validare l'archivio salvato:
una chiave in RAM non nasconde un profilo corrotto. Il blocco e il recupero
dopo crash eliminano archivio RAM, selezione e staging, dopo la chiusura del
gateway. Una chiusura della sola chat conserva le chiavi RAM fino al blocco
del generale. Nessuna operazione converte implicitamente una voce RAM in
credenziale persistente.

Durante lo sblocco il controller legge i dati in chiaro; i blocchi persistenti
sul supporto sono cifrati da LUKS. I segreti e i buffer serializzati usano
`Zeroizing`, senza promessa di cancellazione di ogni copia interna alla RAM.

Un aggiornamento scrive `abliteration.keys.json.pending`, esegue `fsync`,
rinomina sul file definitivo e sincronizza la directory. Un fallimento prima
della rinomina conserva l'archivio precedente. Un nuovo tentativo può eliminare
lo staging privato rimasto; symlink, hardlink e permessi/proprietari inattesi
vengono rifiutati. Il lock globale serializza le operazioni con il lifecycle.

Il vecchio `profile/abliteration.key` viene letto come unica chiave «Importata»
finché manca il nuovo archivio. La sola lettura non migra nulla. Alla prima
modifica, il nuovo archivio viene pubblicato e sincronizzato **prima** di
eliminare vecchio file e staging. Se la pulizia si interrompe, il nuovo formato
rimane autorevole. Un archivio nuovo corrotto produce errore: non riattiva la
vecchia chiave. Anche l'archivio vuoto viene conservato per impedire quel fallback.

La rimozione aggiorna l'archivio e non revoca la chiave presso il provider.
Non garantisce cancellazione fisica dei vecchi blocchi della flash, già protetti
dal volume cifrato. Non modifica alcuna password LUKS.

## Verifica sul Mac

La [prova delle credenziali RAM](ram-credentials-evidence.json) supera 40
controlli storage/manager in 308,53 secondi e 31 di rete in 540,80 secondi.
Passano 34 test host sul Mac e in Debian, Clippy sul Mac, due test della
fixture e `bun run check` (34 task). I 13 marcatori cercati nel disco dei
servizi e i 19 nel disco del ciclo di rete non compaiono in chiaro.
La successiva CI `b86547c` passa tutti i 29 job: ripete i 40 controlli storage
e i 31 di rete su x86, con client ricompilato, senza marcatori in chiaro nei
dischi esaminati. Il report separa questa prova da quella locale ARM.

Il manager reale crea, rinomina, sostituisce e rimuove chiavi temporanee;
le prove coprono generale readonly e scrivibile, archivio RAM non leggibile
da UID 1000, staging con symlink rifiutato, quota tmpfs esaurita senza perdere
archivio o selezione precedente, pulizia dopo SIGKILL e blocco delle modifiche
a chat aperta. Nel ciclo di rete, un profilo senza chiavi salvate resta senza
gateway finché l'utente sceglie una nuova chiave RAM; il provider sintetico
osserva quella credenziale attraverso il client compilato. Il blocco elimina
la chiave e lo sblocco successivo richiede una nuova scelta esplicita.

Il client ARM64 riusato proviene dalla CI `bff2090`; il codice OpenCode non
è cambiato nelle successive modifiche Rust/Python. I binari host correnti
sono ricompilati, con manifest dei sorgenti verificato nel guest. La cattura
esterna copre la prima sessione persistente A, non gli stream effimeri.

## Evidenze precedenti

Le prove usano Debian ARM/HVF, un vero manager non privilegiato sotto systemd,
controller Linux, LUKS reale e guest interni TCG. Tutte le credenziali sono
marcatori sintetici. Il report [multiple-keys-evidence.json](multiple-keys-evidence.json)
riporta build, hash, esiti e limiti; le [evidenze precedenti](credentials-evidence.json)
documentano il primo formato a chiave singola.

La suite dei servizi passa 23 controlli in 224,28 secondi; quella con due VM
passa 15 controlli in 307,78 secondi. Passano inoltre i 13 test Rust del pacchetto
host, clippy e il controllo completo del repository (34 task di typecheck).

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-multiple-keys/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-multiple-keys/bundle-new \
  --output artifacts/oblivion-multiple-keys/services-new
```

La suite dei servizi copre input nascosto, permessi, alias dello staging,
migrazione con spazio esaurito e retry, priorità del nuovo archivio e rifiuto
di quello corrotto, gestione delle chiavi nella vera TUI, scelta prima
dell'apertura, assenza di rotazione alla rimozione, readonly byte-identico e
recupero dopo SIGKILL. Il runner cerca i marcatori sul disco esterno; la fixture
verifica che UID 1000 non legga l'archivio e che la console non mostri i segreti.

La suite con due VM e provider sintetico osserva quale delle due credenziali
raggiunge il server TLS attraverso l'adapter OpenCode compilato e il broker.
Verifica che cambiare chiave distrugga il vecchio gateway, mentre scegliere la
stessa lo conserva, e ripete le prove di revoca durante lo stream e all'uscita
di QEMU. Non esercita Abliteration remoto o Tor online. Restano distinti i gate
per power loss, tracing completo, hardware KVM e provider reale.
