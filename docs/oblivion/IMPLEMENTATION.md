# Stato di realizzazione

Obiettivo attivo: completare la roadmap di [PLAN.md](../../PLAN.md), senza ridurre
le garanzie ai soli test già disponibili. VirtualBox resta fuori dal lavoro
immediato. Il prototipo di laboratorio non è una release da usare con dati privati.
Lo sviluppo procede sul Mac ARM disponibile: Docker per build e prove del binario,
Debian/HVF e guest TCG per il laboratorio VM. L'hardware x86/KVM è un gate di
rilascio, non un blocco allo sviluppo sul Mac.

Nuovi requisiti: [chiavi multiple e SOCKS5 globale](KEYS-AND-EGRESS.md), con
Tor predefinito e nessun fallback. La loro implementazione viene prima del
proxy dei tool: il codice attuale usa ancora una sola chiave e Tor online.

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
| Credenziale API          | [Gestione nella TUI fidata](CREDENTIALS.md), file root 0600 nel generale, aggiornamento atomico e blocchi readonly/recupero/sessione aperta                                   | Archivio chiavi multiple, selezione/migrazione, provider reale e Wi-Fi                |
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
del provider reale resta aperto. Lint e i 34 task di
typecheck passano nell’ambiente Debian/Bun fissato dal repository.

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
