# Controllo privato del gateway

Il binario Linux `oblivion-gateway-control` avvia e revoca il broker API nella
VM gateway. Sostituisce il launcher Python del precedente laboratorio. Il
[servizio host](HOST-GATEWAY.md) ora lo collega al generale LUKS e gestisce la
VM gateway. Il [collegamento della sessione](SESSION-NETWORK.md) ora assegna
un token locale e collega la chat al broker. Il protocollo ora include
importazione, attivazione e checkpoint dello [stato Tor](TOR-STATE.md).

## Confini

```text
Host fidato                         VM gateway                         VM chat
socket Unix privato → virtio-serial → supervisore root
                                     │ FD 3: credenziali e revoca
                                     └ broker UID 1300 ← HTTP/token ← agente
                                          └ SOCKS loopback → Tor
```

La porta è `/dev/virtio-ports/oblivion.gateway`, creata da udev come dispositivo
root 0600. Non è una NIC, un filesystem condiviso o una console. La VM chat
non riceve questa porta. L'host deve proteggere il socket QEMU e collegarsi
prima dell'apertura della porta nel guest; il laboratorio lo fa prima del boot
Linux. Il servizio non tenta di riutilizzare una connessione persa.

La chiave passa solo su questo canale e poi sul socket Unix ereditato dal
broker; non viene inserita in argv, ambiente, file del guest o comandi della
fixture. I buffer espliciti dei segreti usano `Zeroizing`; non si promette la
cancellazione di tutte le copie in RAM. L'host e il gateway restano fidati.

Il supervisore gira come root con `CAP_SETUID`, `CAP_SETGID`, `CAP_KILL`,
`CAP_DAC_OVERRIDE` e `CAP_CHOWN`. Le ultime due consentono lettura del cookie
e importazione dello stato nella directory privata di Tor, proprietario UID 1200.
Il figlio cambia UID/GID in 1300, elimina i gruppi supplementari e riceve
ambiente vuoto, stdio nullo, core disabilitati e `NoNewPrivileges`. Le sue
capability effettive, permesse, ereditabili e ambient sono vuote. Il bounding
set ereditato non è vuoto; non gli conferisce quelle capability. Il supervisore
non include il runtime HTTP/TLS del broker.

## Protocollo versione 1

Ogni messaggio è JSON UTF-8 preceduto dalla lunghezza a 4 byte unsigned big
endian. Sono ammessi 1–524288 byte; lo stato Tor selettivo è limitato a 128 KiB. Dal primo byte di una richiesta ci sono
5 secondi per completare header e payload; le risposte hanno lo stesso limite
temporale. L'attesa di un nuovo comando controlla segnali e processo figlio
ogni 100 ms. Il chiamante serializza le richieste; non ci sono ID o pipelining
nel client di laboratorio.

Il primo messaggio è `{"ready":true,"version":1}`. Le risposte ordinarie sono
`{"ok":true,"configured":false,"active":false}`, con i booleani aggiornati.

| Operazione       | Campi aggiuntivi              | Effetto                                                                               |
| ---------------- | ----------------------------- | ------------------------------------------------------------------------------------- |
| `configure`      | `api_key`                     | Una sola configurazione per VM; 16–4096 byte ASCII stampabili senza spazi             |
| `grant`          | `token`                       | 64 caratteri esadecimali; avvia un solo broker, dopo configurazione                   |
| `revoke`         | Nessuno                       | Chiude il broker e ne attende l'uscita; ripetibile quando inattivo                    |
| `status`         | Nessuno                       | Restituisce solo configurazione presente e autorizzazione attiva                      |
| `shutdown`       | Nessuno                       | Revoca il broker e termina il supervisore                                             |
| `tor-initialize` | `state` stringa oppure `null` | Una sola importazione, dopo configure e senza broker attivo; verifica la versione Tor |
| `tor-enable`     | Nessuno                       | Abilita la rete solo dopo inizializzazione                                            |
| `tor-snapshot`   | `stop` booleano               | Restituisce stato selettivo e bootstrap; se stop, revoca il broker e ferma prima Tor  |

