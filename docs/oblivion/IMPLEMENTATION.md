# Stato di realizzazione

Obiettivo attivo: completare la roadmap di [PLAN.md](../../PLAN.md), senza ridurre
le garanzie ai soli test già disponibili. VirtualBox resta fuori dal lavoro
immediato. Il prototipo di laboratorio non è una release da usare con dati privati.
Lo sviluppo procede sul Mac ARM disponibile: Docker per build e prove del binario,
Debian/HVF e guest TCG per il laboratorio VM. L'hardware x86/KVM è un gate di
rilascio, non un blocco allo sviluppo sul Mac.

L’[importazione dei documenti](FILE-COPY.md) è ora collegata al controller e
al manager: `Ctrl+^`, scelta della sorgente e del percorso relativo, manifest
e conferma esplicita, quindi file verificati in `/vault/imports/<id>/files`.
Una VM sorgente senza rete riceve il solo dispositivo readonly; il ricevitore
UID 1001 resta separato dalla chat e lavora nel suo volume. Nessun filesystem
è condiviso fra ambienti. Blocco e chiusura revocano il relay e rimuovono
VM sorgente, porta e mapping prima di chiudere LUKS.

Le [evidenze del commit `4f75dfc`](manager-import-evidence.json) registrano
nove controlli Linux in VM sul Mac (207,65 s): annullamento, copia e permessi,
riapertura, perdita della sorgente, destinatario congelato e chiusura forzata,
blocco ed effimero. La sorgente resta byte-identica e il percorso effimero
non modifica il generale; nessuno dei 14 marcatori cercati compare nel disco
scrivibile. I test usano ext4 virtuale e una shell UID 1000, non un provider
reale o USB fisica. Passano 35/38 test del motore su Mac/Linux ARM e x86,
34/35 test host su Mac/Linux ARM, Clippy e 34 task del controllo completo.

Le prove hanno corretto tre guasti: backend QMP ancora occupato dopo
`device_del`, rename impedita da due mount distinti e connect QMP senza
scadenza quando la VM è congelata. Il blocco non è dichiarato finché la
pulizia non riesce; la chiusura forzata resta una scelta esplicita.
La CI `2e5c2f4` completa tutti i 31 job. La successiva `9331b27` supera il
job x86 di importazione e 31 job complessivi; il ciclo host/rete è ancora in corso.
Il recupero nel manager aggiunge ora un elenco delle copie, ricevute private
persistenti e cancellazione esplicita dei soli tentativi non consegnati.
Le modifiche successive della chat non vengono ricalcolate né eliminate.
Restano export, guasti I/O,
FAT/exFAT, supporti fisici, A→B, OnionShare e immagine live.

La [riconciliazione verificata](import-recovery-evidence.json), commit `9d57eb6`,
porta il ciclo a 12 controlli in 239,63 secondi. La fixture termina davvero
il ricevitore con SIGKILL dopo 64 KiB e prima della fine dello stream; il
manager mostra il tentativo privato e lo elimina solo dopo conferma.
Una nuova apertura LUKS conserva ricevute, documenti modificati e la pulizia
confermata. Passano 43 test del motore sul Mac e 46 su entrambe le architetture
Linux, incluso il recupero dopo una ricevuta finale non letta. Nessuno dei
15 marcatori cercati compare nel disco scrivibile; la sorgente e il generale
nel percorso effimero restano invariati. Tutti i sorgenti runtime e le fixture
corrispondono all’immagine provata; due file di test sono stati rafforzati dopo
la build e verificati separatamente sulle tre piattaforme.

