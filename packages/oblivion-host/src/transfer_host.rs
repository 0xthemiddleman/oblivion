use crate::storage::{Error, Result, require};
use crate::vm::{self, Role};
use oblivion_host::channel::Channel;
use oblivion_host::transfers::{self, Source, Started, Status};
use oblivion_transfer::relay::{self, Permit};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, Instant};

const UNIT: &str = "oblivion-transfer-host.service";
const JOB: &str = "/run/oblivion/transfer.json";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Job {
    id: String,
    #[serde(default)]
    recovery: bool,
}

pub fn status(configured: bool, general: &Path) -> Result<Status> {
    Ok(Status {
        configured,
        active: !matches!(vm::unit_state(UNIT)?.as_str(), "inactive" | "failed"),
        sources: if configured {
            sources(general)?
        } else {
            Vec::new()
        },
    })
}

fn sources(general: &Path) -> Result<Vec<Source>> {
    let entries: Vec<_> = fs::read_dir("/sys/class/block")
        .map_err(|_| Error("device_list_failed"))?
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| fs::canonicalize(entry.path()).ok())
        .collect();
    let mounted =
        fs::read_to_string("/proc/self/mountinfo").map_err(|_| Error("mount_check_failed"))?;
    let backing = fs::metadata(general)
        .map_err(|_| Error("image_check_failed"))?
        .dev();
    let mut devices = Vec::new();
    for path in &entries {
        // Offer leaf devices only. A partitioned whole disk must never bypass
        // the exclusion of a mounted partition or the live/profile medium.
        if entries
            .iter()
            .any(|child| child.parent() == Some(path.as_path()))
        {
            continue;
        }
        let usb = path.ancestors().any(|parent| {
            fs::read_link(parent.join("subsystem"))
                .ok()
                .is_some_and(|link| link.ends_with("usb"))
        });
        let fixture = cfg!(feature = "lab-emulation")
            && fs::read_to_string(path.join("serial"))
                .ok()
                .is_some_and(|serial| serial.trim() == "oblivion-import");
        if !usb && !fixture {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            continue;
        }
        let Ok(id) = fs::read_to_string(path.join("dev")) else {
            continue;
        };
        let id = id.trim();
        if mounted
            .lines()
            .any(|line| line.split_whitespace().nth(2) == Some(id))
        {
            continue;
        }
        let Some((major, minor)) = id.split_once(':').and_then(|(major, minor)| {
            Some((major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?))
        }) else {
            continue;
        };
        if libc::major(backing) == major && libc::minor(backing) == minor {
            continue;
        }
        let Some(bytes) = fs::read_to_string(path.join("size"))
            .ok()
            .and_then(|n| n.trim().parse::<u64>().ok())
            .and_then(|n| n.checked_mul(512))
            .filter(|n| *n > 0)
        else {
            continue;
        };
        devices.push(Source {
            id: id.to_owned(),
            name: name.to_owned(),
            bytes,
        });
    }
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(devices)
}

pub fn start(source: &str, general: &Path) -> Result<Started> {
    idle()?;
    let source = sources(general)?
        .into_iter()
        .find(|device| device.id == source)
        .ok_or(Error("source_not_available"))?;
    let path = format!("/dev/{}", source.name);
    let device = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| Error("source_not_available"))?;
    let metadata = device
        .metadata()
        .map_err(|_| Error("source_not_available"))?;
    require(
        metadata.file_type().is_block_device()
            && format!(
                "{}:{}",
                libc::major(metadata.rdev()),
                libc::minor(metadata.rdev())
            ) == source.id,
        "source_changed",
    )?;
    // A readonly mapping holds the selected block device for the operation.
    // Only this mapping is delegated to the networkless import VM; the host
    // never mounts or parses the external filesystem.
    vm::execute(
        "/usr/sbin/dmsetup",
        &[
            "create".into(),
            "oblivion-import".into(),
            "--readonly".into(),
            "--table".into(),
            format!("0 {} linear {} 0", source.bytes / 512, source.id),
        ],
    )?;
    let path = CString::new(vm::IMPORT_DEVICE).map_err(|_| Error("invalid_device"))?;
    require(
        unsafe { libc::chown(path.as_ptr(), vm::IMPORT_UID, vm::IMPORT_UID) } == 0,
        "device_owner_failed",
    )?;
    fs::set_permissions(vm::IMPORT_DEVICE, fs::Permissions::from_mode(0o400))
        .map_err(|_| Error("device_permissions_failed"))?;
    start_job(false)
}

pub fn start_recovery() -> Result<Started> {
    idle()?;
    start_job(true)
}

fn idle() -> Result<()> {
    require(
        matches!(vm::unit_state(UNIT)?.as_str(), "inactive" | "failed")
            && vm::stopped_role(Role::Import)?,
        "transfer_already_running",
    )?;
    require(
        !Path::new(JOB).exists() && !Path::new(vm::IMPORT_DEVICE).exists(),
        "transfer_cleanup_required",
    )?;
    Ok(())
}

fn start_job(recovery: bool) -> Result<Started> {
    let id = crate::storage::random_id()?;
    let mut job = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(JOB)
        .map_err(|_| Error("transfer_state_failed"))?;
    job.write_all(
        &serde_json::to_vec(&Job {
            id: id.clone(),
            recovery,
        })
        .map_err(|_| Error("transfer_state_failed"))?,
    )
    .map_err(|_| Error("transfer_state_failed"))?;
    vm::execute("/usr/bin/systemctl", &["start".into(), UNIT.into()])?;
    Ok(Started { id })
}

