# Manager fidato e console: primo prototipo

24 ottobre 2026. `oblivion-manager` è una TUI Rust separata dal guest e dal
motore OpenCode. Il ciclo completo del manager è stato provato sul Mac ARM nel
laboratorio Debian: **9 controlli superati in 95,09 secondi**. Le
[evidenze](manager-evidence.json) registrano gli hash di sorgenti, binari e immagini.
È ora collegato anche alla prima TUI di conversazione ridotta, con una prova distinta descritta sotto. Non è ancora una live installabile né l’interfaccia definitiva.

## Flusso implementato

Il manager parte con logo ASCII viola e stato letto dal controller. Se il
generale esiste ed è chiuso, Invio apre il campo password nascosto. Se manca,
propone la prima creazione con conferma della password. Dopo lo sblocco mostra
solo identificatori neutri delle sessioni; nessun titolo o contenuto della chat
è copiato nell'indice amministrativo.

La lista mostra anche spazio disponibile e dimensione della prossima sessione.
I apre la vista separata dei tentativi incompleti; D e una seconda conferma D
eliminano solo il tentativo selezionato. La preparazione generale interrotta ha
una schermata dedicata. Dettagli e prove in [recupero della creazione](STORAGE-RECOVERY.md).

| Comando                      | Effetto                                                                    |
| ---------------------------- | -------------------------------------------------------------------------- |
| Invio sul generale chiuso    | Password generale; nessuna sessione aperta automaticamente                 |
| N                            | Nuova password e conferma; crea il volume e apre la nuova VM               |
| J/K o frecce, poi Invio      | Seleziona un identificatore; chiede la password e apre quel vault          |
| Ctrl+] nella console         | Chiede la chiusura della VM; torna alla lista solo dopo la chiusura LUKS   |
| C con sessione ancora aperta | Riprova la chiusura ordinata                                               |
| F, poi seconda conferma F    | Arresto forzato esplicito, con avviso sulla possibile perdita di modifiche |
| B                            | Blocca sessione e generale                                                 |
| Q                            | Blocca entrambi e termina il manager                                       |

Con il generale sbloccato e nessuna sessione aperta, **G** gestisce le
[chiavi Abliteration nominate](CREDENTIALS.md): aggiunta nascosta, rinomina,
sostituzione, predefinita e rimozione confermata. **A** e **X** nella schermata
principale impostano o rimuovono la chiave selezionata. La UI mostra nomi e
stato, mai i segreti. Con più chiavi propone la scelta prima dell'apertura;
in readonly permette solo la selezione in RAM. Il controller consegna al
gateway esclusivamente quella selezionata.

Il nuovo codice aggiunge **R** per il [percorso Tor/SOCKS](KEYS-AND-EGRESS.md):
profilo salvato nel generale oppure scelta temporanea fino al suo blocco,
con credenziali nascoste. Il cambio richiede chat chiusa. Questa schermata è
compilata sul Mac e in [Debian ARM64 tramite CI](linux-ci-evidence.json);
le nuove prove interattive Debian sono preparate ma ancora ineseguite.
Le evidenze precedenti del manager
non coprono il nuovo menu. Nessuna modifica alla chat TUI upstream.

Le password si inseriscono fuori dalla VM, senza eco né history; i buffer
posseduti dal manager sono azzerati e il processo Linux disabilita i core dump
e l'accesso tramite dump del processo. La creazione richiede almeno 16 byte;
il campo accetta al massimo 1024 byte e rifiuta l'input eccedente. Questo non è
una garanzia di azzeramento immediato di tutte le copie effettuate dal kernel.
Il manager normale rifiuta root e sistemi diversi da Linux. `--preview` funziona
anche su Mac, mostra solo dati sintetici e non contatta il controller.

Gli errori di operazione non sono trasformati in uno stato «bloccato». La UI
rilegge lo stato derivato dal kernel; una VM non responsiva mantiene il volume
aperto e il blocco delle altre sessioni. Il ritorno dalla sola console non
equivale alla chiusura: il manager attende la conferma del controller.

Le nuove [unità di sistema e il recupero](HOST-SERVICES.md) supervisionano manager
e controller. Se il recupero fallisce, il manager mostra **RECUPERO NECESSARIO**;
il controller vieta nuove aperture anche dopo un riavvio della UI. B riprova il
blocco ordinato; F seguito da una seconda F arresta forzatamente la VM e tenta
il blocco di entrambi i volumi. Annullare conserva la sessione aperta. Sotto
systemd, Q blocca tutto e riavvia il manager sulla schermata iniziale.

## Confine della console

