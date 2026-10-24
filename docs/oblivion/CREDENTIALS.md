# Credenziale Abliteration nel generale cifrato

Il manager fidato permette di salvare, sostituire e rimuovere la API key
Abliteration dopo lo sblocco del volume generale, con tutte le sessioni chiuse.
Il [collegamento dal controller LUKS](HOST-GATEWAY.md) al supervisore privato
è implementato: con il gateway configurato, salvare la credenziale avvia una
VM dedicata e gliela consegna. Tor resta disabilitato e non viene ancora
autorizzato un broker per la chat. Adapter e token della sessione sono il passo
successivo.

## Comportamento

- **A — Imposta:** campo nascosto nella TUI fidata; Esc annulla. Sono accettati
  da 16 a 1024 byte ASCII stampabili senza spazi, compatibili con un bearer token.
- **X — Rimuovi:** richiede una seconda X di conferma; qualunque altro tasto
  conserva la credenziale. Non revoca la chiave presso il provider.
- La UI mostra soltanto configurata/non configurata. Quando il generale è
  chiuso, lo stato IPC non espone neppure questo booleano (`null`).
- Sessione aperta, VM ancora attiva, recupero richiesto e profilo in sola lettura
  impediscono gli aggiornamenti anche tramite il client IPC, indipendentemente
  da ciò che mostra la TUI.

Il manager passa la credenziale sul canale locale autenticato già usato per le
password. L’operazione `set-api-key` ammette il solo segreto; `clear-api-key`
non ne accetta uno. Nessun percorso, comando o contenuto di file viene scelto
dal chiamante. Non esiste un’operazione per rileggere la chiave dal manager.
Il file salvato è letto solo dai servizi fidati del controller. Il proprietario
del canale lo consegna al supervisore della VM gateway, mai alla chat. Un
aggiornamento ferma il vecchio gateway prima di modificare il file; il blocco
globale verifica la fine della VM prima di chiudere il generale.

## Persistenza

Il file è `profile/abliteration.key` dentro il generale LUKS2 montato dal
controller, con proprietario root e permessi 0600; la directory del profilo è 0700. Non viene montato nelle VM. La cifratura è quella del volume: durante lo
sblocco il controller vede il file in chiaro, mentre i blocchi persistenti sul
supporto rimangono cifrati. I campi in memoria sono contenitori `Zeroizing`,
senza promessa di cancellazione di ogni copia interna alla RAM.

Un aggiornamento scrive `abliteration.key.pending` nello stesso volume,
esegue `fsync`, rinomina sul file definitivo e sincronizza la directory. Una
scrittura fallita prima della rinomina conserva la chiave precedente. Un
successivo tentativo può eliminare il file di staging privato lasciato dal
fallimento. Symlink, hardlink e proprietari/permessi diversi da quelli previsti
vengono rifiutati. Il lock globale serializza queste operazioni con il lifecycle
dei volumi.

La rimozione cancella il file attivo e lo staging noto e sincronizza la
directory. Non garantisce cancellazione fisica di vecchi blocchi della flash;
le copie precedenti erano già dentro il volume cifrato. Non viene cambiata
la password del generale né quella di alcuna sessione.

## Verifica sul Mac

Il test usa il vero manager non privilegiato sotto systemd, l’IPC del controller
Linux e un volume LUKS su disco sintetico, in Debian ARM/HVF. La sessione interna
usa TCG. La chiave è un marcatore di prova, mai una credenziale reale.

```sh
docker build --platform linux/arm64 --target host-artifacts \
  --output type=local,dest=artifacts/oblivion-credentials/bundle \
  -f script/oblivion/Dockerfile.vm-lab .
python3 script/oblivion/run-vm-lab.py \
  --image artifacts/oblivion-tui/image \
  --services-bundle artifacts/oblivion-credentials/bundle \
  --output artifacts/oblivion-credentials/run-new
```

Le prove coprono input nascosto, annullamento, validazione, permessi, alias del
file di staging, sostituzione atomica con spazio esaurito, blocco e riapertura,
rimozione confermata, sessione attiva, recupero e profilo readonly. Il runner
cerca entrambi i marcatori della chiave sull’intero disco esterno; la fixture
controlla che non compaiano nell’output della TTY e che UID 1000 non legga il file.
La suite passa 19 controlli in 189,58 secondi. Il risultato non sostituisce
tracing completo del filesystem, prova di power loss o integrazione con il
provider. Vedere le [evidenze](credentials-evidence.json).
