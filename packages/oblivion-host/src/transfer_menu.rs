use oblivion_host::control::{self, Result};
use oblivion_host::terminal::{ACTION, BRAND, CLEAR, FEEDBACK, RESET, Terminal};
use oblivion_host::transfers::{self, Command, Started, Status};
use oblivion_transfer::channel::Channel;
use oblivion_transfer::endpoint::{Approval, OfferRequest, read_message, write_message};
use oblivion_transfer::{Entry, Manifest, Receipt};
use std::fs::File;
use std::io;
use std::net::Shutdown;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

pub fn run(terminal: &mut Terminal) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let status: Status = serde_json::from_str(&command(&Command::List)?)?;
        if !status.configured {
            terminal.write(&format!("{CLEAR} Importazione non configurata in questa immagine.\r\n Premi un tasto per tornare alla chat."))?;
            terminal.input(-1)?;
            return Ok(());
        }
        selected = selected.min(status.sources.len().saturating_sub(1));
        let mut view = format!(
            "{CLEAR}{BRAND} OBLIVION - importa documenti{RESET}\r\n\r\n Sorgenti USB non montate, aperte solo in lettura.\r\n"
        );
        let (rows, _) = terminal.size();
        let count = rows.saturating_sub(12).max(1) as usize;
        for (index, source) in status
            .sources
            .iter()
            .enumerate()
            .skip(selected.saturating_sub(count - 1))
            .take(count)
        {
            if source.name.len() > 64
                || !source
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            {
                return Err("invalid_source_name".into());
            }
            view.push_str(&format!(
                " {} {} - {} MiB\r\n",
                if index == selected { ">" } else { " " },
                source.name,
                source.bytes / 1048576
            ));
        }
        if status.sources.is_empty() {
            view.push_str(" Nessuna USB disponibile. Collega una USB e premi R.\r\n");
        }
        if status.active {
            view.push_str(" Un trasferimento e' ancora attivo. Premi C per annullarlo.\r\n");
        }
        view.push_str(&format!("\r\n {ACTION}[J/K] Scegli  [Invio] Importa  [R] Aggiorna\r\n [C] Annulla trasferimento  [B] Blocca tutto  [Esc] Indietro{RESET}"));
        terminal.write(&view)?;
        let input = terminal.input(-1)?;
        match input.as_slice() {
            b"\x1b" => return Ok(()),
            b"b" | b"B" => {
                command(&Command::Cancel)?;
                control::request("lock", None, &[])?;
                return Ok(());
            }
            b"c" | b"C" => {
                command(&Command::Cancel)?;
            }
            b"j" | b"J" | b"\x1b[B" => {
                selected = (selected + 1).min(status.sources.len().saturating_sub(1))
            }
            b"k" | b"K" | b"\x1b[A" => selected = selected.saturating_sub(1),
            b"\r" | b"\n" if !status.active && !status.sources.is_empty() => {
                let Some(path) = terminal.text(
                    "Percorso relativo sulla USB: un file o una cartella, senza / iniziale",
                )?
                else {
                    continue;
                };
                if path.is_empty() {
                    continue;
                }
                terminal.write(&format!(
                    "{CLEAR} Preparazione della sorgente in sola lettura..."
                ))?;
                let started: Started = serde_json::from_str(&command(&Command::Import {
                    source: status.sources[selected].id.clone(),
                })?)?;
                let result = import(terminal, &started, path);
                // Reap the worker and remove its VM, port and readonly mapping
                // even after a lost receipt or a locally interrupted dialogue.
                let stopped = command(&Command::Cancel);
                result?;
                stopped?;
                return Ok(());
            }
            _ => {}
        }
    }
}

fn command(value: &Command) -> Result<String> {
    control::request("transfers", None, &serde_json::to_vec(value)?)
}

