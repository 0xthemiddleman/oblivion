use oblivion_host::control::{self, Result};
use oblivion_host::terminal::{self, ACTION, BRAND, CLEAR, FEEDBACK, LOGO, RESET, Terminal};
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
    gateway_state: Option<String>,
    gateway_vm_state: Option<String>,
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
            view.push_str(" Profilo in sola lettura; guest effimero non ancora disponibile.\r\n [B] Blocca\r\n");
        } else {
            view.push_str(if incomplete {
                " Creazioni interrotte - mai aperte come chat\r\n\r\n"
            } else {
                " Sessioni protette\r\n\r\n"
            });
            let limit = rows
                .saturating_sub(if rows >= 20 && cols >= 64 { 20 } else { 15 })
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
                " API Abliteration: {}  [A] Imposta  [X] Rimuovi\r\n",
                if status.api_key_present == Some(true) {
                    "configurata"
                } else {
                    "non configurata"
                }
            ));
            if let Some(gateway) = &status.gateway_state {
                view.push_str(
                    if gateway == "active" && status.gateway_vm_state.as_deref() == Some("active") {
                        " Gateway pronto; rete non ancora abilitata.\r\n"
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
            if !status.general_mounted || status.general_readonly {
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
                "invalid_api_key" => {
                    "API key non valida: usa da 16 a 1024 caratteri ASCII senza spazi."
                }
                "gateway_start_failed" => {
                    "Gateway non disponibile. Il generale resta aperto: puoi bloccarlo e riprovare."
                }
                "gateway_unavailable" => "Gateway non attivo. Blocca il generale e sbloccalo prima di aprire una sessione.",
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
