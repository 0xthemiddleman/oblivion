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
| VM della sessione | Controller systemd/QEMU, guest readonly e disco dedicato; ciclo A→B→A nel [laboratorio VM](VM-LAB.md) con fixture sintetica | KVM/hardware x86, OpenCode reale, console fidata e servizi di prodotto |
| Console e manager | [Specifica approvata](BOOT-AND-TUI.md) | Rendering reale senza desktop, input fidato, segreti e ciclo VM |
| Fork privacy | [Profilo compilato](PRIVACY-PROFILE.md): log/OTLP, dump, updater, discovery e config di progetto disabilitati; binario ARM provato con cattura pacchetti | Guest reale, altri download/plugin, credenziale esterna e TUI ridotta |
| Rete e provider | [Progetto](NETWORK-AND-FILES.md) e catture di fixture | Broker, Tor, policy, revoca, test di guasto e provider reale |
| Documenti | Flussi concordati | USB, copie A→B, OnionShare, quote e annullamento |
| Effimero | Solo sblocco RO del profilo implementato | Guest RAM completo e zero scritture end-to-end |
| Live USB | Debian minimale scelta | Immagine x86, provisioning, boot fisico, firmware e periferiche |
| Integrità e manutenzione | Requisiti nel piano | Firma/catena boot, backup, upgrade/recovery, revisione indipendente, matrice hardware |

La prima consegna funzionale completa rimane: aprire A con la sua VM, far lavorare
OpenCode su documenti e temporanei, chiudere tutto, aprire B senza accesso ad A,
riaprire A recuperando solo i dati persistenti. Le prove storage sono un componente
di questa consegna. Il ciclo VM ora provato usa una fixture sintetica: non
sostituisce l'integrazione di OpenCode e il collaudo del prodotto su KVM.
