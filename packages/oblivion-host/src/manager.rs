use oblivion_host::control::{self, Result};
use oblivion_host::terminal::{self, ACTION, BRAND, CLEAR, FEEDBACK, LOGO, RESET, Terminal};
use oblivion_host::{egress, keys};
use oblivion_transport::{Authentication, Metadata, Transport};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    general_present: bool,
    general_incomplete: bool,
    general_open: bool,
    general_mounted: bool,
    general_readonly: bool,
    session_open: bool,
    sessions: Vec<String>,
    incomplete_sessions: Vec<String>,
    available_bytes: Option<u64>,
    session_bytes: u64,
    vm_state: Option<String>,
    laboratory_emulation: bool,
    recovery_required: bool,
    api_key_present: Option<bool>,
    api_keys: Option<keys::Status>,
    egress: Option<egress::Status>,
    gateway_state: Option<String>,
    gateway_vm_state: Option<String>,
    gateway_network: bool,
}

fn main() {
    let preview = std::env::args().nth(1).as_deref() == Some("--preview");
    if run(preview).is_err() {
        // A failed lock is not reported as success; a restarted manager queries
        // kernel-derived status again before enabling any new session operation.
        if !preview && cfg!(target_os = "linux") && unsafe { libc::geteuid() } != 0 {
            let _ = control::request("lock", None, &[]);
        }
        eprintln!("manager_stopped_check_controller_state");
        std::process::exit(1);
    }
}