Il controller verifica l'UID del manager e gli passa un descrittore connesso
alla sola console seriale, attraverso `SCM_RIGHTS`. Il nuovo messaggio IPC
`console` non accetta sessione, password, percorso o comando. La directory privata
di QEMU e il socket QMP restano inaccessibili all'UID del manager. Il client
verifica inoltre che il peer del socket di controllo sia root.

La UI ricostruisce le celle del terminale guest con
[`vt100` 0.18.1](https://docs.rs/vt100/0.18.1/vt100/), fissato nel lockfile.
Il renderer emette soltanto posizionamento e colori generati localmente.
Non inoltra OSC, DCS, titolo finestra, clipboard, cambio schermo o richieste di
ridimensionamento del guest. Le risposte alle query ammesse vengono generate
dallo stato del parser e inviate al guest. Le prime tre righe rimangono riservate
all'intestazione fidata e al comando Ctrl+]. Non viene mantenuto uno scrollback
del bridge; la cronologia della chat appartiene al guest.

Sono stati verificati attacchi alle escape sequence, input frammentato e OSC
lunghe, con schermi 80×24 e 32×10. La dipendenza usa `vte` senza feature `std`,
quindi il suo buffer OSC è limitato. Il renderer usa colori ANSI per la console
Linux; i colori RGB del guest vengono convertiti alla palette.

## TUI di conversazione e richieste al manager

La chat mantiene i componenti, la grafica e le interazioni upstream di OpenCode;
sono ammessi rimozioni mirate, branding leggero e collegamenti al manager,
senza redesign del prompt, dei messaggi o delle viste dei tool. Il manager
host resta separato e ospita le schermate amministrative.

Il profilo compilato limita palette, scorciatoie e slash command alle azioni di
conversazione, modelli Abliteration, strumenti e navigazione dei messaggi. Le
schede sono spente; plugin TUI esterni, editor esterno, clipboard condivisa,
apertura diretta dei link, devtools e installazione/connessione di provider non
sono disponibili nella UI. Le scelte sono imposte dal profilo compilato anche se
la configurazione prova a riabilitarle. Gli strumenti dell'agente rimangono quelli
di OpenCode; la riduzione della UI non è un firewall o un controllo delle API.

`/new`, `/sessions` e `/exit` chiedono rispettivamente creazione, selezione e
chiusura. Anche `exit`, `quit`, `:q` e le scorciatoie di uscita seguono il percorso
fidato. Il messaggio OSC `777;oblivion;<azione>` è soltanto un suggerimento del
guest: contiene un enum, mai password, percorsi o comandi. Qualsiasi processo nel
guest può emetterlo. Il manager mostra la richiesta fuori dall'area guest e
richiede un nuovo Ctrl+] dalla propria TTY; Esc torna alla chat. Dopo la conferma
attende la chiusura della VM e del mapping, e soltanto dopo mostra la lista o la
nuova password. Il guest non ottiene il socket amministrativo.

Il decoder conserva i delimitatori di incolla anche fra letture separate. Ctrl+]
all'interno dell'incolla è dato, non conferma; Esc singolo viene inoltrato dopo il
timeout della sequenza. Queste prove coprono sequenze delimitate ordinarie e
frammentate; interruzioni, reconnect e paste esteso durante i cambi di schermata
restano nella matrice da completare. Non si dichiara autenticità hardware di un
tasto ricevuto da una PTY: l'host rimane nel confine fidato.

Nel lancio previsto la CLI usa un'istanza standalone e riprende la conversazione principale
più vecchia nel vault. Le sottosessioni tecniche restano accessibili nello stesso
ambiente. Un primo messaggio in un vault nuovo crea la conversazione; non vengono
copiati titoli nel generale. Dettagli e limiti in [SESSION-MODEL.md](SESSION-MODEL.md).

La [prova interattiva](tui-evidence.json) ha superato **8 controlli in 134,01 secondi**. Esegue il binario compilato e il manager
UID 1000 su una PTY reale, con provider sintetico nel guest e nessuna NIC:

- Tool shell e risposta in A, passaggio a B e assenza dei dati di A.
- Riapertura di A con la stessa cronologia e documento, senza i temporanei RAM.
- Richieste nuova/cambia/chiudi, Esc, Ctrl+] incollato e conferma dalla TTY host.
- Password non presenti nell'output, due mapping chiusi e marcatori assenti nel
  disco spento.

Le schermate PNG derivano dalle celle effettive della PTY del laboratorio;
l'harness attende la fine del ridisegno, senza costruire una copia grafica della
UI. Il guest è fissato a 100×27 sotto tre righe fidate; i test dei componenti
coprono anche 44 colonne. Il ridimensionamento della console seriale non è ancora
implementato. Il bridge può ancora mostrare ridisegni intermedi e la palette a 16 colori perde alcuni bordi a basso contrasto: sincronizzazione del rendering e accessibilità restano da rifinire. Restano etichette e comportamento degli agenti upstream da adattare,
oltre a importazione file, impostazioni, gateway, varianti sperimentali della UI e gestione dei guasti.

