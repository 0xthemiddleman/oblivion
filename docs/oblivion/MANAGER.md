# Manager fidato e console: primo prototipo

24 ottobre 2026. `oblivion-manager` è una TUI Rust separata dal guest e dal
motore OpenCode. Il ciclo completo del manager è stato provato sul Mac ARM nel
laboratorio Debian: **9 controlli superati in 95,09 secondi**. Le
[evidenze](manager-evidence.json) registrano gli hash di sorgenti, binari e immagini.
Non è ancora una live installabile né la TUI di conversazione definitiva.

## Flusso implementato

Il manager parte con logo ASCII viola e stato letto dal controller. Se il
generale è chiuso, Invio apre il campo password nascosto. Dopo lo sblocco mostra
solo identificatori neutri delle sessioni; nessun titolo o contenuto della chat
è copiato nell'indice amministrativo.

| Comando | Effetto |
| --- | --- |
| Invio sul generale chiuso | Password generale; nessuna sessione aperta automaticamente |
| N | Nuova password e conferma; crea il volume e apre la nuova VM |
| J/K o frecce, poi Invio | Seleziona un identificatore; chiede la password e apre quel vault |
| Ctrl+] nella console | Chiede la chiusura della VM; torna alla lista solo dopo la chiusura LUKS |
| C con sessione ancora aperta | Riprova la chiusura ordinata |
| F, poi seconda conferma F | Arresto forzato esplicito, con avviso sulla possibile perdita di modifiche |
| B | Blocca sessione e generale |
| Q | Blocca entrambi e termina il manager |

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

La conversazione usa OpenCode compilato, streaming e shell reali, con provider
sintetico su loopback del guest. Il launcher del guest resta la console di prova:
non è ancora la TUI OpenCode interattiva. La VM esterna usa HVF; quelle interne
TCG, che non viene presentato come una garanzia di isolamento.

## Lavoro ancora necessario

- Adattare la TUI OpenCode, rimuovere schede/comandi superflui e collegare
  `session.new`/`session.list` al flusso fidato, senza concedere autorità al guest.
- Completare input incollato e multilinea, ridimensionamento, ripristino del
  rendering dopo riconnessione e verifica visiva sulla console Linux fisica.
- Gestire avvio automatico, crash/arresto del manager e confinamento dei servizi
  nella live. Un crash del processo non garantisce ancora il blocco automatico.
- Aggiungere effimero completo, impostazioni, broker/rete e trasferimenti.
- Completare animazione opzionale, accessibilità e flusso di prima preparazione.

Le prove non dichiarano invulnerabilità del parser o del manager, non coprono
tutte le sequenze terminale e non sostituiscono revisione indipendente o collaudo
Linux/KVM. La [roadmap](IMPLEMENTATION.md) mantiene questi gate aperti.