fn run(preview: bool) -> Result<()> {
    control::protect()?;
    if !preview && (!cfg!(target_os = "linux") || unsafe { libc::geteuid() } == 0) {
        return Err("linux_nonroot_manager_required".into());
    }
    let mut terminal = Terminal::open()?;
    if preview {
        terminal.write(&format!("{CLEAR}{BRAND}{LOGO}{RESET}\r\n\r\n OBLIVION - ANTEPRIMA, nessun archivio collegato\r\n\r\n > Sessione 7c2a  Bloccata\r\n   Sessione 91f4  Bloccata\r\n\r\n {ACTION}Premi un tasto per uscire. Nessuna password richiesta.{RESET}"))?;
        terminal.input(-1)?;
        return Ok(());
    }
    let mut selected = 0usize;
    let mut notice = String::new();
    let mut requested_new = false;
    let mut incomplete = false;
    loop {
        let status: Status = serde_json::from_str(&control::request("status", None, &[])?)?;
        if status
            .sessions
            .iter()
            .chain(&status.incomplete_sessions)
            .any(|s| {
                s.len() != 32
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err("invalid_session_list".into());
        }
        let sessions = if incomplete {
            &status.incomplete_sessions
        } else {
            &status.sessions
        };
        selected = selected.min(sessions.len().saturating_sub(1));
        let (rows, cols) = terminal.size();
        let mut view = format!(
            "{CLEAR}{BRAND}{} OBLIVION - controllo fidato{RESET}\r\n\r\n",
            if cols >= 64 && rows >= 20 {
                format!("{LOGO}\r\n\r\n")
            } else {
                String::new()
            }
        );
        if status.laboratory_emulation {
            view.push_str(" LABORATORIO TCG - solo dati sintetici\r\n\r\n");
        }
        if status.recovery_required {
            view.push_str(" RECUPERO NECESSARIO - nuove aperture bloccate\r\n\r\n [B] Riprova chiusura e blocco\r\n [F] Arresto forzato e blocco\r\n");
        } else if !status.general_mounted {
            view.push_str(if status.general_open {
                " Archivio generale da richiudere.\r\n\r\n [B] Blocca\r\n"
            } else if !status.general_present && status.general_incomplete {
                " Preparazione generale interrotta.\r\n Nessun archivio pronto; il tentativo incompleto e' conservato.\r\n\r\n [D] Elimina il tentativo incompleto\r\n"
            } else if !status.general_present {
                " Prima preparazione di Oblivion\r\n\r\n [Invio] Crea il volume generale cifrato\r\n"
            } else {
                " Archivio generale bloccato\r\n\r\n [Invio] Sblocca\r\n"
            });
        } else if status.session_open
            || !matches!(
                status.vm_state.as_deref(),
                None | Some("inactive" | "failed")
            )
        {
            view.push_str(" SESSIONE ANCORA APERTA\r\n\r\n [Invio] Console   [C] Chiudi e blocca\r\n [F] Chiusura forzata\r\n");
        } else if status.general_readonly {
            view.push_str(" Profilo in sola lettura; guest effimero non ancora disponibile.\r\n [G] Scegli chiave API  [R] Rete  [B] Blocca\r\n");
        } else {
            view.push_str(if incomplete {
                " Creazioni interrotte - mai aperte come chat\r\n\r\n"
            } else {
                " Sessioni protette\r\n\r\n"
            });
            let limit = rows
                .saturating_sub(if rows >= 20 && cols >= 64 { 21 } else { 16 })
                .max(1) as usize;
            let start = selected.saturating_sub(limit - 1);
            for (index, session) in sessions.iter().enumerate().skip(start).take(limit) {
                view.push_str(&format!(
                    " {} Sessione {}  {}\r\n",
                    if index == selected { ">" } else { " " },
                    &session[..8],
                    if incomplete { "Incompleta" } else { "Bloccata" }
                ));
            }
            if sessions.is_empty() {
                view.push_str(if incomplete {
                    " Nessuna creazione interrotta.\r\n"
                } else {
                    " Nessuna sessione.\r\n"
                });
            }
            view.push_str(&format!(
                "\r\n Spazio disponibile: {} MiB; nuova sessione: {} MiB\r\n",
                status.available_bytes.unwrap_or(0) / 1048576,
                status.session_bytes / 1048576
            ));
            view.push_str(&format!(
                " API Abliteration: {}  [A] Imposta  [X] Rimuovi  [G] Chiavi\r\n",
                if status.api_key_present == Some(true) {
                    "configurata"
                } else {
                    "non configurata"
                }
            ));
            if let Some(keys) = &status.api_keys {
                let name = keys
                    .entries
                    .iter()
                    .find(|entry| Some(&entry.id) == keys.selected.as_ref())
                    .map(|entry| entry.name.as_str())
                    .unwrap_or("nessuna");
                view.push_str(&format!(" Chiave selezionata: {name}\r\n"));
            }
            if let Some(egress) = &status.egress {
                view.push_str(&format!(
                    " Rete: {}{}  [R] Configura\r\n",
                    transport_label(&egress.selected),
                    if egress.temporary {
                        " (temporanea)"
                    } else {
                        ""
                    }
                ));
            }
            if let Some(gateway) = &status.gateway_state {
                view.push_str(
                    if gateway == "active" && status.gateway_vm_state.as_deref() == Some("active") {
                        if status.gateway_network {
                            if status.egress.as_ref().is_some_and(|egress| {
                                matches!(egress.selected, Metadata::Socks5 { .. })
                            }) {
                                " Gateway avviato con SOCKS5.\r\n"
                            } else {
                                " Gateway avviato. Tor può richiedere alcuni istanti.\r\n"
                            }
                        } else {
                            " Gateway pronto; rete non abilitata.\r\n"
                        }
                    } else {
                        " Gateway non attivo.\r\n"
                    },
                );
            }
            view.push_str(&format!(
                " {ACTION}{}\r\n [I] Creazioni interrotte ({}) / sessioni  [B] Blocca{RESET}\r\n",
                if incomplete {
                    "[D] Elimina selezionata  [J/K] Seleziona"
                } else {
                    "[N] Nuova  [Invio] Apri  [J/K] Seleziona"
                },
                status.incomplete_sessions.len()
            ));
        }
        view.push_str(&format!(
            "\r\n {FEEDBACK}{notice}{RESET}\r\n [Q] Blocca tutto ed esci"
        ));
        terminal.write(&view)?;
        let input = if std::mem::take(&mut requested_new) {
            zeroize::Zeroizing::new(vec![b'n'])
        } else {
            terminal.input(-1)?
        };
        let key = if input.as_slice() == b"\x1b[A" {
            b'k'
        } else if input.as_slice() == b"\x1b[B" {
            b'j'
        } else if input.len() == 1 {
            input[0].to_ascii_lowercase()
        } else {
            continue;
        };
        notice.clear();
        if key == b'j' {
            selected = (selected + 1).min(sessions.len().saturating_sub(1));
            continue;
        }
        if key == b'k' {
            selected = selected.saturating_sub(1);
            continue;
        }
        let result: Result<()> = (|| {
            if matches!(key, b'b' | b'q') {
                busy(&mut terminal, "Blocco di Oblivion in corso")?;
                control::request("lock", None, &[])?;
                return Ok(());
            }
            if !status.recovery_required && !status.general_open && matches!(key, b'\r' | b'\n') {
                if !status.general_present {
                    if !status.general_incomplete {
                        if let Some(secret) = new_password(
                            &mut terminal,
                            "Nuovo volume generale - password di almeno 16 byte",
                            "Ripeti la password del volume generale",
                        )? {
                            busy(&mut terminal, "Preparazione del volume generale cifrato")?;
                            control::request("init-general", None, &secret)?;
                        }
                    }
                    return Ok(());
                }
                if let Some(secret) = terminal.password("Password del volume generale")? {
                    busy(&mut terminal, "Sblocco del volume generale")?;
                    control::request("unlock", None, &secret)?;
                }
                return Ok(());
            }
            if status.recovery_required || status.session_open {
                if !status.recovery_required && matches!(key, b'\r' | b'\n') {
                    requested_new = enter_console(&mut terminal)?;
                }
                if key == b'c' {
                    busy(&mut terminal, "Chiusura della sessione")?;
                    control::request("close", None, &[])?;
                }
                if key == b'f' {
                    terminal.flush_input();
                    terminal.write(&format!("{CLEAR}{FEEDBACK} Chiusura forzata: possibile perdita delle ultime modifiche.\r\n Premi F di nuovo per confermare; altro tasto annulla.{RESET}"))?;
                    if terminal.input(-1)?.as_slice().eq_ignore_ascii_case(b"f") {
                        busy(&mut terminal, "Chiusura forzata")?;
                        control::request("force-close", None, &[])?;
                        if status.recovery_required {
                            control::request("lock", None, &[])?;
                        }
                    }
                }
                return Ok(());
            }
            if !status.general_present
                && status.general_incomplete
                && !status.general_open
                && key == b'd'
            {
                if confirm_discard(&mut terminal, "preparazione generale incompleta")? {
                    busy(&mut terminal, "Eliminazione del tentativo incompleto")?;
                    control::request("discard-general-pending", None, &[])?;
                }
                return Ok(());
            }
            if !status.general_mounted {
                return Ok(());
            }
            if key == b'r' {
                transport_menu(&mut terminal)?;
                return Ok(());
            }
            if key == b'g' {
                key_menu(&mut terminal, false)?;
                return Ok(());
            }
            if status.general_readonly {
                return Ok(());
            }
            if key == b'a' {
                if let Some(secret) = terminal
                    .password("API key Abliteration - salvata nel volume generale cifrato")?
                {
                    busy(&mut terminal, "Salvataggio della API key")?;
                    control::request("set-api-key", None, &secret)?;
                }
                return Ok(());
            }
            if key == b'x' && status.api_key_present == Some(true) {
                terminal.flush_input();
                terminal.write(&format!("{CLEAR} Rimuovere la API key salvata?\r\n Premi X di nuovo per confermare; altro tasto annulla."))?;
                if terminal.input(-1)?.as_slice().eq_ignore_ascii_case(b"x") {
                    busy(&mut terminal, "Rimozione della API key")?;
                    control::request("clear-api-key", None, &[])?;
                }
                return Ok(());
            }
            if key == b'i' {
                incomplete = !incomplete;
                selected = 0;
                return Ok(());
            }
            if incomplete {
                if key == b'd' {
                    if let Some(id) = sessions.get(selected) {
                        if confirm_discard(
                            &mut terminal,
                            &format!("creazione interrotta {}", &id[..8]),
                        )? {
                            busy(&mut terminal, "Eliminazione del tentativo incompleto")?;
                            control::request("discard-session-pending", Some(id), &[])?;
                        }
                    }
                }
                return Ok(());
            }
            let id = if key == b'n' {
                None
            } else if matches!(key, b'\r' | b'\n') {
                status.sessions.get(selected).map(String::as_str)
            } else {
                return Ok(());
            };
            if key != b'n' && id.is_none() {
                return Ok(());
            }
            if status
                .api_keys
                .as_ref()
                .is_some_and(|keys| keys.entries.len() > 1)
                && !key_menu(&mut terminal, true)?
            {
                return Ok(());
            }
            let Some(secret) = (if id.is_some() {
                terminal.password("Password della sessione")?
            } else {
                new_password(
                    &mut terminal,
                    "Nuova sessione - password di almeno 16 byte",
                    "Ripeti la password della nuova sessione",
                )?
            }) else {
                return Ok(());
            };
            let id = if let Some(id) = id {
                id.to_owned()
            } else {
                busy(&mut terminal, "Creazione del volume cifrato")?;
                control::request("create", None, &secret)?.trim().to_owned()
            };
            busy(&mut terminal, "Apertura della VM della sessione")?;
            control::request("open", Some(&id), &secret)?;
            drop(secret);
            requested_new = enter_console(&mut terminal)?;
            Ok(())
        })();
        if let Err(error) = result {
            notice = match error.to_string().as_str() {
                "passwords_do_not_match" => "Le password non coincidono.",
                "password_too_short" => "Usa almeno 16 byte per la nuova password.",
                "invalid_key_name" => "Nome non valido: da 1 a 64 byte, senza caratteri di controllo o spazi ai bordi.",
                "api_key_missing" => "Seleziona una chiave API salvata prima di aprire la sessione.",
                "too_many_api_keys" => "Archivio pieno: sono consentite al massimo 64 chiavi.",
                "invalid_api_key" => {
                    "API key non valida: usa da 16 a 1024 caratteri ASCII senza spazi."
                }
                "gateway_start_failed" => {
                    "Gateway non disponibile. Il generale resta aperto: puoi bloccarlo e riprovare."
                }
                "gateway_unavailable" => "Gateway non attivo. Blocca il generale e sbloccalo prima di aprire una sessione.",
                "gateway_not_ready" => "Il percorso di rete non è ancora pronto. Attendi e riprova ad aprire la sessione.",
                "gateway_shutdown_unconfirmed" => "Chiusura non confermata. Il gateway è fermo: blocca di nuovo per chiudere il generale.",
                "invalid_proxy_endpoint" => "Proxy non valido: indica un IPv4 e una porta; loopback e rete interna non sono ammessi.",
                "invalid_proxy_authentication" => "Username e password SOCKS devono contenere da 1 a 255 byte, senza caratteri di controllo.",
                "external_proxy_requires_network" => "La rete è disabilitata in questa immagine: il proxy esterno non può essere attivato.",
                "session_must_be_closed" => "Chiudi la sessione prima di cambiare chiave o percorso di rete.",
                "session_channel_failed" | "invalid_session_ready" => "La sessione non ha completato l'avvio protetto. Chiudila prima di riprovare.",
                "insufficient_space" => {
                    "Spazio insufficiente. Controlla le creazioni interrotte prima di riprovare."
                }
                _ => "Operazione non completata. Verifica lo stato mostrato prima di continuare.",
            }
            .to_owned();
        } else if key == b'q' {
            return Ok(());
        }
        terminal.flush_input();
    }
}

fn new_password(
    terminal: &mut Terminal,
    title: &str,
    repeat: &str,
) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>> {
    let Some(secret) = terminal.password(title)? else {
        return Ok(None);
    };
    if secret.len() < 16 {
        return Err("password_too_short".into());
    }
    let Some(confirmation) = terminal.password(repeat)? else {
        return Ok(None);
    };
    if *secret != *confirmation {
        return Err("passwords_do_not_match".into());
    }
    Ok(Some(secret))
}

fn confirm_discard(terminal: &mut Terminal, label: &str) -> Result<bool> {
    terminal.flush_input();
    terminal.write(&format!("{CLEAR}{FEEDBACK} Elimina {label}?\r\n Questa operazione non si puo' annullare.\r\n Premi D di nuovo per confermare; altro tasto annulla.{RESET}"))?;
    Ok(terminal.input(-1)?.as_slice().eq_ignore_ascii_case(b"d"))
}

fn busy(terminal: &mut Terminal, message: &str) -> Result<()> {
    terminal.write(&format!(
        "{CLEAR}{BRAND} OBLIVION - controllo fidato{RESET}\r\n\r\n {message}..."
    ))
}

fn enter_console(terminal: &mut Terminal) -> Result<bool> {
    let result = terminal::bridge(terminal, control::console()?);
    busy(terminal, "Chiusura della sessione")?;
    control::request("close", None, &[])?;
    Ok(result? == terminal::Action::New)
}

fn transport_label(transport: &Metadata) -> String {
    match transport {
        Metadata::Tor {} => "Tor".to_owned(),
        Metadata::Socks5 {
            endpoint,
            authenticated,
        } => format!(
            "SOCKS5 {endpoint}{}",
            if *authenticated { " con password" } else { "" }
        ),
    }
}

fn transport_menu(terminal: &mut Terminal) -> Result<()> {
    let status: Status = serde_json::from_str(&control::request("status", None, &[])?)?;
    let egress = status.egress.ok_or("general_not_open")?;
    terminal.flush_input();
    terminal.write(&format!("{CLEAR}{BRAND} OBLIVION - percorso di rete{RESET}\r\n\r\n Attuale: {}{}\r\n Salvato: {}\r\n\r\n [T] Tor  [S] SOCKS5  [P] Usa profilo salvato\r\n [Esc] Indietro\r\n\r\n Il cambio chiude il gateway precedente.\r\n Nessuna connessione diretta o cambio automatico.\r\n {}",
        transport_label(&egress.selected), if egress.temporary { " (fino al blocco del generale)" } else { "" },
        transport_label(&egress.saved), if status.general_readonly { "Profilo in sola lettura: modifiche solo in RAM." } else { "" }))?;
    let key = terminal.input(-1)?;
    let transport = match key.as_slice() {
        b"p" | b"P" => {
            busy(terminal, "Ripristino del percorso salvato")?;
            control::request("egress", None, br#"{"action":"saved"}"#)?;
            return Ok(());
        }
        b"t" | b"T" => Transport::default(),
        b"s" | b"S" => {
            let Some(transport) = proxy_settings(terminal)? else {
                return Ok(());
            };
            transport
        }
        _ => return Ok(()),
    };
    transport.validate()?;
    let save = if status.general_readonly {
        false
    } else {
        terminal.flush_input();
        terminal.write(&format!("{CLEAR} Percorso scelto: {}\r\n\r\n [S] Salva nel generale cifrato\r\n [T] Usa fino al blocco del generale\r\n [Esc] Annulla", transport_label(&transport.metadata())))?;
        match terminal.input(-1)?.as_slice() {
            b"s" | b"S" => true,
            b"t" | b"T" => false,
            _ => return Ok(()),
        }
    };
    let command = if save {
        egress::Command::Save { transport }
    } else {
        egress::Command::Select { transport }
    };
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&command)?);
    busy(terminal, "Cambio del percorso di rete")?;
    control::request("egress", None, &bytes)?;
    Ok(())
}

