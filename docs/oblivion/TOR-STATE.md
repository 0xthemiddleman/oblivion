# Stato Tor nel profilo cifrato

Il gateway importa le guardie di Tor dal volume generale prima di abilitare
la rete. Il servizio host conserva soltanto record operativi selezionati,
tramite il canale virtio privato: il generale non viene montato nel gateway.
La configurazione root `gateway_network: true` abilita questo percorso;
il default `false` conserva il funzionamento offline del laboratorio.

## Dati conservati

`profile/tor-state` è un file root 0600 nel generale LUKS. Contiene i record
`Guard`, `TorVersion` e `LastWritten`. I record comprendono identità e ordine
delle guardie, date di campionamento/conferma e contatori path bias. Sono
metadati operativi persistenti: la garanzia non è assenza di ogni metadato.
Non vengono salvati contenuti delle chat, destinazioni visitate, log, storico
aggregato della banda, cache delle directory o statistiche di costruzione
dei circuiti. Il resto del DataDirectory rimane nel tmpfs del gateway.

La selezione è fissata a **Tor 0.4.9.14** e validata dal crate condiviso
`oblivion-tor-state`, senza accesso a filesystem o rete. Limiti: 128 KiB,
512 record, 2048 byte per record, campi riconosciuti senza duplicati e date
valide. Una versione diversa, un campo Guard sconosciuto o un profilo salvato
non selettivo interrompono l'avvio. Un aggiornamento di Tor richiede una
revisione del formato; lo stato non viene cancellato o migrato implicitamente.

Tor conserva le guardie tra riavvii; azzerarle a ogni chat cambia il suo
comportamento di selezione. I campi riconosciuti sono stati confrontati con
`entrynodes.c` nella [sorgente esatta di Tor 0.4.9.14](https://dist.torproject.org/tor-0.4.9.14.tar.gz).
La [specifica delle guardie](https://spec.torproject.org/guard-spec/)
descrive selezione e persistenza. Il filtro Oblivion rifiuta estensioni
sconosciute anziché fingere di conservarle: è un confine versionato, non un
parser generico per qualsiasi versione Tor.

## Avvio e checkpoint

1. La nuova VM avvia Tor con `DisableNetwork 1`.
2. Il supervisore ferma Tor, importa l'eventuale stato con proprietario UID
   1200 e riavvia il servizio. Verifica la versione tramite il socket control
   Unix e cookie privato. I comandi e i percorsi sono fissi.
3. L'host riceve uno stato selettivo iniziale. Lo salva atomicamente se manca
   o se i record Guard sono cambiati e il generale è scrivibile.
4. Solo dopo questa conferma invia `SETCONF DisableNetwork=0`, se richiesto
   dalla configurazione. `check` e `grant` online restano negati finché Tor
   non dichiara `PROGRESS=100` e l'host non acquisisce uno stato con guardie.
5. Ogni cinque secondi viene acquisito un nuovo checkpoint. La scrittura nel
   generale avviene solo se cambiano i record Guard; il solo `LastWritten`
   diverso non genera una scrittura. Si usano file temporaneo nella stessa
   directory cifrata, fsync, rename e fsync della directory.
6. Il blocco normale revoca il broker, ferma ordinatamente Tor, acquisisce
   l'ultimo stato e attende il salvataggio prima di distruggere la VM.

`READY=1` del servizio significa inizializzazione completata, non bootstrap
Tor completato. Il controller verifica la disponibilità online **prima** di
sbloccare il volume della sessione. Durante l'attesa il manager invita a
riprovare; non apre un collegamento diretto alternativo.

Tor usa `AvoidDiskWrites 0`: le scritture frequenti avvengono nel suo tmpfs.
Nel codice 0.4.9.14, `entrynodes.c` programma il salvataggio delle modifiche
alle guardie dopo 30 secondi; `AvoidDiskWrites 1` porta quel ritardo a dieci
minuti. `statefile.c` aggiorna anche sezioni che il filtro esclude. Il checkpoint
host aggiunge fino a circa cinque secondi in condizioni normali; scheduling,
I/O e guasti possono allungare la finestra. Un crash prima del checkpoint può
perdere le ultime modifiche, mentre l'ultimo file atomico riuscito rimane.
La chiusura ordinata chiede il flush a Tor fermandolo: `SAVECONF` salva la
configurazione, non questo stato. Vedere i
[comandi del protocollo control Tor](https://spec.torproject.org/control-spec/commands.html).

## Sola lettura e guasti

Con generale readonly, lo stato viene copiato in RAM e usato senza salvataggi.
Questa proprietà riguarda il profilo; la sessione completamente effimera è
ancora un gate separato. Senza guardie salvate Tor può campionarne in RAM;
queste scelte andranno perse alla chiusura di un avvio readonly.

Un errore di salvataggio iniziale impedisce l'attivazione della rete. Un
checkpoint periodico fallito termina il proprietario e la VM gateway.
Se fallisce il checkpoint durante il blocco, il gateway viene comunque
fermato ma il controller restituisce `tor_state_not_saved`: il generale non
viene dichiarato chiuso. Un nuovo blocco completa la chiusura. La revoca
urgente senza risposta distrugge la VM senza aspettare un nuovo checkpoint.

## Riproduzione sul Mac

Le prove usano host Debian ARM/HVF e gateway annidato TCG, LUKS reale, systemd
e la root gateway di produzione. La fixture modifica soltanto impostazioni
pubbliche del laboratorio in RAM e usa password e API key sintetiche.

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-tor-state/bundle-new \
  -f script/oblivion/Dockerfile.vm-lab .
docker build --platform linux/arm64 --target gateway-network-artifacts \
  --output type=local,dest=artifacts/oblivion-tor-state/image-new \
  -f script/oblivion/Dockerfile.vm-lab .
PYTHONDONTWRITEBYTECODE=1 python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-tor-state/bundle-new \
  --gateway-image artifacts/oblivion-tor-state/image-new \
  --tor-state --tor-online \
  --output artifacts/oblivion-tor-state/run-new
```

Omettere `--tor-online` per eliminare la NIC dell'host esterno. La variante
online aggiunge una cattura PCAP fuori dalla VM host e avvia Tor pubblico;
non fa richieste al provider. Una pausa esplicita fra fixture e runner consente
di copiare la cattura prima di avviare il gateway online. La scansione dei marcatori riguarda il disco
virtuale cifrato; log seriali e PCAP del laboratorio restano artefatti di test
sul Mac. I risultati sono in [tor-state-evidence.json](tor-state-evidence.json).

Il collaudo online passa otto controlli in 267,78 secondi, inclusi importazione,
riuso readonly, formato non ammesso, spazio esaurito, bootstrap e riapertura.
La cattura iniziale contiene zero pacchetti; quella completa contiene 6763
pacchetti IPv4/TCP e due ARP, senza UDP o IPv6. Passano anche gli otto controlli
della rete di sessione, i 19 del manager/storage e 12 per ciascuno dei guasti
di controllo con frame eccessivo e incompleto. I binari provati sono associati
agli hash dei sorgenti e dei manifest nel report.

Il primo tentativo online aveva passato i controlli nel guest ma fallito
l'analisi della fase iniziale: confrontava orologi distanti circa un'ora.
Quel risultato è conservato come fallito; la prova con pausa e cattura separata
elimina il confronto fra timestamp.

Il collaudo non sostituisce KVM su hardware, perdita improvvisa di alimentazione,
guasti I/O, streaming Abliteration o revisione indipendente. Rimangono da
integrare configurazione Wi-Fi/bridge, proxy dei tool e immagine live finale.
