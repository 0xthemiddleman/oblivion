# Adapter compilato attraverso il broker

Il vero binario Oblivion completa ora una conversazione sintetica attraverso
il broker Rust: riceve una chiamata `shell` in più eventi SSE, esegue lo strumento,
invia il risultato, riceve testo in streaming e riprende la stessa sessione con
una seconda esecuzione del client. La prova usa il protocollo Abliteration
OpenAI-compatible già presente nel fork.

## Percorso verificato

```text
CLI compilata + server standalone
  → HTTP locale con token della sessione
  → broker Rust e validazione del payload
  → SOCKS5 con hostname e credenziale di isolamento
  → TLS verificato per api.abliteration.ai
  → provider sintetico nel test
```

Il provider vede la chiave sintetica del broker, non il token locale nel corpo
della conversazione. Il test controlla origin, modello, header Authorization,
hostname remoto SOCKS, porta e credenziale di isolamento. Il certificato sintetico
viene aggiunto soltanto dal test interno Rust al suo client HTTP. Il costruttore
e il binario di produzione non acquistano opzioni per cambiare origin, CA o TLS.

La chiamata allo strumento contiene argomenti divisi in due eventi. Lo strumento
scrive un documento sintetico e legge il contesto allegato. Il provider verifica
che il risultato sia presente nella richiesta successiva, e il test controlla
il documento effettivamente scritto. La seconda esecuzione usa lo stesso ID
di sessione e deve inviare il contesto e la risposta precedenti.

I test ispezionano i file dello spazio applicativo per token, chiave del provider,
log, heap snapshot e CPU profile. Le directory sono in RAM nella VM e vengono
rimosse al termine. Questa scansione riguarda lo spazio creato dal test, non
costituisce una nuova verifica di ogni syscall o percorso del sistema.

## Revoca durante la risposta

Una prova separata invia una risposta lasciando il flusso SSE aperto, senza
finish, `[DONE]` o EOF. Aspetta `step_start` dal client: nel percorso osservato
questo evento nasce dopo il primo output dell'adapter. Solo allora revoca il
broker, attende la fine dei task HTTP e verifica la chiusura della specifica
connessione TLS del provider. Il listener deve scomparire e il client deve
terminare riportando un errore.

Il primo collaudo ha rilevato che il broker chiudeva correttamente, ma il client
continuava ad attendere: il criterio upstream consente dieci retry con circa
84 secondi di attesa nominale. Nel **profilo compilato Oblivion**, `ModelRetry`
ora rifiuta i retry automatici delle richieste al modello. Vale anche per
errori transitori e rate limit: un nuovo invio richiede l'utente. Le normali
chiamate successive per eseguire strumenti o continuare una risposta rimangono
parte del lavoro richiesto. Il comportamento upstream fuori dal profilo resta
coperto dai test esistenti.

Questa prova revoca direttamente l'istanza reale del broker. La successiva
[suite del ciclo VM](STREAM-LIFECYCLE.md) verifica ora anche controller LUKS,
supervisore e due VM separate. Il provider remoto reale resta da collaudare.

## Laboratorio Mac e cattura

La VM Debian ARM viene avviata con HVF sul Mac, con 2 GiB di RAM, nessuno swap
e la sola interfaccia loopback. Esegue i test come UID 1000 senza nuovi privilegi.
Un binario CLI aggiornato può entrare da un dispositivo virtuale readonly:
viene copiato in RAM, verificato per dimensione/hash e reso readonly prima
dell'esecuzione. Il guest non riceve cartelle del Mac.

Durante i due test del client, uno sniffer `AF_PACKET` esamina i pacchetti in
RAM e conserva solo contatori. L'esecuzione registrata ha **776 pacchetti TCP
IPv4 loopback**, zero UDP/IPv6, pacchetti persi o troncati. I contatori del kernel
corrispondono ai pacchetti letti. Non vengono conservati payload HTTP o token
nella cattura. Le richieste applicative vengono controllate separatamente dal
provider sintetico; questi contatori non dimostrano assenza di tentativi falliti
prima della trasmissione di un pacchetto.

```sh
docker build --platform linux/arm64 --target gateway-artifacts \
  --output type=local,dest=artifacts/oblivion-adapter-broker/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --gateway-bundle artifacts/oblivion-adapter-broker/bundle-new \
  --adapter-binary artifacts/oblivion-adapter-broker/cli-01/cli-linux-arm64/bin/opencode \
  --output artifacts/oblivion-adapter-broker/run-new
```

La build CLI usa il [profilo privacy](PRIVACY-PROFILE.md), con `--oblivion` e
una directory di output nuova. Il runner verifica la copia effettivamente
eseguita. Senza `--adapter-binary` riusa il binario nell'immagine base: la vecchia
immagine conservata contiene ancora il criterio di retry precedente e fallisce
intenzionalmente il nuovo controllo di revoca. Le immagini complete devono
essere ricostruite con il nuovo binario prima del rilascio.

Il [report](adapter-broker-evidence.json) conserva il primo fallimento, gli hash
della build corretta, i 21 controlli Linux e le verifiche native. I due test del
client sono `ignored` nel normale `cargo test`, perché richiedono una CLI
compilata; la fixture VM li esegue esplicitamente e richiede entrambi gli esiti
positivi. Nessuna API key reale viene usata.

Restano da verificare provider e Tor reali, compaction e titoli come casi
dedicati, allegati completi, limiti/errori durante il ciclo del controller,
proxy dei tool, immagine live e hardware x86/KVM.
