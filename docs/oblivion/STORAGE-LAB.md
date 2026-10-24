# Laboratorio del controller LUKS2

24 ottobre 2026. Implementazione in [oblivion-host](../../packages/oblivion-host/README.md).
Il laboratorio usa una VM Debian ARM64 con HVF sul Mac di sviluppo. Verifica
storage e IPC reali; **non verifica l'isolamento delle future VM di sessione
x86/KVM**, la console fisica o la live USB.

L'ultima esecuzione verificata ha superato **17 controlli** in circa 36 secondi,
senza marcatori in chiaro sul disco esaminato. Il [riepilogo con hash](storage-evidence.json)
corrisponde ai sorgenti compilati presenti nel repository. `bun run check` ha
superato lint e 34 task di typecheck upstream; la compilazione Rust è verificata
separatamente nella build del laboratorio.

## Riproduzione

Prerequisiti del laboratorio attuale: macOS ARM64, Docker e QEMU con HVF.
Dal repository:

```sh
docker build --file script/oblivion/Dockerfile.storage-lab --target artifacts \
  --output type=local,dest=artifacts/oblivion-storage/image .
python3 script/oblivion/run-storage-lab.py \
  --image artifacts/oblivion-storage/image \
  --output artifacts/oblivion-storage/my-new-run
```

La directory di esecuzione deve essere nuova: il runner non sovrascrive prove
precedenti. Crea un disco virtuale vuoto da 1 GiB e avvia la VM senza rete,
cartelle condivise, clipboard, dispositivi fisici o credenziali dell'utente.
Il build Docker non è privilegiato e il suo contesto include soltanto i sorgenti
necessari. Le dipendenze si scaricano durante la build, non durante la prova.

L'init del laboratorio formatta esclusivamente quel disco virtuale. **Non va
eseguito su un host o avviato con dischi reali collegati.** La fixture usa password
pubbliche e documenti sintetici. Tutti gli artefatti locali restano sotto
`artifacts/`, escluso da Git.

## Evidenza raccolta

- Rifiuto di sovrascrittura e password errate senza apertura del generale.
- Due contenitori LUKS2 distinti, password separate e un solo mapping di sessione.
- La rimozione del link del dispositivo in `/dev/mapper` non nasconde un mapping
  ancora attivo: stato e interblocco leggono la tabella esposta dal kernel in sysfs.
- Dati differenti su A/B e ripristino dei dati di A dopo il cambio sessione.
- Rifiuto di chiusura e blocco globale quando il volume è occupato.
- SIGKILL del controller mentre cryptsetup è fermato: il figlio mantiene il lock;
  riprendendolo, una nuova istanza ricostruisce lo stato dal kernel.
- Profilo generale persistente, ID opachi e blocco ripetibile.
- Sblocco effimero del profilo: device read-only, mount senza journal replay,
  rifiuto di remount RW e hash dell'immagine invariato dopo il ciclo.
- Client con UID non privilegiato, controllo `SO_PEERCRED`, rifiuto di altri UID,
  operazioni arbitrarie, frame non validi e percorsi manipolati.
- L'apertura dal manager richiede la configurazione della VM: non espone un
  semplice sblocco del volume senza avviare l'ambiente isolato.
- Ricerca dei marcatori e delle password sintetiche sull'intero disco virtuale
  spento, inclusi lo spazio libero e il journal esterno.

Per queste prove una **fixture fidata** monta temporaneamente i filesystem di A/B
nella VM di laboratorio. Il controller di prodotto non lo fa. Questo prova il
comportamento dello storage, non la separazione tra guest differenti.

Il runner conserva `serial.log`, `data.raw` e `summary.json`; l'immagine include
hash di kernel/initramfs, versioni Debian e hash dei sorgenti compilati. Una ricerca
di stringhe senza risultati è un controllo aggiuntivo, non una dimostrazione
universale di assenza di ogni rappresentazione del contenuto. Non misura dati
compressi, catture dell'host o attacchi alla RAM.

L'immagine base Docker è fissata per digest e Cargo usa il lockfile. I pacchetti
APT sono registrati nel manifest ma non ancora fissati a uno snapshot: questa
build di laboratorio non è dichiarata riproducibile bit per bit né firmata.

## Verifiche ancora aperte

Il ciclo VM e il passaggio del blocco sono ora provati separatamente nel
[laboratorio VM](VM-LAB.md), con guest sintetico e senza attestazione KVM.
Il [manager](MANAGER.md) ora fornisce l'interfaccia password e il
[recupero della creazione](STORAGE-RECOVERY.md) ha prove di interruzione, pulizia
confermata e spazio esaurito durante la preparazione dei volumi. Mancano boot del
prodotto, Tor, provider reale e trasferimenti. Restano da provare interruzione di
alimentazione, errori I/O, disco pieno nel guest, backup, integrità dei volumi e
comportamento su hardware x86. La modalità effimera provata qui riguarda il solo
profilo generale in sola lettura, non l'intera esperienza effimera.