Verifica precedente di storage e rete: [40 controlli LUKS/manager e 31 del ciclo di rete](ram-credentials-evidence.json)
passati. Include la TUI effimera, quota RAM insufficiente, recupero dopo crash,
webfetch/shell reali, cattura esterna e due chat effimere con stream revocati.
Le nuove chiavi temporanee sono verificate anche con generale scrivibile:
restano in RAM, non possono diventare predefinite persistenti e scompaiono
al blocco o al recupero dopo crash. Passano 34 test host sul Mac e in Debian
ARM64, Clippy sul Mac e i 34 task del controllo completo del repository.
Il generale resta invariato nel percorso effimero. Non prova l'intera USB,
l'OOM fisico o il provider remoto reale.
La CI `65112f3` termina inoltre con **29 job passati**, 36 controlli storage
e 26 del ciclo di rete x86, senza i 18 marcatori cercati sul disco. La successiva
CI `51f203b` passa tutti i quattro job eseguiti, con 37 controlli storage e 29
di rete: quota RAM e stream effimeri sono verificati anche in CI. La CI
`b86547c` verifica poi le credenziali RAM con un client x86 ricompilato:
tutti i 29 job passano, con 40 controlli storage e 31 di rete, senza i 19
marcatori cercati sul disco. I report distinguono le coperture.

Le [chiavi multiple](CREDENTIALS.md) sono implementate nel manager fidato:
archivio versionato, migrazione, scelta per apertura e nessuna rotazione
automatica. Il [SOCKS5 globale](KEYS-AND-EGRESS.md) viene prima del proxy dei
tool: il [connettore](EGRESS-CONNECTOR.md) è compilato e verificato separatamente,
mentre l'integrazione con supervisore e firewall è ora compilata in Debian ARM64
dalla [CI del fork](linux-ci-evidence.json). Passano 19 test host e 15 test di
libreria gateway; i due casi dell'adapter esclusi richiedono prove VM separate.
La [matrice di rete](transport-vm-evidence.json) supera inoltre 12 casi SOCKS
e 9 casi Tor offline, con due VM nuove per caso e catture esterne.
Passano anche `bun run check` e il compilatore TypeScript ufficiale, 34 task
ciascuno. Questo supera il blocco delle build locali dovuto agli errori I/O di
Docker, senza riparare il disco del Mac. Configurazione host e selezione nel
manager sono compilate anche per Linux. La nuova
[suite LUKS/manager](host-lifecycle-ci-evidence.json) supera 28 controlli su
host x86/KVM con sessione TCG: profili, chiavi, readonly, disco pieno,
blocco dei cambi a chat aperta e recupero dopo crash. Nessun marcatore
sintetico di password, API key o credenziali proxy compare sul disco o nella
console. Passano tutti i 25 job CI, compresi gli avvii diretti della TUI ARM
e x86. I [tentativi precedenti](host-transport-ci-evidence.json) conservano
i fallimenti del laboratorio ARM annidato e la diagnosi. Questa suite host
non avvia un gateway: cambi effettivi Tor↔SOCKS, provider reale e KVM della
sessione rimangono gate distinti.
La nuova [suite SOCKS completa](host-socks-stream-ci-evidence.json) collega
ora profilo cifrato, client corrente, gateway e proxy/provider sintetico
esterno: **18 controlli passati**, inclusi cambio chiave, revoca dello stream,
crash, ripristino del profilo e stato Tor invariato. Tutti i 25 job della
relativa CI sono verdi. Non è una prova del provider remoto o della
transizione host Tor↔SOCKS. La [prova successiva della transizione](host-transport-transition-evidence.json)
supera 21 controlli: guardia salvata preservata, Tor offline che blocca
l'apertura prima del volume, ritorno al SOCKS salvato e nuovo token/stream.
Non trova nessuno dei 14 marcatori sintetici cercati sul disco, inclusi prompt
e risposta. I quattro job eseguiti passano; la matrice di rete invariata viene
esclusa. La conversazione reale via Tor rimane da verificare.

