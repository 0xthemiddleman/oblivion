use crate::storage::{Error, Result, lock_named, require};
use crate::vm::{self, Role};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;
use zeroize::Zeroizing;

const UNIT: &str = "oblivion-gateway-host.service";
const SOCKET: &str = "/run/oblivion/gateway-control.sock";

#[derive(Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Request<'a> {
    Grant { token: &'a str },
    Revoke {},
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    ready: bool,
    version: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    ok: bool,
    configured: bool,
    active: bool,
}

pub fn state() -> Result<String> {
    vm::unit_state(UNIT)
}

pub fn start() -> Result<()> {
    require(
        matches!(state()?.as_str(), "inactive" | "failed"),
        "gateway_already_running",
    )?;
    vm::execute("/usr/bin/systemctl", &["start".to_owned(), UNIT.to_owned()])
        .map_err(|_| Error("gateway_start_failed"))?;
    require(state()? == "active", "gateway_start_failed")
}

pub fn stop() -> Result<()> {
    // Always inspect both units: the host process may have died before its
    // ExecStopPost, or a previous cleanup may have been interrupted.
    if !matches!(state()?.as_str(), "inactive" | "failed") {
        vm::execute("/usr/bin/systemctl", &["stop".to_owned(), UNIT.to_owned()])?;
    }
    vm::stop_role(Role::Gateway, true)?;
    require(
        matches!(state()?.as_str(), "inactive" | "failed"),
        "gateway_still_running",
    )
}

pub fn grant() -> Result<Zeroizing<String>> {
    require(state()? == "active", "gateway_unavailable")?;
    let mut bytes = Zeroizing::new([0u8; 32]);
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(bytes.as_mut()))
        .map_err(|_| Error("random_failed"))?;
    let token = Zeroizing::new(
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    request(&Request::Grant { token: &token }, true)?;
    // Only the point-to-point Ethernet socket is accessible to the session VM.
    // Its parent remains private to UID 1101; systemd binds this one socket.
    let path = c"/run/oblivion-gateway-vm/inside.sock";
    require(
        unsafe { libc::chown(path.as_ptr(), vm::GATEWAY_UID, vm::VM_UID) } == 0,
        "gateway_network_unavailable",
    )?;
    fs::set_permissions(
        Path::new("/run/oblivion-gateway-vm/inside.sock"),
        fs::Permissions::from_mode(0o660),
    )
    .map_err(|_| Error("gateway_network_unavailable"))?;
    Ok(token)
}

pub fn revoke() -> Result<()> {
    if state()? == "active" && request(&Request::Revoke {}, false).is_ok() {
        return Ok(());
    }
    // A missing acknowledgement is not a successful revocation. Destroy the
    // entire gateway before allowing storage closure or another session.
    stop()
}

fn request(request: &Request<'_>, active: bool) -> Result<()> {
    let mut channel = UnixStream::connect(SOCKET).map_err(|_| Error("gateway_unavailable"))?;
    peer(&channel, 0)?;
    channel
        .set_read_timeout(Some(Duration::from_secs(10)))
        .and_then(|()| channel.set_write_timeout(Some(Duration::from_secs(5))))
        .map_err(|_| Error("gateway_channel_failed"))?;
    let bytes =
        Zeroizing::new(serde_json::to_vec(request).map_err(|_| Error("gateway_encode_failed"))?);
    send(&mut channel, &bytes)?;
    configured(&mut channel, active)
}

pub fn serve(config: &vm::Config, key: Zeroizing<Vec<u8>>) -> Result<()> {
    let _lock = lock_named("gateway.lock")?;
    config.validate()?;
    config.start_role(Role::Gateway)?;
    let result = connected(key);
    let stopped = vm::stop_role(Role::Gateway, true);
    result?;
    stopped
}

