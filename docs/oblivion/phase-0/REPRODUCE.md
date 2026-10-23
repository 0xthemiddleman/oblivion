# Riprodurre l'audit di fase 0

L'audit usa esclusivamente dati sintetici. Richiede Git, Python 3, Docker e spazio per le dipendenze del monorepo. Il riferimento eseguito è Linux arm64 in Docker Desktop su macOS; non prova il comportamento su altri kernel/architetture o nella futura live x86-64.

Usare un checkout dedicato: le dipendenze installate nel container sono Linux e non vanno sovrapposte a un'installazione macOS usata per sviluppare. Eseguire i comandi dalla radice del fork. Non avviare `dev:live`: si collegherebbe a un servizio utente preesistente.

## Base e inventario

```sh
git rev-parse v2.1.4
# 68bc26b7da1990ce10ffecfbc1e09eb13aa5d824
python3 script/oblivion/inventory.py
```

Il generatore legge solo i file tracciati nel commit upstream e rifiuta differenze nei sorgenti/manifests/lockfile/catalogo selezionati. Le aggiunte Oblivion non entrano nell'inventario upstream. Vengono prodotti:

- `source-manifest.csv`: file, ambito, righe e SHA-256;
- `io-candidates.csv`: candidati filesystem, DB, logging, rete, processi, ambiente;
- `endpoint-literals.csv`: stringhe URL, **non** una allowlist;
- `runtime-dependencies.csv`: dipendenze della chiusura workspace della CLI, con quelle esterne marcate come non interamente revisionate;
- `upstream.json`: versione, commit, conteggi, espressioni di ricerca, hash del lockfile e del catalogo;
- `abliteration-catalog.json`: voce originale del provider nel catalogo fissato.

Una ricerca lessicale non riconosce ogni possibile API o alias. La verifica manuale e il tracing descritti nel report sono necessari per interpretarla. Una modifica futura ai sorgenti richiederà un nuovo inventario riferito al nuovo commit; non aggirare il controllo di integrità per confrontare versioni diverse.

## Ambiente delle prove

Il Dockerfile fissa Bun 1.4.4 e il digest della sua immagine. I pacchetti di diagnostica apt non sono fissati a uno snapshot: questa è una procedura di ricerca ripetibile, non una build di release bit-identica.

```sh
docker build -f script/oblivion/Dockerfile.audit -t oblivion-audit:v2.1.4 .
docker run --rm --log-driver none \
  --mount "type=bind,source=$PWD,target=/work" \
  --workdir /work -e HOME=/tmp/audit-home \
  oblivion-audit:v2.1.4 bun install --frozen-lockfile --ignore-scripts
```

Build e installazione richiedono rete per le dipendenze. `--ignore-scripts` è deliberato: le prove non necessitano dei lifecycle script dell'intero monorepo. Non sostituisce una revisione dei pacchetti eseguiti. Non passare al container credenziali, HOME dell'utente, socket Docker o configurazioni npm personali.

Durante le prove il sorgente è in sola lettura; solo `artifacts/oblivion-phase0` è montato scrivibile dall'host. Il filesystem del container è effimero. Il container ha `--network none`, logging Docker disabilitato e `SYS_PTRACE` per osservare i suoi processi. Questi flag servono al laboratorio; non sono la sandbox di produzione proposta per Oblivion.

## Prove dinamiche

```sh
sh script/oblivion/run-probes.sh
```

Il wrapper esegue `probe.py`, `temp-probe.mjs` e `summarize.py`. Non legge il profilo OpenCode dell'utente. Ogni scenario ha HOME, TMPDIR e radici XDG nuovi; il provider ascolta solo su loopback del container.

Il provider fittizio usa il formato OpenAI-compatible previsto dall'adapter nativo `abliteration-ai`. La sola chiave è `audit-dummy-key`; il canary è `OBLIVION_AUDIT_SYNTHETIC_7c924e`. Il tool shell scrive un documento, un file con percorso assoluto `/tmp/oblivion-outside-xdg.txt` e output lungo; verifica se il figlio ha ricevuto la chiave fittizia. L'assert sul valore non registra segreti reali.