Il motore puro delle regole in `oblivion-transport::policy` aggiunge nove
test passati sul Mac e in Debian ARM/x86 a quelli host precedenti; passa
anche Clippy sul Mac. Valida
destinazioni, wildcard delimitate, porte e ambiti e applica precedenza
`deny > ask > allow`. La [descrizione tecnica](NETWORK-AND-FILES.md) distingue
questo nucleo dal proxy dei tool, dalle conferme e dalla persistenza ancora
da collegare. La [CI successiva](network-policy-ci-evidence.json) copre il
motore puro e ripete il ciclo SOCKS completo: tutti i 25 job passano.
Il connettore ha un canale tool con policy esplicita. Il proxy HTTP/CONNECT
supera otto test in Debian ARM64; passano tutti i 28 test di libreria gateway
e i controlli del repository. Le [evidenze](tools-proxy-component-evidence.json)
registrano anche i 21 scenari di rete passati e due fallimenti VM: comando ARM
con acceleratore errato, corretto in TCG; riapertura della console x86 dopo
crash del manager, da diagnosticare. La suite stream host non è partita.
Il servizio `oblivion-tools` e il suo avvio opzionale sotto il supervisore sono
compilati, con UID 1301 e ingresso 8112. La [prova successiva in VM ARM](tools-service-evidence.json)
supera 56 controlli: 28 test di libreria, 10 di attivazione, hardening dei binari
di produzione e due prove dell'adapter compilato. Tutti i 25 job passano:
anche 28 controlli storage x86 e 21 dello stream host completo, senza nessuno
dei 14 marcatori sintetici cercati sul disco. Il problema precedente della
console non è stato riprodotto né spiegato. I [quattro scenari del percorso tool completo](tools-vm-evidence.json)
passano con 13 controlli ciascuno, inclusi HTTP e CONNECT/TLS, firewall e chiusura
degli stream su revoca o crash. La CI termina con tutti i 29 job verdi.
Il manager ora modifica regole globali o per sessione, salvate nel generale
o temporanee in RAM fino al blocco; il controller le include nel grant privato.
Passano 13 test delle regole sul Mac e in Debian ARM/x86, oltre a Clippy sul Mac.
La [prova delle regole host](host-policy-evidence.json) supera 33 controlli
storage/manager e 23 del ciclo completo: menu, persistenza, ENOSPC, readonly,
attivazione del proxy tool e rifiuti dal guest. Nessuno dei 14 marcatori cercati
compare sul disco del ciclo host. Tutti i 29 job CI sono verdi, inclusi 56
controlli del gateway ARM.
Senza profilo il servizio tool resta spento. `ask` blocca; conferme interattive,
scadenze e revoca dei singoli permessi restano da collegare. Le variabili proxy
sono ora nel launcher del guest, con il solo token locale. Due test nativi
della fixture passano; la nuova prova del client compilato, di `webfetch`,
redirect, shell e regole distinte A/B è stata eseguita nelle VM, ma il
[primo tentativo](compiled-tools-attempt.json) fallisce nel subprocess degli
strumenti con exit 1 e stderr vuoto. Il vecchio assert non conservava stdout,
dove il client JSON pubblica gli errori; quel tentativo non permette di
diagnosticarne la causa.
Sono aggiunti diagnostici limitati ai dati sintetici del laboratorio, senza
log nel prodotto. I 33 controlli storage/manager passano ancora, ma la sequenza
stream si ferma dopo sette controlli: né Bun/proxy né i successivi casi A/B
sono validati da questo tentativo.
Il [report precedente](host-transport-status.json)
rimane lo snapshot della sola verifica nativa.

La [ripetizione con diagnostici](compiled-tools-diagnosis.json) individua
l'assert errato della fixture sui messaggi dei tool bloccati: OpenCode produce
`Transport error (GET ...)`. Il nuovo test controlla anche gli stati JSON dei
cinque `webfetch`; il collaudo completo sul Mac supera ora 29 controlli,
compresi A/B e stream effimeri. La cattura esterna della prima A verifica
436 pacchetti TCP sul solo SOCKS configurato, senza UDP o IPv6.
Dopo che l'utente ha liberato spazio, Docker sul Mac completa la build Linux
del controller e i 32 test host. Ripristinate le dipendenze dal lockfile e Bun
1.4.4, passano anche `bun run check` e `bun run typecheck:tsc`, 34 task ciascuno.
Le prove LUKS/VM sono nuovamente eseguibili localmente.

