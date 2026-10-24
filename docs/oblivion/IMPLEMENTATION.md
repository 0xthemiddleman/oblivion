# Stato di realizzazione

Obiettivo attivo: completare la roadmap di [PLAN.md](../../PLAN.md), senza ridurre
le garanzie ai soli test già disponibili. VirtualBox resta fuori dal lavoro
immediato. Il prototipo di laboratorio non è una release da usare con dati privati.
Lo sviluppo procede sul Mac ARM disponibile: Docker per build e prove del binario,
Debian/HVF e guest TCG per il laboratorio VM. L'hardware x86/KVM è un gate di
rilascio, non un blocco allo sviluppo sul Mac.

| Area | Stato ed evidenza | Gate ancora aperto |
| --- | --- | --- |
| Audit iniziale v2 | Inventario, revisione e prove sintetiche in [fase 0](phase-0/AUDIT.md) | TUI completa e Abliteration/Tor reali |
| Controller cifratura | Rust + cryptsetup, due livelli, interblocco, blocco e profilo RO in [laboratorio](STORAGE-LAB.md) | Recovery creazione, power loss, quota/esaurimento disco |
| Privilegi | Client UID limitato e socket con credenziali del peer; nessun accesso diretto al generale | Servizi di sistema e confinamento nel prodotto |
| VM della sessione | Controller systemd/QEMU, guest readonly e disco dedicato; ciclo A→B→A nel [laboratorio VM](VM-LAB.md) con OpenCode compilato, shell e provider sintetico interni al guest | KVM/hardware x86, crash e servizi di prodotto |
| Console e manager | [Prototipo Rust](MANAGER.md): password nascoste, IPC, console filtrata e ciclo A→B→A da TTY; 9 controlli del manager e 8 del ciclo con TUI interattiva passati | Paste esteso/resize, crash dei servizi e console fisica |
| Fork privacy | [Profilo compilato](PRIVACY-PROFILE.md): log/OTLP, dump, updater, discovery e config di progetto disabilitati; binario ARM provato con cattura pacchetti e nel guest | Altri download/plugin, credenziale esterna e rifinitura della TUI |
| Rete e provider | [Progetto](NETWORK-AND-FILES.md) e catture di fixture | Broker, Tor, policy, revoca, test di guasto e provider reale |
| Documenti | Flussi concordati | USB, copie A→B, OnionShare, quote e annullamento |
| Effimero | Solo sblocco RO del profilo implementato | Guest RAM completo e zero scritture end-to-end |
| Live USB | Debian minimale scelta | Immagine x86, provisioning, boot fisico, firmware e periferiche |
| Integrità e manutenzione | Requisiti nel piano | Firma/catena boot, backup, upgrade/recovery, revisione indipendente, matrice hardware |

Il ciclo funzionale è ora provato anche con la TUI interattiva: aprire A
con la sua VM, far lavorare OpenCode su documenti e temporanei, chiudere tutto,
aprire B senza i dati di A, riaprire A e riprendere la stessa conversazione senza
i temporanei. Il provider rimane una fixture locale al guest. Mancano ancora
integrazione dei servizi, gestione dei guasti, rete reale e collaudo del prodotto su KVM. Il laboratorio usa una console a dimensioni fisse; la TUI è ancora un primo adattamento, con parti della terminologia e del comportamento degli agenti upstream da rifinire.
