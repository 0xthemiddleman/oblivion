use crate::storage::{Error, Result, lock_named, require};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use zeroize::Zeroizing;

pub const SOCKET: &str = "/run/oblivion-control.sock";

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    InitGeneral,
    Unlock,
    UnlockEphemeral,
    Create,
    Open,
    Close,
    ForceClose,
    Lock,
    Status,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    operation: Operation,
    session: Option<String>,
    secret_len: u16,
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    output: String,
    error: Option<String>,
}

pub fn serve(config: &Path, uid: u32) -> Result<()> {
    let _lock = lock_named("daemon.lock")?;
    // /run is root-owned. The lock prevents unlinking the socket of a live daemon.
    // A manager can connect but cannot replace its inode or choose another path.
    if Path::new(SOCKET).exists() {
        fs::remove_file(SOCKET).map_err(|_| Error("socket_cleanup_failed"))?;
    }
    let listener = UnixListener::bind(SOCKET).map_err(|_| Error("socket_bind_failed"))?;
    fs::set_permissions(SOCKET, fs::Permissions::from_mode(0o600))
        .map_err(|_| Error("socket_permissions_failed"))?;
    unsafe {
        require(
            libc::chown(c"/run/oblivion-control.sock".as_ptr(), uid, !0) == 0,
            "socket_owner_failed",
        )?;
    }
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let response = match handle(&mut stream, config, uid) {
            Ok(response) => response,
            Err(error) => Response {
                ok: false,
                output: String::new(),
                error: Some(error.0.to_owned()),
            },
        };
        // One request per connection; neither payloads nor errors are logged.
        if let Ok(bytes) = serde_json::to_vec(&response) {
            let _ = stream.write_all(&bytes);
        }
    }
    Err(Error("listener_stopped"))
}

fn handle(stream: &mut UnixStream, config: &Path, uid: u32) -> Result<Response> {
    let mut peer = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    unsafe {
        require(
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                peer.as_mut_ptr().cast(),
                &mut length,
            ) == 0,
            "peer_check_failed",
        )?;
        require(peer.assume_init().uid == uid, "unauthorized_peer")?;
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| Error("timeout_setup_failed"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| Error("timeout_setup_failed"))?;
    let mut size = [0; 4];
    stream
        .read_exact(&mut size)
        .map_err(|_| Error("invalid_frame"))?;
    let size = u32::from_be_bytes(size) as usize;
    require((1..=1024).contains(&size), "invalid_frame_length")?;
    let mut header = vec![0; size];
    stream
        .read_exact(&mut header)
        .map_err(|_| Error("invalid_frame"))?;
    let request: Request = serde_json::from_slice(&header).map_err(|_| Error("invalid_request"))?;
    require(request.secret_len <= 1024, "invalid_secret_length")?;
    require(
        matches!(request.operation, Operation::Open) == request.session.is_some(),
        "invalid_session_argument",
    )?;
    if let Some(id) = &request.session {
        require(
            id.len() == 32
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid_session_id",
        )?;
    }
    let needs_secret = matches!(
        request.operation,
        Operation::InitGeneral
            | Operation::Unlock
            | Operation::UnlockEphemeral
            | Operation::Create
            | Operation::Open
    );
    require(
        needs_secret == (request.secret_len > 0),
        "unexpected_secret",
    )?;
    let mut secret = Zeroizing::new(vec![0; request.secret_len as usize]);
    stream
        .read_exact(&mut secret)
        .map_err(|_| Error("incomplete_secret"))?;
    let operation = match request.operation {
        Operation::InitGeneral => "init-general",
        Operation::Unlock => "unlock",
        Operation::UnlockEphemeral => "unlock-ephemeral",
        Operation::Create => "create",
        Operation::Open => "enter",
        Operation::Close => "leave",
        Operation::ForceClose => "force-leave",
        Operation::Lock => "lock",
        Operation::Status => "status",
    };
    let mut command =
        Command::new(std::env::current_exe().map_err(|_| Error("executable_unavailable"))?);
    command
        .args(["--config"])
        .arg(config)
        .arg(operation)
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(id) = request.session {
        command.arg(id);
    }
    let mut child = command
        .spawn()
        .map_err(|_| Error("operation_start_failed"))?;
    let input = child
        .stdin
        .take()
        .ok_or(Error("secret_pipe_failed"))?
        .write_all(&secret);
    let output = child
        .wait_with_output()
        .map_err(|_| Error("operation_wait_failed"))?;
    input.map_err(|_| Error("secret_pipe_failed"))?;
    Ok(Response {
        ok: output.status.success(),
        output: String::from_utf8(output.stdout).map_err(|_| Error("invalid_response"))?,
        error: if output.status.success() {
            None
        } else {
            Some(
                String::from_utf8(output.stderr)
                    .map_err(|_| Error("invalid_response"))?
                    .trim()
                    .to_owned(),
            )
        },
    })
}