Non sono accettati percorsi, URL, indirizzi SOCKS, comandi o campi sconosciuti.
Il listener è fisso su `10.88.0.1:8111` e SOCKS su `127.0.0.1:9050`. Una nuova
autorizzazione dopo `revoke` crea un nuovo processo; il vecchio token non viene
trasferito. Un secondo `grant` mentre il primo è attivo è un errore fatale.

## Arresto e guasti

La revoca chiude il canale del broker. Dopo al massimo 5 secondi senza uscita,
il supervisore lo termina forzatamente e lo attende. La conferma arriva solo
dopo la fine del processo, quindi dopo la chiusura dei suoi listener e socket.

Input non valido, scadenza, perdita del canale, SIGTERM/SIGINT o morte inattesa
del broker revocano l'accesso e fanno terminare il servizio. Se il canale è
ancora disponibile, l'unica risposta di errore è
`{"ok":false,"error":"control_failed"}`. Non contiene richieste o errori
del provider. `PR_SET_PDEATHSIG` e il cgroup systemd coprono anche la morte
improvvisa del supervisore. Il servizio non viene riavviato automaticamente.

`ExecStopPost` termina Tor dopo l'uscita del supervisore. La risposta a
`shutdown` certifica l'arresto del broker, **non** l'arresto già completato di
Tor o della VM: il servizio host attende e verifica anche quel lifecycle.
La VM rimane accesa finché l'host non la ferma. Il comando `revoke` da solo
lascia attivi supervisore e Tor, per consentire una nuova autorizzazione.

Tor parte disabilitato. Il servizio host importa lo stato, acquisisce il
checkpoint iniziale e invia `tor-enable` solo con `gateway_network: true`.
`tor-snapshot` risponde con `{ok, state, bootstrapped}`; il servizio host valida
nuovamente lo stato al proprio confine e lo salva nel generale scrivibile.
`stop: true` attende l’arresto ordinato di Tor e legge il suo ultimo stato.
Questo checkpoint precede la distruzione della VM; un crash conserva invece
l’ultimo checkpoint riuscito. La risposta `shutdown` da sola non salva lo stato.

## Laboratorio e limiti

Le prove usano due VM Debian ARM indipendenti su Mac/HVF, chiavi sintetiche,
immagine readonly e cattura pacchetti fuori dai guest. Le varianti
`--control-fault` coprono scollegamento, shutdown, campi sconosciuti, frame
eccessivo/incompleto, doppia autorizzazione e crash del supervisore o broker.
Tor viene riavviato esplicitamente con rete disabilitata prima di questi
guasti: la prova deve osservarne l'arresto, non trovare un processo già morto.

Passano otto esecuzioni offline, ognuna con 12 controlli, e un'esecuzione con
Tor reale con 13 controlli. Il caso di frame incompleto richiede 16,45 secondi;
gli altri offline fra 11,38 e 12,11 secondi. La prova Tor passa in 16,56 secondi.
Tutte usano la stessa immagine finale senza overlay. Un precedente tentativo
HTTPS è fallito dopo il bootstrap Tor: rimane nel report con causa non
identificata. Il runner ora attende la diagnostica della fixture prima di
terminare QEMU in caso di errore.

Vedere [riproduzione e confini della rete](NETWORK-LAB.md) e
[risultati del controllo privato](gateway-control-evidence.json). Le prove successive di
[Tor nel generale LUKS](TOR-STATE.md) coprono importazione e checkpoint.
Streaming dal vero provider e isolamento QEMU nell’host Linux di produzione
rimangono gate aperti.

La prima prova ha trovato `setuid(1300) = EPERM`: l'impostazione esplicita
`User=root`, insieme a seccomp/NoNewPrivileges, aveva rimosso `CAP_SETUID` prima
dell'exec. L'unità ora mantiene l'identità root predefinita senza capability
ambient. La scelta è verificata sui processi reali ed è coerente con il
[percorso di avvio systemd v257.9](https://github.com/systemd/systemd/blob/v257.9/src/core/exec-invoke.c#L5338-L5375).