Scenari:

1. `--help`: osserva effetti dell'inizializzazione.
2. `run --standalone --auto --format json --file context.txt <canary>`: DB normale, conversazione con allegato e tool shell.
3. Stessa conversazione con `OPENCODE_DB=:memory:`: confronta DB e persistenze indipendenti.
4. Stessa conversazione con refresh modelli abilitato: osserva i tentativi di rete aggiuntivi senza consentirli.

Configurazione comune: update disabilitati, snapshot disabilitati, FFF/filewatcher disabilitati, configurazione di progetto e integrazione Claude disabilitate. Il refresh modelli è disabilitato eccetto nel quarto scenario. Queste scelte riducono interferenze e sono parte del perimetro della prova; il report non presume di aver attivato ogni funzione upstream.

La CLI dai sorgenti avvia un server figlio. Per caricare correttamente JSX anche nel figlio il fixture cwd contiene `bunfig.toml` e `tsconfig.json` con il preload OpenTUI e la configurazione CLI. Sono file del laboratorio, non perdite attribuite a OpenCode. Anche stdout/stderr e `syscalls.log` sono prodotti dall'harness e sono esclusi dalla ricerca delle copie canary attribuite all'applicazione; `workspace/context.txt` è l'input e viene escluso.

Gli assert richiedono uscita normale, risposta e ritorno del tool, bearer fittizio, scritture fuori XDG, propagazione della chiave fittizia, log e output persistenti e presenza/assenza del DB prevista. Le prove documentano il comportamento **non protetto** della base: dopo le correzioni di prodotto alcune aspettative dovranno essere invertite in test separati di confinamento.

Gli helper testati separatamente verificano il residuo dopo un errore di `renderer.suspend()` e il file ancora leggibile dopo `BrowserFiles.save`. Non emulano tutta la TUI. Sono diagnostica standalone, non nuovi unit test dell'applicazione.

Risultati completi locali:

```text
artifacts/oblivion-phase0/runtime-results.json
artifacts/oblivion-phase0/<scenario>-<timestamp>/syscalls.log
artifacts/oblivion-phase0/<scenario>-<timestamp>/stdout.txt
artifacts/oblivion-phase0/<scenario>-<timestamp>/stderr.txt
artifacts/oblivion-phase0/temp-results.json
artifacts/oblivion-phase0/temp-syscalls.log
```

Ogni esecuzione crea directory nuove; gli esiti dell'ultima vengono riassunti in `docs/oblivion/phase-0/runtime-summary.json`. PID, porte, timestamp, nomi e hash cambiano tra esecuzioni. Il riepilogo di rete estrae soltanto `bind`/`connect` IPv4: non è un parser esaustivo di strace e non dimostra assenza di altre syscall. Per approfondimenti consultare il trace intero.

`dependency-review.json` registra versioni e hash dei file installati esaminati manualmente. I nomi dei chunk OpenTUI sono legati al lockfile fissato; una nuova versione richiede di aggiornare la revisione, non semplicemente di trovare un chunk con un nome simile.

## Controlli del repository

Il controllo completo richiede un checkout scrivibile per cache Turbo, file di build e typecheck. Rimane senza rete e con HOME sintetica:

```sh
docker run --rm --network none --log-driver none \
  --mount "type=bind,source=$PWD,target=/work" \
  --workdir /work -e HOME=/tmp/audit-home -e TURBO_TELEMETRY_DISABLED=1 \
  oblivion-audit:v2.1.4 bun run check
git diff --check
python3 script/oblivion/inventory.py
```

Esito registrato: check riuscito, 34 task typecheck; lint senza errori ma con warning upstream. Nessun cambiamento ai sorgenti applicativi o al lockfile. Il controllo fallito iniziale su filesystem in sola lettura non è attribuito a un errore upstream: Turbo doveva scrivere la cache.

Non sono stati eseguiti test dell'intero monorepo, chiamate cloud reali, TUI end-to-end, Tor o prove LUKS/VM. I criteri ancora aperti sono esplicitati nell'[audit](AUDIT.md#7-limiti-e-verifiche-ancora-aperte).
