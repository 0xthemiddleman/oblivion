use oblivion_host::control::{self, Result};
use oblivion_host::terminal::{self, ACTION, BRAND, CLEAR, FEEDBACK, LOGO, RESET, Terminal};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    general_open: bool,
    general_mounted: bool,
    general_readonly: bool,
    session_open: bool,
    sessions: Vec<String>,
    vm_state: Option<String>,
    laboratory_emulation: bool,
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
    loop {
        let status: Status = serde_json::from_str(&control::request("status", None, &[])?)?;
        if status.sessions.iter().any(|s| {
            s.len() != 32
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err("invalid_session_list".into());
        }
        selected = selected.min(status.sessions.len().saturating_sub(1));
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
        if !status.general_mounted {
            view.push_str(if status.general_open {
                " Archivio generale da richiudere.\r\n\r\n [B] Blocca\r\n"
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
            view.push_str(" Sessioni protette\r\n\r\n");
            let limit = rows
                .saturating_sub(if rows >= 20 && cols >= 64 { 17 } else { 12 })
                .max(1) as usize;
            let start = selected.saturating_sub(limit - 1);
            for (index, session) in status.sessions.iter().enumerate().skip(start).take(limit) {
                view.push_str(&format!(
                    " {} Sessione {}  Bloccata\r\n",
                    if index == selected { ">" } else { " " },
                    &session[..8]
                ));
            }
            if status.sessions.is_empty() {
                view.push_str(" Nessuna sessione.\r\n");
            }
            view.push_str(&format!("\r\n {ACTION}[N] Nuova  [Invio] Apri  [J/K] Seleziona\r\n [B] Blocca Oblivion{RESET}\r\n"));
        }
        view.push_str(&format!(
            "\r\n {FEEDBACK}{notice}{RESET}\r\n [Q] Blocca tutto ed esci"
        ));
        terminal.write(&view)?;
        let input = terminal.input(-1)?;
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
            selected = (selected + 1).min(status.sessions.len().saturating_sub(1));
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
            if !status.general_open && matches!(key, b'\r' | b'\n') {
                if let Some(secret) = terminal.password("Password del volume generale")? {
                    busy(&mut terminal, "Sblocco del volume generale")?;
                    control::request("unlock", None, &secret)?;
                }
                return Ok(());
            }
            if status.session_open {
                if matches!(key, b'\r' | b'\n') {
                    let result = terminal::bridge(&mut terminal, control::console()?);
                    busy(&mut terminal, "Chiusura della sessione")?;
                    control::request("close", None, &[])?;
                    result?;
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
                    }
                }
                return Ok(());
            }
            if !status.general_mounted || status.general_readonly {
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
            let Some(secret) = terminal.password(if id.is_some() {
                "Password della sessione"
            } else {
                "Nuova sessione - password di almeno 16 byte"
            })?
            else {
                return Ok(());
            };
            let id = if let Some(id) = id {
                id.to_owned()
            } else {
                if secret.len() < 16 {
                    return Err("password_too_short".into());
                }
                let Some(confirmation) =
                    terminal.password("Ripeti la password della nuova sessione")?
                else {
                    return Ok(());
                };
                if *secret != *confirmation {
                    return Err("passwords_do_not_match".into());
                }
                busy(&mut terminal, "Creazione del volume cifrato")?;
                control::request("create", None, &secret)?.trim().to_owned()
            };
            busy(&mut terminal, "Apertura della VM della sessione")?;
            control::request("open", Some(&id), &secret)?;
            drop(secret);
            let result = terminal::bridge(&mut terminal, control::console()?);
            busy(&mut terminal, "Chiusura della sessione")?;
            control::request("close", None, &[])?;
            result
        })();
        if let Err(error) = result {
            notice = match error.to_string().as_str() {
                "passwords_do_not_match" => "Le password non coincidono.",
                "password_too_short" => "Usa almeno 16 byte per la nuova password.",
                _ => "Operazione non completata. Verifica lo stato mostrato prima di continuare.",
            }
            .to_owned();
        } else if key == b'q' {
            return Ok(());
        }
        terminal.flush_input();
    }
}

fn busy(terminal: &mut Terminal, message: &str) -> Result<()> {
    terminal.write(&format!(
        "{CLEAR}{BRAND} OBLIVION - controllo fidato{RESET}\r\n\r\n {message}..."
    ))
}
