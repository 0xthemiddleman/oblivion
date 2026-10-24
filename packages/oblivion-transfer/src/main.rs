use oblivion_transfer::channel::Channel;
use oblivion_transfer::endpoint;
use std::fs::File;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::time::Duration;

fn main() {
    if execute().is_err() {
        eprintln!("transfer_failed");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
            return Err("dump_protection_failed".into());
        }
        libc::umask(0o077);
        #[cfg(target_os = "linux")]
        if libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) != 0
            || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
        {
            return Err("dump_protection_failed".into());
        }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3
        || !matches!(args[0].as_str(), "send" | "receive")
        || !Path::new(&args[1]).is_absolute()
    {
        return Err("invalid_arguments".into());
    }
    let budget: u64 = args[2].parse()?;
    if !(1..=3_600_000).contains(&budget) {
        return Err("invalid_deadline".into());
    }
    // The launcher hands off an already authorized descriptor, never a socket
    // path or port supplied by the peer. Selected names stay off argv/environment.
    let flags = unsafe { libc::fcntl(3, libc::F_GETFD) };
    if flags < 0 || unsafe { libc::fcntl(3, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err("missing_private_channel".into());
    }
    let mut channel = Channel::new(
        unsafe { File::from_raw_fd(3) },
        Duration::from_millis(budget),
    )?;
    if args[0] == "send" {
        endpoint::send(&mut channel, Path::new(&args[1]))?;
    }
    if args[0] == "receive" {
        endpoint::accept(&mut channel, Path::new(&args[1]))?;
    }
    Ok(())
}