fn import(terminal: &mut Terminal, started: &Started, path: String) -> Result<()> {
    if started.id.len() != 32
        || !started
            .id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("invalid_transfer_id".into());
    }
    let stream = UnixStream::connect(transfers::SOCKET)?;
    #[cfg(target_os = "linux")]
    {
        let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = size_of::<libc::ucred>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut peer as *mut libc::ucred).cast(),
                &mut length,
            )
        } != 0
            || peer.uid != 0
        {
            return Err("untrusted_transfer_controller".into());
        }
    }
    let mut channel = Channel::new(
        File::from(OwnedFd::from(stream.try_clone()?)),
        Duration::from_secs(transfers::BUDGET_SECONDS),
    )?;
    write_message(
        &mut channel,
        &OfferRequest {
            id: started.id.clone(),
            limits: transfers::limits(),
            paths: vec![path],
        },
    )?;
    terminal.write(&format!(
        "{CLEAR} Lettura dell'elenco e verifica dei file...\r\n [Esc] Annulla  [B] Blocca tutto"
    ))?;
    let Some(manifest) = wait(terminal, &stream, |channel| {
        Manifest::read(channel, transfers::limits())
    })?
    else {
        return Ok(());
    };
    let mut selected = 0usize;
    loop {
        let (rows, cols) = terminal.size();
        let count = rows.saturating_sub(14).max(1) as usize;
        let mut view = format!(
            "{CLEAR}{BRAND} OBLIVION - conferma importazione{RESET}\r\n\r\n {} voci, {} byte. La USB resta invariata.\r\n\r\n",
            manifest.entries().len(),
            manifest.bytes()
        );
        for (index, entry) in manifest
            .entries()
            .iter()
            .enumerate()
            .skip(selected.saturating_sub(count - 1))
            .take(count)
        {
            // Manifest decoding rejects control/bidi characters. Quotes and a
            // fixed ordinal keep source filenames visibly separate from actions.
            let name: String = entry
                .path()
                .chars()
                // A Unicode scalar can occupy two terminal cells. The
                // conservative bound also works without a width database.
                .take(cols.saturating_sub(20) as usize / 2)
                .collect();
            view.push_str(&format!(
                " {} {:4} {} '{}'\r\n",
                if index == selected { ">" } else { " " },
                index + 1,
                if matches!(entry, Entry::Directory { .. }) {
                    "dir "
                } else {
                    "file"
                },
                name
            ));
        }
        view.push_str(&format!("\r\n Confermando, i file verificati saranno disponibili alla chat.\r\n Usarli come contesto puo' inviarne contenuti ad Abliteration.\r\n\r\n {ACTION}[I] Importa nella chat  [J/K] Scorri\r\n [Esc] Annulla  [B] Blocca tutto{RESET}"));
        terminal.write(&view)?;
        let input = terminal.input(1000)?;
        // Refresh against the worker's fixed deadline even while the user is
        // reading; a dead connection must not remain an approvable offer.
        channel.check()?;
        let mut state = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // The protocol is silent until approval. Data, EOF, HUP and errors
        // all invalidate this offer; never consume them as a fresh manifest.
        if unsafe { libc::poll(&mut state, 1, 0) } < 0 || state.revents != 0 {
            return Err("transfer_expired".into());
        }
        match input.as_slice() {
            b"\x1b" => return Ok(()),
            b"b" | b"B" => {
                stream.shutdown(Shutdown::Both)?;
                command(&Command::Cancel)?;
                control::request("lock", None, &[])?;
                return Ok(());
            }
            b"j" | b"J" | b"\x1b[B" => {
                selected = (selected + 1).min(manifest.entries().len().saturating_sub(1))
            }
            b"k" | b"K" | b"\x1b[A" => selected = selected.saturating_sub(1),
            b"i" | b"I" => break,
            _ => {}
        }
    }
    write_message(
        &mut channel,
        &Approval {
            id: started.id.clone(),
            manifest_sha256: manifest.sha256(),
        },
    )?;
    terminal.write(&format!(
        "{CLEAR} Copia e verifica in corso...\r\n [Esc] Interrompi  [B] Blocca tutto"
    ))?;
    let Some(receipt): Option<Receipt> =
        wait(terminal, &stream, |channel| read_message(channel, 1024))
            .map_err(|_| "transfer_outcome_unknown")?
    else {
        return Err("transfer_outcome_unknown".into());
    };
    if receipt.id != started.id
        || receipt.manifest_sha256 != manifest.sha256()
        || receipt.entries != manifest.entries().len()
        || receipt.bytes != manifest.bytes()
    {
        return Err("transfer_outcome_unknown".into());
    }
    terminal.write(&format!("{CLEAR}{FEEDBACK} Importazione verificata.{RESET}\r\n\r\n File disponibili nella sessione:\r\n /vault/imports/{}/files\r\n\r\n Premi un tasto per tornare al manager.", started.id))?;
    terminal.input(-1)?;
    Ok(())
}

fn wait<T: Send + 'static>(
    terminal: &mut Terminal,
    stream: &UnixStream,
    read: impl FnOnce(&mut Channel) -> io::Result<T> + Send + 'static,
) -> Result<Option<T>> {
    let copy = stream.try_clone()?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let result = Channel::new(
            File::from(OwnedFd::from(copy)),
            Duration::from_secs(transfers::BUDGET_SECONDS),
        )
        .and_then(|mut channel| read(&mut channel));
        let _ = sender.send(result);
    });
    loop {
        match receiver.try_recv() {
            Ok(result) => {
                worker.join().map_err(|_| "transfer_reader_failed")?;
                return result.map(Some).map_err(|_| "transfer_failed".into());
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                worker.join().map_err(|_| "transfer_reader_failed")?;
                return Err("transfer_failed".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let input = terminal.input(50)?;
        if matches!(input.as_slice(), b"\x1b" | b"b" | b"B") {
            stream.shutdown(Shutdown::Both)?;
            worker.join().map_err(|_| "transfer_reader_failed")?;
            command(&Command::Cancel)?;
            if input.as_slice() != b"\x1b" {
                control::request("lock", None, &[])?;
            }
            return Ok(None);
        }
    }
}