## Riprodurre sul Mac

Anteprima nativa senza volumi o password:

```sh
cargo run --manifest-path packages/oblivion-host/Cargo.toml \
  --bin oblivion-manager -- --preview
```

Per la prova reale, preparare il binario come nel
[profilo privacy](PRIVACY-PROFILE.md), quindi:

```sh
docker build --file script/oblivion/Dockerfile.vm-lab \
  --build-arg GUEST=opencode-guest --build-arg HOST=manager-host \
  --target artifacts --output type=local,dest=artifacts/oblivion-manager/image .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-manager/image \
  --output artifacts/oblivion-manager/my-new-run
```

Il laboratorio avvia il manager con UID 1000 su una PTY reale. Inserisce una
password sbagliata, poi quella corretta, crea A e B dalla UI, riprende la chat
di A, ferma QEMU con SIGSTOP e verifica annullamento e conferma della chiusura
forzata. Infine controlla che entrambe le cifrature siano chiuse e che nessuna
password sintetica compaia nell'output della TTY. Il runner cerca i marcatori
anche nel disco spento: nessuna corrispondenza osservata.

Questa prima variante mantiene il launcher a comandi del laboratorio. La variante seguente esegue invece la TUI interattiva compilata. La VM esterna usa HVF; quelle interne
TCG, che non viene presentato come una garanzia di isolamento.

Per la variante interattiva:

```sh
docker build --file script/oblivion/Dockerfile.vm-lab \
  --build-arg GUEST=tui-guest --build-arg HOST=tui-host \
  --target artifacts --output type=local,dest=artifacts/oblivion-tui/image .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --output artifacts/oblivion-tui/my-new-run
```

Per la suite TUI completa, usare Node reale nel container: i test upstream del
loader ESM non funzionano con lo shim Node di Bun.

```sh
docker build -f script/oblivion/Dockerfile.tui-tests -t oblivion-tui-tests:v2.1.4 .
docker run --rm --network none --log-driver none \
  --mount type=bind,source="$PWD",target=/work --workdir /work/packages/tui \
  -e HOME=/tmp/oblivion-tui-tests oblivion-tui-tests:v2.1.4 \
  bun test --define OBLIVION_PROFILE=true --timeout 30000 test/oblivion.test.tsx --rerun-each 10
```

Per cambiare soltanto il test Python sul Mac, aggiungere `--refresh-tui-test` al
runner. L'opzione è accettata solo con un'immagine `tui-host`: aggiunge un archivio
CPIO all'initramfs del laboratorio e monta il solo script di collaudo dalla RAM
sopra quello della root host. Conserva l'ordine degli hook Debian. Il formato a
più archivi è [documentato dal kernel](https://docs.kernel.org/driver-api/early-userspace/buffer-format.html).
Non condivide un filesystem Mac e non modifica guest o controller. Le evidenze
separano manifest dell'immagine, hash dell'initramfs di prova e hash dello script
eseguito, verificato dall'interno della VM. Non è un'opzione dell'avvio di prodotto.

La suite ordinaria si esegue nello stesso container con `bun run test`: profilo
Oblivion compilato fuori, regressioni upstream attive. Le prove dedicate hanno
superato 30 esecuzioni, la suite ordinaria 1.546 test senza errori e il controllo
root 34 task. I quattro test Rust della console passano su Mac e nel compilatore
Linux del laboratorio. Tutte le credenziali e i dati di prova sono sintetici.

## Lavoro ancora necessario

- Completare branding leggero e terminologia mirata, percorso dei file e
  inventario dei comandi residui, conservando grafica e componenti della chat upstream.
- Completare input incollato e multilinea, ridimensionamento, ripristino del
  rendering dopo riconnessione e verifica visiva sulla console Linux fisica.
- Integrare le unità di sistema nella live e completare guasti I/O/pubblicazione,
  OOM, shutdown completo e power loss. Il laboratorio copre ora i crash dei
  processi di manager/controller e il recupero con VM non responsiva.
- Aggiungere effimero completo, impostazioni, broker/rete e trasferimenti.
- Completare animazione opzionale, accessibilità e flusso di prima preparazione.

Le prove non dichiarano invulnerabilità del parser o del manager, non coprono
tutte le sequenze terminale e non sostituiscono revisione indipendente o collaudo
Linux/KVM. La [roadmap](IMPLEMENTATION.md) mantiene questi gate aperti.
