# Chiavi Abliteration nel generale cifrato

Il manager fidato permette di salvare più API key nominate, scegliere una
predefinita e selezionare quella da usare prima dell'apertura di una sessione.
Le operazioni amministrative restano separate dalla chat: i componenti TUI
upstream di OpenCode non cambiano.

Il [controller LUKS](HOST-GATEWAY.md) consegna al supervisore della VM gateway
soltanto la chiave selezionata. La chat riceve un [token locale revocabile](SESSION-NETWORK.md),
mai le chiavi salvate. Il percorso online usa ancora Tor; il
[SOCKS5 globale alternativo](KEYS-AND-EGRESS.md) resta da implementare.

## Comportamento

- **G — Chiavi:** elenco con nomi, indicatore della predefinita e della selezionata.
  J/K o frecce spostano la selezione; Invio sceglie la chiave, Esc torna indietro.
- Nell'elenco: **A** aggiunge, **R** rinomina, **S** sostituisce il segreto,
  **D** salva la predefinita, **X** rimuove dopo una seconda X di conferma.
- I campi delle API key sono nascosti. Sono accettati da 16 a 1024 byte ASCII
  stampabili senza spazi. Il nome visibile è da 1 a 64 byte UTF-8, senza controlli
  o spazi iniziali/finali; l'archivio contiene al massimo 64 chiavi.
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
permette esclusivamente la selezione in RAM. Il gateway precedente viene
fermato e distrutto prima del cambio; una scrittura fallita lo lascia fermo.
Scegliere di nuovo la stessa chiave, con gateway già attivo, conserva il suo
processo e il bootstrap Tor in corso. Il blocco globale verifica la fine della
VM gateway prima di chiudere il generale.

## Persistenza e migrazione

L'archivio versionato è `profile/abliteration.keys.json`, nel generale LUKS2,
root 0600 dentro directory 0700. Contiene versione, predefinita e voci con
identificatore casuale, nome e segreto. Il profilo non viene montato nelle VM.
Il file `/run/oblivion/api-key-selection`, root 0600 in RAM, contiene solo
l'identificatore selezionato: assenza significa predefinita, file vuoto significa
nessuna chiave. Viene eliminato al blocco e allo sblocco del generale.

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
