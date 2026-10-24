# Sessioni effimere

Il percorso host e il comando nel manager superano la
[suite Linux sul Mac](ephemeral-evidence.json): **37 controlli passati** con
device mapper, cryptsetup, systemd e TUI compilata, oltre a **29 controlli
di rete**, inclusi gli stream effimeri. L'host Debian usa HVF;
la VM di sessione usa TCG. Non è una verifica dell'isolamento KVM del prodotto.

Prima dello sblocco, **E** seleziona l'avvio effimero. La password generale
apre il dispositivo e il filesystem in sola lettura, senza replay del journal
in scrittura. Il manager mostra il limite in MiB e avverte che chiudere la
sessione distrugge conversazione e documenti. **N** o Invio apre una chat nuova;
le sessioni persistenti non sono elencate e il controller ne rifiuta l'apertura.
Le chiavi API già salvate possono essere selezionate in RAM.

Il controller crea `/run/oblivion/ephemeral.luks`, root 0600, usando la quota
`session_mib` già configurata. Il percorso runtime deve essere su tmpfs e lo
swap deve essere disattivato. Lo spazio viene riservato prima dell'apertura,
senza fallback su disco; una preparazione fallita va chiusa con Blocca prima
di riprovare. Il controller riusa LUKS2 e la formattazione ext4 delle sessioni
normali, con 64 byte casuali passati soltanto tramite pipe e poi azzerati.
Non chiede né salva una password di sessione e non crea un contenitore sulla
USB. Il file temporaneo di blocchi rimane esclusivamente in RAM.

La VM riceve il solo dispositivo a blocchi `oblivion-session`, come una chat
persistente. Il controller non monta il suo filesystem. Rimangono gli stessi
controlli di mutua esclusione, confinamento QEMU, consegna del token e revoca
del gateway. Il grant delle regole web usa un nuovo identificatore casuale:
non eredita le regole associate a una sessione persistente.

Alla chiusura si revoca la rete, si arresta la VM, si chiude il mapping e solo
allora si rimuove il file RAM. Se il dispositivo è ancora occupato, la chiusura
fallisce e non dichiara distrutta la sessione. Il recupero dopo arresto del
manager/controller usa lo stesso percorso. Un'apertura successiva crea una
nuova immagine e un nuovo segreto; non esiste recupero della chat precedente.
Il riuso di LUKS2 non rende il testo illeggibile durante l'elaborazione in RAM.

La prova in `vm-services-test.py` usa i servizi effettivi e la TUI OpenCode
compilata con un provider sintetico locale. Verifica:

- scelta E nel manager, generale readonly e rifiuto dei vault persistenti;
- file temporaneo su tmpfs, quota riservata, permessi e assenza di mount host;
- quota tmpfs esaurita: rifiuto senza VM o fallback su disco, file parziale
  interbloccato fino alla pulizia e nuova apertura dopo Blocca;
- scrittura di un documento tramite lo strumento shell della chat;
- chiusura effettiva di VM, mapping e loop device, rimozione del file RAM;
- seconda chat senza documento, temporanei o conversazione della prima;
- crash del manager, recupero e immagine generale interamente identica prima
  e dopo il percorso effimero.

La suite di rete supera inoltre tre controlli con gateway e SOCKS/provider
sintetico esterno: token nuovo senza ereditare le regole della sessione A,
stream dal client compilato revocato alla chiusura e seconda chat terminata
dal blocco generale. Controlla la rimozione del disco RAM e del loop device,
il rifiuto del vecchio token e l'immagine generale invariata. La prova viene
eseguita separatamente da quella TUI; non implica una conversazione Tor reale.

Restano aperti credenziali nuove solo in RAM, OOM della memoria fisica,
uso effimero con Tor reale, integrazione nel boot live e verifica dell'intera
USB invariata. Il confronto dell'immagine LUKS generale non dimostra da solo
che nessun altro settore del supporto sia stato modificato.