fn connected(key: Zeroizing<Vec<u8>>) -> Result<()> {
    let mut stream = UnixStream::connect(format!("{}/control.sock", Role::Gateway.runtime()))
        .map_err(|_| Error("gateway_channel_unavailable"))?;
    peer(&stream, vm::GATEWAY_UID)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(5))))
        .map_err(|_| Error("gateway_channel_failed"))?;
    vm::resume_gateway()?;
    let ready: Ready = serde_json::from_slice(&receive(&mut stream)?)
        .map_err(|_| Error("invalid_gateway_ready"))?;
    require(ready.ready && ready.version == 1, "invalid_gateway_ready")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| Error("gateway_channel_failed"))?;
    #[derive(Serialize)]
    struct Configure<'a> {
        operation: &'static str,
        api_key: &'a str,
    }
    let request = Zeroizing::new(
        serde_json::to_vec(&Configure {
            operation: "configure",
            api_key: std::str::from_utf8(&key).map_err(|_| Error("invalid_api_key"))?,
        })
        .map_err(|_| Error("gateway_encode_failed"))?,
    );
    send(&mut stream, &request)?;
    drop(request);
    drop(key);
    configured(&mut stream, false)?;
    if let Ok(meta) = fs::symlink_metadata(SOCKET) {
        require(meta.file_type().is_socket(), "invalid_gateway_socket")?;
        fs::remove_file(SOCKET).map_err(|_| Error("gateway_socket_failed"))?;
    }
    let listener = UnixListener::bind(SOCKET).map_err(|_| Error("gateway_socket_failed"))?;
    listener
        .set_nonblocking(true)
        .map_err(|_| Error("gateway_socket_failed"))?;
    crate::ipc::notify_ready()?;
    let mut active = false;
    // Tor remains disabled until its persistent state handoff is integrated.
    // Only privileged storage workers can request a grant or revocation.
    loop {
        match listener.accept() {
            Ok((mut client, _)) => {
                peer(&client, 0)?;
                client
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .and_then(|()| client.set_write_timeout(Some(Duration::from_secs(5))))
                    .map_err(|_| Error("gateway_channel_failed"))?;
                let bytes = receive(&mut client)?;
                let request: Request<'_> =
                    serde_json::from_slice(&bytes).map_err(|_| Error("invalid_gateway_request"))?;
                let next = match request {
                    Request::Grant { token } => {
                        require(
                            !active
                                && token.len() == 64
                                && token.bytes().all(|byte| byte.is_ascii_hexdigit()),
                            "invalid_gateway_grant",
                        )?;
                        true
                    }
                    Request::Revoke {} => false,
                };
                send(&mut stream, &bytes)?;
                configured(&mut stream, next)?;
                active = next;
                send(
                    &mut client,
                    if active {
                        b"{\"ok\":true,\"configured\":true,\"active\":true}"
                    } else {
                        b"{\"ok\":true,\"configured\":true,\"active\":false}"
                    },
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return Err(Error("gateway_socket_failed")),
        }
        std::thread::sleep(Duration::from_millis(250));
        send(&mut stream, b"{\"operation\":\"status\"}")?;
        configured(&mut stream, active)?;
    }
}

pub fn peer(stream: &UnixStream, uid: u32) -> Result<()> {
    let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    require(
        unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut peer as *mut libc::ucred).cast(),
                &mut length,
            ) == 0
        } && peer.uid == uid,
        "invalid_gateway_peer",
    )
}

fn configured(stream: &mut UnixStream, active: bool) -> Result<()> {
    let status: Status =
        serde_json::from_slice(&receive(stream)?).map_err(|_| Error("invalid_gateway_status"))?;
    require(
        status.ok && status.configured && status.active == active,
        "gateway_state_mismatch",
    )
}

fn send(stream: &mut UnixStream, bytes: &[u8]) -> Result<()> {
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .and_then(|()| stream.write_all(bytes))
        .map_err(|_| Error("gateway_channel_failed"))
}

fn receive(stream: &mut UnixStream) -> Result<Zeroizing<Vec<u8>>> {
    let mut size = [0; 4];
    stream
        .read_exact(&mut size)
        .map_err(|_| Error("gateway_channel_failed"))?;
    let size = u32::from_be_bytes(size) as usize;
    require((1..=16384).contains(&size), "invalid_gateway_frame")?;
    let mut bytes = Zeroizing::new(vec![0; size]);
    stream
        .read_exact(&mut bytes)
        .map_err(|_| Error("gateway_channel_failed"))?;
    Ok(bytes)
}