pub fn prepared() -> Result<bool> {
    Ok(Path::new(JOB)
        .try_exists()
        .map_err(|_| Error("transfer_state_failed"))?
        || Path::new(vm::IMPORT_DEVICE)
            .try_exists()
            .map_err(|_| Error("device_check_failed"))?)
}

pub fn stop() -> Result<()> {
    if !matches!(vm::unit_state(UNIT)?.as_str(), "inactive" | "failed") {
        vm::execute("/usr/bin/systemctl", &["stop".into(), UNIT.into()])?;
    }
    require(
        matches!(vm::unit_state(UNIT)?.as_str(), "inactive" | "failed"),
        "transfer_still_running",
    )?;
    cleanup()
}

fn job() -> Result<Option<Job>> {
    if !Path::new(JOB)
        .try_exists()
        .map_err(|_| Error("transfer_state_failed"))?
    {
        return Ok(None);
    }
    let file = crate::storage::trusted_file(Path::new(JOB))?;
    require(
        file.metadata()
            .map_err(|_| Error("transfer_state_failed"))?
            .len()
            <= 1024,
        "invalid_transfer_state",
    )?;
    let job: Job = serde_json::from_reader(file).map_err(|_| Error("invalid_transfer_state"))?;
    require(
        job.id.len() == 32
            && job
                .id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid_transfer_state",
    )?;
    Ok(Some(job))
}

pub fn cleanup() -> Result<()> {
    vm::stop_role(Role::Import, true)?;
    if let Some(job) = job()? {
        vm::remove_copy_port(&job.id)?;
    }
    if Path::new(vm::IMPORT_DEVICE)
        .try_exists()
        .map_err(|_| Error("device_check_failed"))?
    {
        vm::execute(
            "/usr/sbin/dmsetup",
            &["remove".into(), "oblivion-import".into()],
        )?;
    }
    if job()?.is_some() {
        fs::remove_file(JOB).map_err(|_| Error("transfer_cleanup_failed"))?;
    }
    Ok(())
}

pub fn serve(config: Option<&vm::Config>, uid: u32) -> Result<()> {
    let job = job()?.ok_or(Error("transfer_not_prepared"))?;
    let source = if job.recovery {
        None
    } else {
        let config = config.ok_or(Error("transfers_not_configured"))?;
        config.validate()?;
        config.start_role(Role::Import)?;
        let source = UnixStream::connect("/run/oblivion-import-vm/copy.sock")
            .map_err(|_| Error("transfer_port_unavailable"))?;
        crate::gateway::peer(&source, vm::IMPORT_UID)?;
        vm::resume_import()?;
        Some(source)
    };
    let destination = vm::copy_port(&job.id)?;
    for port in source.iter().chain(std::iter::once(&destination)) {
        let mut ready = [0; 6];
        Channel::new(port, Duration::from_secs(90))
            .read_exact(&mut ready)
            .map_err(|_| Error("transfer_guest_not_ready"))?;
        require(&ready == b"ready\n", "invalid_transfer_guest")?;
    }
    let source = source.map(|source| File::from(OwnedFd::from(source)));
    let destination = File::from(OwnedFd::from(destination));
    let mut owners = source
        .iter()
        .chain(std::iter::once(&destination))
        .map(|port| relay::peer_process(port).map_err(|_| Error("transfer_owner_failed")))
        .collect::<Result<Vec<_>>>()?;
    let listener =
        UnixListener::bind(transfers::SOCKET).map_err(|_| Error("transfer_socket_failed"))?;
    fs::set_permissions(transfers::SOCKET, fs::Permissions::from_mode(0o600))
        .map_err(|_| Error("transfer_socket_failed"))?;
    require(
        unsafe {
            libc::chown(
                c"/run/oblivion-transfer-host/control.sock".as_ptr(),
                uid,
                !0,
            )
        } == 0,
        "transfer_socket_failed",
    )?;
    listener
        .set_nonblocking(true)
        .map_err(|_| Error("transfer_socket_failed"))?;
    crate::ipc::notify_ready()?;
    let deadline = Instant::now() + Duration::from_secs(transfers::BUDGET_SECONDS);
    loop {
        let mut descriptors = vec![libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        descriptors.extend(owners.iter().map(|owner| libc::pollfd {
            fd: owner.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }));
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error("transfer_expired"))?;
        require(
            unsafe {
                libc::poll(
                    descriptors.as_mut_ptr(),
                    descriptors.len() as _,
                    remaining.as_millis().min(i32::MAX as u128) as i32,
                )
            } >= 0,
            "transfer_wait_failed",
        )?;
        require(
            !descriptors[1..].iter().any(|fd| fd.revents != 0),
            "transfer_owner_exited",
        )?;
        if descriptors[0].revents == 0 {
            continue;
        }
        let (control, _) = listener
            .accept()
            .map_err(|_| Error("transfer_socket_failed"))?;
        crate::gateway::peer(&control, uid)?;
        for port in source.iter().chain(std::iter::once(&destination)) {
            let mut port = port;
            port.write_all(if job.recovery { b"R" } else { b"G" })
                .map_err(|_| Error("transfer_guest_not_ready"))?;
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error("transfer_expired"))?;
        let control = File::from(OwnedFd::from(control));
        let Some(source) = source else {
            return relay::recover(
                destination,
                control,
                owners.pop().ok_or(Error("transfer_owner_failed"))?,
                remaining,
            )
            .map_err(|_| Error("transfer_failed"));
        };
        return relay::run_permitted(
            source,
            destination,
            control,
            owners,
            remaining,
            Some(Permit {
                id: job.id,
                limits: transfers::limits(),
            }),
        )
        .map_err(|_| Error("transfer_failed"));
    }
}
