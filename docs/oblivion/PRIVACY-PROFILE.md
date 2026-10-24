# Primo profilo privacy compilato

24 ottobre 2026. Lo sviluppo prosegue sul Mac ARM, con Bun 1.4.4 in Docker e
binario Linux ARM64 compilato dalla pipeline upstream. `--oblivion` fissa nel
binario le restrizioni: `OBLIVION_PROFILE=false` nell'ambiente non le disattiva.
La build ordinaria mantiene il comportamento upstream per confronto e regressioni.
Il [riepilogo delle prove](privacy-evidence.json) registra binario, sorgenti,
immagine del laboratorio e cattura pacchetti.

## Restrizioni implementate

- Il layer applicativo di osservabilità non costruisce logger su file/stderr né
  esportatori OTLP. Può ancora essere creata una directory dei log vuota.
- Heap dump rifiutato anche dall'endpoint HTTP; SIGUSR1 e SIGPROF non avviano
  snapshot o profili CPU. Le segnalazioni d'errore necessarie alla UI rimangono.
- Updater disabilitato, inclusi upgrade espliciti. Gli aggiornamenti dovranno
  essere gestiti fuori dal guest tramite il percorso firmato del prodotto.
- Catalogo incorporato limitato ad `abliteration-ai`; niente refresh remoto o
  discovery automatico dei provider locali. L'adapter nativo continua a usare
  streaming e strumenti senza i plugin provider opzionali.
- Il server CLI non carica configurazione dal progetto o integrazioni `.claude`,
  anche se le variabili d'ambiente provano a riabilitarle. Un documento importato
  non può cambiare il provider attraverso il file di configurazione del progetto.

## Prove eseguite sul Mac

Il binario compilato ha superato sei controlli con provider locale sintetico,
shell reale, server HTTP reale, `strace` e `tcpdump`. La cattura aggiornata contiene 126
pacchetti senza perdite o troncamenti. Nessun tentativo DNS o discovery sulle
porte dei provider locali osservato. Catalogo e OTLP sono indirizzati verso
endpoint esca: non ricevono richieste. Il test include una configurazione di
progetto che tenta di cambiare endpoint, due segnali diagnostici, richiesta
HTTP di heap dump e comando di upgrade.

Il test unitario compila entrambe le varianti e verifica anche il controllo
positivo: il profilo upstream genera davvero log e OTLP, quello Oblivion no.
Dieci ripetizioni sono passate, insieme ai 98 test del package util e ai 34 task
di `bun run check`. Tutti i dati, i prompt e le chiavi sono sintetici.

## Riproduzione

Preparare l'immagine `oblivion-audit:v2.1.4` e le dipendenze come nel
[laboratorio di fase 0](phase-0/REPRODUCE.md). Dalla root del repository:

```sh
docker run --rm --log-driver none \
  --mount type=bind,source="$PWD",target=/work --workdir /work/packages/cli \
  -e HOME=/tmp/oblivion-build-home -e TURBO_TELEMETRY_DISABLED=1 \
  -e OPENCODE_CHANNEL=oblivion-lab -e OPENCODE_VERSION=2.1.4-oblivion.0 \
  oblivion-audit:v2.1.4 bun run script/build.ts \
  --single --skip-install --skip-web-ui --oblivion \
  --outdir=/work/artifacts/oblivion-build

mkdir -p artifacts/oblivion-profile/my-new-run
docker run --rm --network none --log-driver none \
  --cap-add SYS_PTRACE --cap-add NET_RAW \
  --mount type=bind,source="$PWD",target=/work,readonly \
  --mount type=bind,source="$PWD/artifacts/oblivion-profile/my-new-run",target=/out \
  --workdir /work oblivion-audit:v2.1.4 python3 script/oblivion/profile-probe.py
```

Usare una directory di esecuzione nuova. La build può scaricare dipendenze; la
prova è offline. Non passare credenziali o home personali al container.
PCAP, trace e output diagnostici in `artifacts/` appartengono all'harness
sintetico e sono esclusi da Git.

## Limiti e lavoro successivo

La cattura descritta qui riguarda il binario CLI e server. La [prova interattiva](MANAGER.md) copre separatamente la TUI nel guest; non equivale ancora a una cattura di tutti i suoi percorsi di rete.
Il successivo [laboratorio VM](VM-LAB.md) esegue lo stesso binario nel guest e
verifica la riapertura della chat nel volume LUKS. Database, messaggi e output
dei tool rimangono dati persistenti di sessione. Non è una prova di assenza universale di
scritture né un divieto di rete. Plugin esterni, MCP, download degli strumenti,
configurazioni amministrative e percorsi TUI residui richiedono ancora interventi. Il profilo ora disabilita anche plugin TUI esterni, clipboard condivisa, apertura diretta dei link, schede, devtools e comandi esclusi. Mantiene il motore e il toolset upstream.
Le opzioni CLI avanzate, compreso un endpoint server esplicito, non sono ancora tutte eliminate dalla distribuzione. Il launcher del laboratorio usa soltanto il server standalone dentro il guest. Filtrare il catalogo non costituisce una policy che impedisce qualsiasi altro
provider; quella garanzia richiede il gateway esterno.

La chiave fittizia del laboratorio è ancora nell'ambiente del processo; la
chiave reale dovrà stare soltanto nel broker, fuori dal guest e dalla shell.
Non sono state inviate richieste reali ad Abliteration o a Tor.
Lo sviluppo su Mac non rende privato macOS: l'immagine live, il manager fidato,
il completamento del guest e le verifiche su hardware Linux/KVM restano da completare.
