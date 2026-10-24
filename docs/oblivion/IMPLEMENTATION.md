# Stato di realizzazione

Obiettivo attivo: completare la roadmap di [PLAN.md](../../PLAN.md), senza ridurre
le garanzie ai soli test già disponibili. VirtualBox resta fuori dal lavoro
immediato. Il prototipo di laboratorio non è una release da usare con dati privati.
Lo sviluppo procede sul Mac ARM disponibile: Docker per build e prove del binario,
Debian/HVF e guest TCG per il laboratorio VM. L'hardware x86/KVM è un gate di
rilascio, non un blocco allo sviluppo sul Mac.

| Area                     | Stato ed evidenza                                                                                                                                                             | Gate ancora aperto                                                                    |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Audit iniziale v2        | Inventario, revisione e prove sintetiche in [fase 0](phase-0/AUDIT.md)                                                                                                        | TUI completa e Abliteration/Tor reali                                                 |
| Controller cifratura     | Rust + cryptsetup, due livelli; [creazione e recupero](STORAGE-RECOVERY.md), pulizia confermata e prove ENOSPC esterne/interne al generale                                    | I/O, power loss, perdita header, cambio password e disco pieno nel guest              |
| Privilegi                | Client UID limitato e socket con credenziali del peer; [unità systemd](HOST-SERVICES.md) con manager senza capability, dispositivi e rete separati                            | Integrazione nell'immagine live e revisione del controller privilegiato               |
| VM della sessione        | Controller systemd/QEMU, guest readonly e disco dedicato; ciclo A→B→A nel [laboratorio VM](VM-LAB.md); recupero dopo crash manager/controller e VM non responsiva             | KVM/hardware x86, OOM, shutdown completo e power loss                                 |
| Console e manager        | [Prototipo Rust](MANAGER.md): password nascoste, IPC, console filtrata, TUI interattiva e stato di recupero persistente fra riavvii dei servizi                               | Paste esteso/resize, guasti I/O e console fisica                                      |
| Fork privacy             | [Profilo compilato](PRIVACY-PROFILE.md): log/OTLP, dump, updater, discovery e config di progetto disabilitati; binario ARM provato con cattura pacchetti e nel guest          | Altri download/plugin, credenziale esterna e rifinitura della TUI                     |
| Rete e provider          | [Broker API](API-BROKER.md), [supervisore privato](GATEWAY-CONTROL.md), [gestione host](HOST-GATEWAY.md) e [due VM su Mac](NETWORK-LAB.md): firewall, Tor e revoca verificata | Streaming/provider reale, proxy e crash VM                                            |
| Credenziale API          | [Gestione nella TUI fidata](CREDENTIALS.md), file root 0600 nel generale, aggiornamento atomico e blocchi readonly/recupero/sessione aperta                                   | Provider reale e configurazione Wi-Fi                                                 |
| Documenti                | Flussi concordati                                                                                                                                                             | USB, copie A→B, OnionShare, quote e annullamento                                      |
| Effimero                 | Solo sblocco RO del profilo implementato                                                                                                                                      | Guest RAM completo e zero scritture end-to-end                                        |
| Live USB                 | Debian minimale scelta                                                                                                                                                        | Immagine x86, provisioning, boot fisico, firmware e periferiche                       |
| Integrità e manutenzione | Requisiti nel piano                                                                                                                                                           | Firma/catena boot, backup, upgrade/recovery, revisione indipendente, matrice hardware |

Il ciclo funzionale è ora provato anche con la TUI interattiva: aprire A
con la sua VM, far lavorare OpenCode su documenti e temporanei, chiudere tutto,
aprire B senza i dati di A, riaprire A e riprendere la stessa conversazione senza
i temporanei. Il provider rimane una fixture locale al guest. Mancano ancora
integrazione dei servizi nell'immagine live, guasti I/O, power loss e shutdown,
streaming del provider nel ciclo delle sessioni e collaudo del prodotto su KVM. Il laboratorio separato ha ora una prova Tor reale, senza richieste ad Abliteration. Il laboratorio usa una console a dimensioni fisse; la TUI è ancora un primo adattamento, con parti della terminologia e del comportamento degli agenti upstream da rifinire.

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