Vincolo UI confermato: mantenere grafica, componenti e interazioni della chat
OpenCode, con rimozioni mirate e branding leggero. Nessun redesign della chat;
le nuove schermate amministrative rimangono nel manager host separato.

Il [percorso effimero](EPHEMERAL.md) collega ora E prima dello sblocco, generale
readonly e nuova VM con disco LUKS2 solo in RAM. Il segreto viene generato per
apertura e non salvato; la chiusura rimuove il file dopo VM e mapping. Passano
32 test host nativi e Clippy sul Mac. La [suite servizi aggiornata](ephemeral-evidence.json)
supera **37 controlli Linux** sul Mac: TUI reale, scritture, seconda chat pulita,
recupero dopo crash e quota tmpfs insufficiente senza fallback. Nessuno dei
12 marcatori cercati compare sul disco. Passa anche la prova effimera con
gateway attivo e SOCKS/provider sintetico: due token distinti, vecchio token
rifiutato, stream chiusi e dischi RAM rimossi, generale invariato. Le
[credenziali temporanee](ram-credentials-evidence.json) sono ora verificate
separatamente; Tor reale, OOM fisico e verifica dell'intera USB restano da completare.

| Area                     | Stato ed evidenza                                                                                                                                                             | Gate ancora aperto                                                                    |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Audit iniziale v2        | Inventario, revisione e prove sintetiche in [fase 0](phase-0/AUDIT.md)                                                                                                        | TUI completa e Abliteration/Tor reali                                                 |
| Controller cifratura     | Rust + cryptsetup, due livelli; [creazione e recupero](STORAGE-RECOVERY.md), pulizia confermata e prove ENOSPC esterne/interne al generale                                    | I/O, power loss, perdita header, cambio password e disco pieno nel guest              |
| Privilegi                | Client UID limitato e socket con credenziali del peer; [unità systemd](HOST-SERVICES.md) con manager senza capability, dispositivi e rete separati                            | Integrazione nell'immagine live e revisione del controller privilegiato               |
| VM della sessione        | Controller systemd/QEMU, guest readonly e disco dedicato; ciclo A→B→A nel [laboratorio VM](VM-LAB.md); recupero dopo crash manager/controller e VM non responsiva             | KVM/hardware x86, OOM, shutdown completo e power loss                                 |
| Console e manager        | [Prototipo Rust](MANAGER.md): password nascoste, IPC, console filtrata, TUI interattiva e stato di recupero persistente fra riavvii dei servizi                               | Paste esteso/resize, guasti I/O e console fisica                                      |
| Fork privacy             | [Profilo compilato](PRIVACY-PROFILE.md): log/OTLP, dump, updater, discovery e config di progetto disabilitati; binario ARM provato con cattura pacchetti e nel guest          | Altri download/plugin, immagini aggiornate e rifinitura della TUI                     |
| Rete e provider          | [Broker API](API-BROKER.md), [supervisore privato](GATEWAY-CONTROL.md), [gestione host](HOST-GATEWAY.md) e [due VM su Mac](NETWORK-LAB.md): firewall, Tor e revoca verificata | SOCKS5 globale, provider reale, proxy dei tool e shutdown                             |
| Credenziali API          | [Chiavi nominate nella TUI fidata](CREDENTIALS.md), archivio root 0600 nel generale, migrazione atomica, selezione in RAM e blocchi readonly/recupero/sessione aperta         | Provider reale, power loss durante migrazione e Wi-Fi                                 |
| Documenti                | [Import USB e riconciliazione nel manager](FILE-COPY.md)                                                                                                                                                             | Export, USB fisica, I/O, FAT/exFAT, copie A→B e OnionShare                                      |
| Effimero                 | [Manager e VM con disco RAM](EPHEMERAL.md): TUI, seconda chat, gateway, crash e quota insufficiente verificati in Linux                                                      | Tor reale, OOM fisico, credenziali temporanee e zero scritture sull'intera USB          |
| Live USB                 | Debian minimale scelta                                                                                                                                                        | Immagine x86, provisioning, boot fisico, firmware e periferiche                       |
| Integrità e manutenzione | Requisiti nel piano                                                                                                                                                           | Firma/catena boot, backup, upgrade/recovery, revisione indipendente, matrice hardware |