fn proxy_settings(terminal: &mut Terminal) -> Result<Option<Transport>> {
    let Some(endpoint) = terminal.text("Proxy SOCKS5 - IPv4:porta (esempio 192.168.1.20:1080)")?
    else {
        return Ok(None);
    };
    let endpoint = endpoint.parse().map_err(|_| "invalid_proxy_endpoint")?;
    terminal.flush_input();
    terminal.write(&format!("{CLEAR} Il proxy vede le destinazioni richieste.\r\n Username e password SOCKS viaggiano in chiaro fino al proxy.\r\n Le richieste API usano HTTPS verificato.\r\n\r\n [A] Con autenticazione  [N] Senza autenticazione\r\n [Esc] Annulla"))?;
    let authentication = match terminal.input(-1)?.as_slice() {
        b"a" | b"A" => {
            let Some(username) = terminal.password("Username SOCKS5 (nascosto)")? else {
                return Ok(None);
            };
            let Some(password) = terminal.password("Password SOCKS5")? else {
                return Ok(None);
            };
            Some(Authentication {
                username: zeroize::Zeroizing::new(std::str::from_utf8(&username)?.to_owned()),
                password: zeroize::Zeroizing::new(std::str::from_utf8(&password)?.to_owned()),
            })
        }
        b"n" | b"N" => None,
        _ => return Ok(None),
    };
    Ok(Some(Transport::Socks5 {
        endpoint,
        authentication,
    }))
}

