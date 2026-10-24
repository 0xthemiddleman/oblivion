use oblivion_gateway::{Broker, Credentials};
use std::{io, os::fd::FromRawFd, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, UnixStream},
    signal::unix::{SignalKind, signal},
    time::timeout,
};
use zeroize::Zeroizing;

fn main() {
    // No configurable URL, command-line secret, environment credential or file.
    // FD 3 is a private controller stream; FD 4 is an already bound listener.
    let result = harden().and_then(|()| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?
            .block_on(run())
    });
    if result.is_err() {
        eprintln!("oblivion-api: service_failed");
        std::process::exit(1);
    }
}

async fn run() -> io::Result<()> {
    let control = unsafe { std::os::unix::net::UnixStream::from_raw_fd(3) };
    control.peer_addr()?;
    control.set_nonblocking(true)?;
    let listener = unsafe { std::net::TcpListener::from_raw_fd(4) };
    listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(listener)?;
    let mut control = BufReader::new(UnixStream::from_std(control)?);
    let mut input = Zeroizing::new(Vec::new());
    timeout(
        Duration::from_secs(5),
        (&mut control).take(8193).read_until(b'\n', &mut input),
    )
    .await??;
    if input.len() > 8192 || !input.ends_with(b"\n") {
        return Err(io::Error::other("invalid_control"));
    }
    let credentials: Credentials =
        serde_json::from_slice(&input).map_err(|_| io::Error::other("invalid_control"))?;
    drop(input);
    let broker = Arc::new(Broker::new(credentials).map_err(io::Error::other)?);
    let mut termination = signal(SignalKind::terminate())?;
    let mut interruption = signal(SignalKind::interrupt())?;
    control.get_mut().write_all(b"{\"ready\":true}\n").await?;
    let service = broker.clone().serve(listener);
    tokio::pin!(service);
    tokio::select! {
        biased;
        _ = control.read_u8() => {}, // EOF, error, or any byte revokes the grant.
        _ = termination.recv() => {},
        _ = interruption.recv() => {},
        result = &mut service => return result,
    }
    broker.revoke();
    service.await
}

fn harden() -> io::Result<()> {
    if std::env::args_os().len() != 1 {
        return Err(io::Error::other("arguments_denied"));
    }
    // Check inherited FDs before Tokio can allocate its own descriptors. Taking
    // ownership of an absent FD after runtime startup could close its poller.
    for (fd, control) in [(3, true), (4, false)] {
        let mut kind = 0;
        let mut length = size_of::<libc::c_int>() as libc::socklen_t;
        let mut address: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut address_length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut libc::c_int).cast(),
                &mut length,
            )
        } != 0
            || kind != libc::SOCK_STREAM
            || unsafe {
                libc::getsockname(
                    fd,
                    (&mut address as *mut libc::sockaddr_storage).cast(),
                    &mut address_length,
                )
            } != 0
            || (control && address.ss_family as i32 != libc::AF_UNIX)
            || (!control && ![libc::AF_INET, libc::AF_INET6].contains(&(address.ss_family as i32)))
            || unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } != 0
        {
            return Err(io::Error::other("invalid_descriptors"));
        }
        // Darwin exposes the constant but rejects SO_ACCEPTCONN. The Mac lab
        // still validates socket type/family; production is Linux-only.
        #[cfg(target_os = "linux")]
        if !control {
            let mut accepting = 0;
            if unsafe {
                libc::getsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_ACCEPTCONN,
                    (&mut accepting as *mut libc::c_int).cast(),
                    &mut length,
                )
            } != 0
                || accepting == 0
            {
                return Err(io::Error::other("listener_required"));
            }
        }
    }
    let core = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core) } != 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe { libc::umask(0o077) };
    #[cfg(target_os = "linux")]
    {
        if unsafe { libc::geteuid() } == 0
            || std::fs::read_to_string("/proc/swaps")?.lines().count() != 1
            || unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0
            || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        {
            return Err(io::Error::other("isolation_required"));
        }
        let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = size_of::<libc::ucred>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                3,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut peer as *mut libc::ucred).cast(),
                &mut length,
            )
        } != 0
            || (!cfg!(feature = "lab-emulation") && peer.uid != 0)
        {
            return Err(io::Error::other("controller_required"));
        }
    }
    #[cfg(not(target_os = "linux"))]
    if !cfg!(feature = "lab-emulation") {
        return Err(io::Error::other("linux_required"));
    }
    Ok(())
}