Il ciclo funzionale è ora provato anche con la TUI interattiva: aprire A
con la sua VM, far lavorare OpenCode su documenti e temporanei, chiudere tutto,
aprire B senza i dati di A, riaprire A e riprendere la stessa conversazione senza
i temporanei. Il provider rimane una fixture locale al guest. Mancano ancora
integrazione dei servizi nell'immagine live, guasti I/O, power loss e shutdown,
provider remoto reale e collaudo del prodotto su KVM. Il laboratorio separato ha ora una prova Tor reale, senza richieste ad Abliteration. Il laboratorio usa una console a dimensioni fisse; restano da rifinire testi, comandi residui e integrazione con il manager, preservando grafica e componenti della chat upstream.

Il controller LUKS ora avvia il gateway dal profilo salvato, con UID host
1101 separato dalla chat, e ne verifica la fine prima del blocco. Le
[evidenze della gestione host](host-gateway-evidence.json) coprono credenziali,
crash e riuso readonly del profilo. La [rete della sessione](SESSION-NETWORK.md)
collega NIC, token locale e revoca al ciclo del volume. Lo [stato Tor](TOR-STATE.md)
viene importato prima dell’attivazione online, salvato selettivamente nel
generale scrivibile e acquisito anche alla chiusura ordinata. Il bootstrap
e la presenza delle guardie precedono nuove autorizzazioni online. Il collaudo
del provider reale resta aperto. La precedente verifica di lint e dei 34 task di
typecheck è passata nell’ambiente Debian/Bun fissato dal repository. Per le
modifiche correnti al trasporto, il comando canonico locale non era partito
per l'errore I/O Docker. La successiva [CI Linux](linux-ci-evidence.json)
lo ha eseguito con successo sul commit indicato nel report.

Il grant ora nasce soltanto dopo l'avvio di QEMU sospeso e resta associato al
suo pidfd. La [revoca all'uscita del processo](session-exit-evidence.json)
copre SIGKILL e uscita QMP senza chiusura preventiva del controller. Il volume
non viene dichiarato chiuso dall'osservatore; rimangono le verifiche storage
prima di una nuova apertura.

I [canali privati host](CHANNEL-DEADLINES.md) ora usano scadenze complessive
per frame e scambi, inclusi token e QMP. Le richieste storage incomplete vengono
rifiutate; un errore sul canale gateway provoca la distruzione della sua VM.
Le prove e i limiti della copertura sono nel [report dedicato](channel-deadlines-evidence.json).

L'[adapter compilato](ADAPTER-BROKER.md) completa ora streaming, strumento e
ripresa attraverso il broker reale con SOCKS/TLS sintetici. La revoca durante
lo stream chiude la connessione del provider e termina il client con errore;
il profilo Oblivion disabilita i retry automatici di `ModelRetry`. Questa prova
gira nella singola VM di laboratorio. La successiva [suite del ciclo VM](STREAM-LIFECYCLE.md)
verifica ora la revoca di stream attivi attraverso controller LUKS, supervisore
e due VM separate: chiusura prima del volume e arresto improvviso di QEMU.
Il provider remoto reale e lo shutdown globale rimangono gate distinti.
