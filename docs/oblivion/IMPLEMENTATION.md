# Stato di realizzazione

Obiettivo attivo: completare la roadmap di [PLAN.md](../../PLAN.md), senza ridurre
le garanzie ai soli test già disponibili. VirtualBox resta fuori dal lavoro
immediato. Il prototipo di laboratorio non è una release da usare con dati privati.

| Area | Stato ed evidenza | Gate ancora aperto |
| --- | --- | --- |
| Audit iniziale v2 | Inventario, revisione e prove sintetiche in [fase 0](phase-0/AUDIT.md) | TUI completa e Abliteration/Tor reali |
| Controller cifratura | Rust + cryptsetup, due livelli, interblocco, blocco e profilo RO in [laboratorio](STORAGE-LAB.md) | Recovery creazione, power loss, quota/esaurimento disco |
| Privilegi | Client UID limitato e socket con credenziali del peer; nessun accesso diretto al generale | Servizi di sistema e confinamento nel prodotto |
| VM della sessione | Architettura definita; nessuna VM di prodotto ancora avviata | Immagine guest, KVM, handoff del blocco, processi, arresto e riapertura |
| Console e manager | [Specifica approvata](BOOT-AND-TUI.md) | Rendering reale senza desktop, input fidato, segreti e ciclo VM |
| Fork privacy | Audit dei punti da modificare | Disabilitazione runtime di log/discovery/download e TUI ridotta |
| Rete e provider | [Progetto](NETWORK-AND-FILES.md) e catture di fixture | Broker, Tor, policy, revoca, test di guasto e provider reale |
| Documenti | Flussi concordati | USB, copie A→B, OnionShare, quote e annullamento |
| Effimero | Solo sblocco RO del profilo implementato | Guest RAM completo e zero scritture end-to-end |
| Live USB | Debian minimale scelta | Immagine x86, provisioning, boot fisico, firmware e periferiche |
| Integrità e manutenzione | Requisiti nel piano | Firma/catena boot, backup, upgrade/recovery, revisione indipendente, matrice hardware |

La prima consegna funzionale completa rimane: aprire A con la sua VM, far lavorare
OpenCode su documenti e temporanei, chiudere tutto, aprire B senza accesso ad A,
riaprire A recuperando solo i dati persistenti. Le prove storage sono un componente
di questa consegna, non la sua sostituzione.
