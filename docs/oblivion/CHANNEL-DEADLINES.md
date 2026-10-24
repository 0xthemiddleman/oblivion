# Scadenze degli scambi privati host

Il controller usa una scadenza monotona per ogni scambio sui socket privati.
Prima, i timeout del socket valevano per la singola lettura o scrittura:
ricevere periodicamente pochi byte poteva prolungare un frame incompleto.
Ora header, corpo e risposte appartenenti allo stesso scambio consumano un
unico tempo disponibile. I byte ricevuti non lo rinnovano.

La modifica è in `packages/oblivion-host/src/channel.rs`, riutilizzata da
controller, client del manager, proprietario del gateway e controllo QEMU.
Non aggiunge log, file temporanei, processi o dipendenze runtime.

## Limiti applicati

| Confine                                 | Tempo complessivo     | Contenuto                                                                                 |
| --------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------- |
| Client → controller storage             | 10 s                  | Lunghezza, JSON e password/chiave; stessa scadenza durante tutta l'ammissione lato server |
| Risposta del controller                 | 10 s per inviarla     | Trasferimento eventuale del descrittore console e JSON                                    |
| Attesa della risposta lato manager      | 120 s                 | Attesa dell'operazione, descrittore eventuale e JSON fino a EOF                           |
| Ammissione del proprietario gateway     | 5 s                   | Lunghezza e corpo del frame root-only                                                     |
| Risposta del proprietario gateway       | 5 s                   | Frame completo dopo l'elaborazione                                                        |
| Worker storage → proprietario gateway   | 10 s, 30 s per `stop` | Richiesta completa e conferma dello stato atteso                                          |
| Avvio del canale gateway                | 60 s                  | Ripresa QMP e frame `ready`                                                               |
| Proprietario host → supervisore gateway | 30 s per operazione   | Richiesta e conferma; anche configurazione, stato e snapshot Tor                          |
| Consegna del token di sessione          | 60 s                  | Ripresa QMP, `token\n`, invio del token e `ready\n`                                       |
| QMP                                     | 5 s                   | Greeting, capabilities, comando e risposta; gli eventi intermedi non rinnovano il tempo   |

I limiti di dimensione restano separati: richiesta storage fino a 1024 byte
di JSON e 1024 di segreto, risposta client fino a 1 MiB, frame gateway fino a
512 KiB, riga QMP fino a 65536 byte. Un messaggio piccolo può comunque essere
troppo lento e viene rifiutato.

Ogni operazione imposta il timeout del socket al tempo residuo della propria
scadenza. Interruzioni e timeout anticipati del kernel vengono ritentati solo
entro quella scadenza. Una nuova operazione costruisce un nuovo budget;
non eredita il timeout residuo lasciato sul socket. Anche le righe QMP già
nel buffer vengono controllate, così una sequenza di eventi non evita il limite.

## Esito di un timeout

Una richiesta storage incompleta viene rifiutata prima di avviare il worker;
il controller resta disponibile. Una risposta non ricevuta dal client non
equivale però a un'operazione annullata: il manager deve verificare lo stato,
come per gli altri errori del controller.

Un errore del canale gateway termina il proprietario host e distrugge la VM
gateway. Una revoca senza conferma usa il percorso esistente di distruzione
prima della chiusura storage. Il fallimento del token lascia visibile il
recupero necessario; nessun timeout dichiara automaticamente chiuso un LUKS.

I tempi iniziano dopo il collegamento del socket. Non sono un limite globale
per `cryptsetup`, processi figli, filesystem, avvio systemd o l'intera procedura
di chiusura. Non cambiano i timeout delle richieste HTTP al provider o gli
handshake interni al broker nella VM gateway. I limiti del supervisore virtio
rimangono descritti nel [protocollo del gateway](GATEWAY-CONTROL.md).

Il controllo del pidfd resta nel ciclo del proprietario: una richiesta in
corso può ritardare la revoca fino al proprio limite. Non si promette revoca
istantanea, né una scadenza fisica esatta in presenza di sospensione del processo
o dell'host.

## Prove riproducibili sul Mac

`cargo test --locked` da `packages/oblivion-host` usa veri socket Unix anche
su macOS. Copre scambio frammentato bidirezionale, avanzamento continuo senza
rinnovo, lettura parziale, peer che non consuma una scrittura, scadenza già
esaurita, EOF e nuovo budget per lo scambio successivo.

Il laboratorio Linux aggiunge due prove sul controller reale con UID 1000:
JSON incompleto e segreto incompleto, inviando un byte al secondo. Entrambe
devono essere rifiutate; lo stesso servizio deve poi elaborare `status`.
La suite delle sessioni invia invece un frame incompleto al proprietario
gateway: il timeout deve provocare la fine del servizio e della sua VM,
lasciando esplicito lo stato dei volumi.

Il [report delle verifiche](channel-deadlines-evidence.json) registra build,
hash e regressioni. Le prove usano segreti sintetici, Mac/HVF e guest Linux
ARM/TCG. Non includono un peer QMP che generi continuamente eventi o un guest
che frammenti deliberatamente il token handshake: questi percorsi condividono
la primitiva verificata e sono esercitati nel normale ciclo delle sessioni.
KVM/x86, provider reale, power loss e revisione indipendente restano gate aperti.
