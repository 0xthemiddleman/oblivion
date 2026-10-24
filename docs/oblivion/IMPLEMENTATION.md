# Stato di realizzazione

Obiettivo attivo: completare la roadmap di [PLAN.md](../../PLAN.md), senza ridurre
le garanzie ai soli test già disponibili. VirtualBox resta fuori dal lavoro
immediato. Il prototipo di laboratorio non è una release da usare con dati privati.
Lo sviluppo procede sul Mac ARM disponibile: Docker per build e prove del binario,
Debian/HVF e guest TCG per il laboratorio VM. L'hardware x86/KVM è un gate di
rilascio, non un blocco allo sviluppo sul Mac.

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
console non è stato riprodotto né spiegato. Quattro scenari del percorso tool completo sono pronti ma non ancora
eseguiti. Il controller host non invia una policy: il servizio resta spento
nel normale ciclo. Policy del manager, conferme e client Bun restano da collegare.
Il [report precedente](host-transport-status.json)
rimane lo snapshot della sola verifica nativa.

Vincolo UI confermato: mantenere grafica, componenti e interazioni della chat
OpenCode, con rimozioni mirate e branding leggero. Nessun redesign della chat;
le nuove schermate amministrative rimangono nel manager host separato.

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
| Documenti                | Flussi concordati                                                                                                                                                             | USB, copie A→B, OnionShare, quote e annullamento                                      |
| Effimero                 | Solo sblocco RO del profilo implementato                                                                                                                                      | Guest RAM completo e zero scritture end-to-end                                        |
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
