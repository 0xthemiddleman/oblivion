# Controllo privato del gateway

Il binario Linux `oblivion-gateway-control` avvia e revoca il broker API nella
VM gateway. Sostituisce il launcher Python del precedente laboratorio. Il
controller LUKS non lo usa ancora: la consegna della credenziale dal generale,
il lifecycle della VM gateway e l'adapter OpenCode sono il prossimo collegamento.

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

Il supervisore gira come root con solo `CAP_SETUID`, `CAP_SETGID` e `CAP_KILL`.
Il figlio cambia UID/GID in 1300, elimina i gruppi supplementari e riceve
ambiente vuoto, stdio nullo, core disabilitati e `NoNewPrivileges`. Le sue
capability effettive, permesse, ereditabili e ambient sono vuote. Il bounding
set ereditato non è vuoto; non gli conferisce quelle capability. Il supervisore
non include il runtime HTTP/TLS del broker.

## Protocollo versione 1

Ogni messaggio è JSON UTF-8 preceduto dalla lunghezza a 4 byte unsigned big
endian. Sono ammessi 1–16384 byte. Dal primo byte di una richiesta ci sono
5 secondi per completare header e payload; le risposte hanno lo stesso limite
temporale. L'attesa di un nuovo comando controlla segnali e processo figlio
ogni 100 ms. Il chiamante serializza le richieste; non ci sono ID o pipelining
nel client di laboratorio.

Il primo messaggio è `{"ready":true,"version":1}`. Le risposte ordinarie sono
`{"ok":true,"configured":false,"active":false}`, con i booleani aggiornati.

| Operazione  | Campi aggiuntivi | Effetto                                                                   |
| ----------- | ---------------- | ------------------------------------------------------------------------- |
| `configure` | `api_key`        | Una sola configurazione per VM; 16–4096 byte ASCII stampabili senza spazi |
| `grant`     | `token`          | 64 caratteri esadecimali; avvia un solo broker, dopo configurazione       |
| `revoke`    | Nessuno          | Chiude il broker e ne attende l'uscita; ripetibile quando inattivo        |
| `status`    | Nessuno          | Restituisce solo configurazione presente e autorizzazione attiva          |
| `shutdown`  | Nessuno          | Revoca il broker e termina il supervisore                                 |

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
Tor o della VM: il futuro host deve attendere e verificare anche quel lifecycle.
La VM rimane accesa finché l'host non la ferma. Il comando `revoke` da solo
lascia attivi supervisore e Tor, per consentire una nuova autorizzazione.

Tor resta disabilitato per default. Nessuna operazione di questo protocollo
abilita Internet. Prima di aggiungerla occorre collegare lo stato persistente
delle guardie al generale cifrato e definire il checkpoint all'arresto.

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
[risultati del controllo privato](gateway-control-evidence.json). Il laboratorio
non dimostra ancora integrazione con LUKS, blocco globale, streaming dal vero
provider o isolamento QEMU nell'host Linux di produzione.

La prima prova ha trovato `setuid(1300) = EPERM`: l'impostazione esplicita
`User=root`, insieme a seccomp/NoNewPrivileges, aveva rimosso `CAP_SETUID` prima
dell'exec. L'unità ora mantiene l'identità root predefinita senza capability
ambient. La scelta è verificata sui processi reali ed è coerente con il
[percorso di avvio systemd v257.9](https://github.com/systemd/systemd/blob/v257.9/src/core/exec-invoke.c#L5338-L5375).
