use crate::storage::{Error, Result, lock_named, require};
use crate::vm::{self, Role};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;
use zeroize::Zeroizing;

const UNIT: &str = "oblivion-gateway-host.service";

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
        } && peer.uid == vm::GATEWAY_UID,
        "invalid_gateway_peer",
    )?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(5))))
        .map_err(|_| Error("gateway_channel_failed"))?;
    vm::resume_gateway()?;
    let ready: Ready = serde_json::from_slice(&receive(&mut stream)?)
        .map_err(|_| Error("invalid_gateway_ready"))?;
    require(ready.ready && ready.version == 1, "invalid_gateway_ready")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
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
    configured(&mut stream)?;
    crate::ipc::notify_ready()?;
    // Tor remains disabled and no API grant is issued until session token
    // delivery and persistent Tor state are integrated. A lost/paused guest
    // must terminate this owner so systemd removes the VM's credential memory.
    loop {
        std::thread::sleep(Duration::from_millis(250));
        send(&mut stream, b"{\"operation\":\"status\"}")?;
        configured(&mut stream)?;
    }
}

fn configured(stream: &mut UnixStream) -> Result<()> {
    let status: Status =
        serde_json::from_slice(&receive(stream)?).map_err(|_| Error("invalid_gateway_status"))?;
    require(
        status.ok && status.configured && !status.active,
        "gateway_not_idle",
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