fn key_menu(terminal: &mut Terminal, choosing: bool) -> Result<bool> {
    let mut selected = 0usize;
    loop {
        let status: Status = serde_json::from_str(&control::request("status", None, &[])?)?;
        let keys = status.api_keys.ok_or("general_not_open")?;
        selected = selected.min(keys.entries.len().saturating_sub(1));
        let mut view = format!("{CLEAR}{BRAND} OBLIVION - chiavi Abliteration{RESET}\r\n\r\n");
        if choosing {
            view.push_str(" Scegli la chiave per questa apertura.\r\n");
        }
        let limit = terminal.size().0.saturating_sub(10).max(1) as usize;
        for (index, entry) in keys
            .entries
            .iter()
            .enumerate()
            .skip(selected.saturating_sub(limit - 1))
            .take(limit)
        {
            view.push_str(&format!(
                " {} {}{}{}\r\n",
                if index == selected { ">" } else { " " },
                entry.name,
                if keys.default.as_ref() == Some(&entry.id) {
                    " [predefinita]"
                } else {
                    ""
                },
                if keys.selected.as_ref() == Some(&entry.id) {
                    " [selezionata]"
                } else {
                    ""
                }
            ));
        }
        if keys.entries.is_empty() {
            view.push_str(" Nessuna chiave salvata.\r\n");
        }
        view.push_str("\r\n [Invio] Usa  [J/K] Seleziona  [Esc] Indietro\r\n");
        if !status.general_readonly && !choosing {
            view.push_str(
                " [A] Aggiungi  [R] Rinomina  [S] Sostituisci\r\n [D] Predefinita  [X] Rimuovi\r\n",
            );
        }
        terminal.write(&view)?;
        let input = terminal.input(-1)?;
        let byte = match input.as_slice() {
            b"\x1b[A" => b'k',
            b"\x1b[B" => b'j',
            [byte] => byte.to_ascii_lowercase(),
            _ => continue,
        };
        if byte == 27 {
            return Ok(false);
        }
        if byte == b'j' {
            selected = (selected + 1).min(keys.entries.len().saturating_sub(1));
            continue;
        }
        if byte == b'k' {
            selected = selected.saturating_sub(1);
            continue;
        }
        let entry = keys.entries.get(selected);
        let command = if matches!(byte, b'\r' | b'\n') {
            let Some(entry) = entry else {
                continue;
            };
            keys::Command::Select {
                id: entry.id.clone(),
            }
        } else {
            if status.general_readonly || choosing {
                continue;
            }
            if byte == b'a' {
                let Some(name) = terminal.name("Nome della nuova chiave (massimo 64 byte)")? else {
                    continue;
                };
                let Some(bytes) = terminal
                    .password("API key Abliteration - salvata nel volume generale cifrato")?
                else {
                    continue;
                };
                keys::Command::Add {
                    name,
                    key: zeroize::Zeroizing::new(std::str::from_utf8(&bytes)?.to_owned()),
                }
            } else {
                let Some(entry) = entry else {
                    continue;
                };
                match byte {
                    b'r' => {
                        let Some(name) = terminal.name("Nuovo nome della chiave")? else {
                            continue;
                        };
                        keys::Command::Rename {
                            id: entry.id.clone(),
                            name,
                        }
                    }
                    b's' => {
                        let Some(bytes) =
                            terminal.password("Sostituisci la API key selezionata")?
                        else {
                            continue;
                        };
                        keys::Command::Replace {
                            id: entry.id.clone(),
                            key: zeroize::Zeroizing::new(std::str::from_utf8(&bytes)?.to_owned()),
                        }
                    }
                    b'd' => keys::Command::Default {
                        id: entry.id.clone(),
                    },
                    b'x' => {
                        terminal.flush_input();
                        terminal.write(&format!("{CLEAR} Rimuovere la chiave {}?\r\n Premi X di nuovo per confermare; altro tasto annulla.", entry.name))?;
                        if !terminal.input(-1)?.as_slice().eq_ignore_ascii_case(b"x") {
                            continue;
                        }
                        keys::Command::Remove {
                            id: entry.id.clone(),
                        }
                    }
                    _ => continue,
                }
            }
        };
        let selected = matches!(command, keys::Command::Select { .. });
        let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&command)?);
        busy(terminal, "Aggiornamento delle chiavi")?;
        control::request("api-keys", None, &bytes)?;
        if selected {
            return Ok(true);
        }
    }
}
