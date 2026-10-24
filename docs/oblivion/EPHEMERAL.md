# Sessioni effimere

Il percorso host e il comando nel manager sono implementati, ma la nuova
prova Linux non è ancora eseguita. Il solo controllo nativo sul Mac non
verifica device mapper, cryptsetup, systemd o la VM di sessione.

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

La prova aggiunta a `vm-services-test.py` usa i servizi effettivi e la TUI
OpenCode compilata con un provider sintetico locale. Deve verificare:

- scelta E nel manager, generale readonly e rifiuto dei vault persistenti;
- file temporaneo su tmpfs, quota riservata, permessi e assenza di mount host;
- scrittura di un documento tramite lo strumento shell della chat;
- chiusura effettiva di VM, mapping e loop device, rimozione del file RAM;
- seconda chat senza documento, temporanei o conversazione della prima;
- crash del manager, recupero e immagine generale interamente identica prima
  e dopo il percorso effimero.

Restano aperti la prova con il gateway attivo, credenziali nuove solo in RAM,
esaurimento della memoria, integrazione nel boot live e verifica dell'intera
USB invariata. Il confronto dell'immagine LUKS generale non dimostra da solo
che nessun altro settore del supporto sia stato modificato.
