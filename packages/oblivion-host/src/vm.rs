use crate::storage::{Error, Result, require};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const UNIT: &str = "oblivion-session.service";
const DEVICE: &str = "/dev/mapper/oblivion-session";
const QMP: &str = "/run/oblivion-session/qmp.sock";
pub const VM_UID: u32 = 1100;

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Accelerator {
    #[default]
    Kvm,
    #[cfg(feature = "lab-emulation")]
    Tcg,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    root: PathBuf,
    kernel: PathBuf,
    initrd: PathBuf,
    filesystem: PathBuf,
    memory_mib: u32,
    cpus: u8,
    #[serde(default)]
    accelerator: Accelerator,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        require(
            (256..=32768).contains(&self.memory_mib),
            "invalid_vm_memory",
        )?;
        require((1..=16).contains(&self.cpus), "invalid_vm_cpus")?;
        for path in [&self.root, &self.kernel, &self.initrd, &self.filesystem] {
            require(
                path.is_absolute()
                    && !path
                        .as_os_str()
                        .as_encoded_bytes()
                        .iter()
                        .any(|byte| b",:%".contains(byte) || byte.is_ascii_whitespace()),
                "invalid_vm_path",
            )?;
            let meta = fs::symlink_metadata(path).map_err(|_| Error("vm_image_missing"))?;
            require(
                meta.uid() == 0 && meta.mode() & 0o022 == 0 && !meta.file_type().is_symlink(),
                "unsafe_vm_image",
            )?;
            require(
                if path == &self.root {
                    meta.is_dir()
                } else {
                    meta.is_file()
                },
                "invalid_vm_image",
            )?;
        }
        if matches!(self.accelerator, Accelerator::Kvm) {
            let device = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/kvm")
                .map_err(|_| Error("kvm_required"))?;
            require(
                device
                    .metadata()
                    .map_err(|_| Error("kvm_required"))?
                    .file_type()
                    .is_char_device(),
                "kvm_required",
            )?;
            // KVM_GET_API_VERSION: checking a real usable device, not just CPU flags.
            require(
                unsafe { libc::ioctl(device.as_raw_fd(), 0xae00) } == 12,
                "kvm_api_unavailable",
            )?;
        }
        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        require(stopped()?, "vm_already_running")?;
        let device = fs::metadata(DEVICE).map_err(|_| Error("session_device_missing"))?;
        require(
            device.file_type().is_block_device(),
            "invalid_session_device",
        )?;
        unsafe {
            require(
                libc::chown(c"/dev/mapper/oblivion-session".as_ptr(), VM_UID, VM_UID) == 0,
                "device_owner_failed",
            )?;
        }
        let mut args = vec![
            "--quiet".to_owned(),
            "--collect".to_owned(),
            "--unit=oblivion-session".to_owned(),
            "--description=Oblivion isolated session".to_owned(),
            "--service-type=exec".to_owned(),
        ];
        let properties = vec![
            "User=1100".to_owned(),
            "Group=1100".to_owned(),
            "UMask=0077".to_owned(),
            format!("RootDirectory={}", self.root.display()),
            format!(
                "BindReadOnlyPaths={}:/kernel {}:/initrd {}:/root.squashfs",
                self.kernel.display(),
                self.initrd.display(),
                self.filesystem.display()
            ),
            format!(
                "BindPaths={DEVICE}:/dev/oblivion-session{}",
                if matches!(self.accelerator, Accelerator::Kvm) {
                    " /dev/kvm"
                } else {
                    ""
                }
            ),
            "RuntimeDirectory=oblivion-session".to_owned(),
            "RuntimeDirectoryMode=0700".to_owned(),
            "ReadWritePaths=/run/oblivion-session".to_owned(),
            "TemporaryFileSystem=/tmp:rw,nosuid,nodev,size=32M".to_owned(),
            "PrivateDevices=yes".to_owned(),
            format!("DeviceAllow={DEVICE} rw"),
            "PrivateNetwork=yes".to_owned(),
            "RestrictAddressFamilies=AF_UNIX".to_owned(),
            "ProtectSystem=strict".to_owned(),
            "ProtectHome=yes".to_owned(),
            "ProtectProc=invisible".to_owned(),
            "ProcSubset=pid".to_owned(),
            "ProtectKernelTunables=yes".to_owned(),
            "ProtectKernelModules=yes".to_owned(),
            "ProtectKernelLogs=yes".to_owned(),
            "ProtectControlGroups=yes".to_owned(),
            "NoNewPrivileges=yes".to_owned(),
            "CapabilityBoundingSet=".to_owned(),
            "RestrictSUIDSGID=yes".to_owned(),
            "LockPersonality=yes".to_owned(),
            "LimitCORE=0".to_owned(),
            "StandardOutput=null".to_owned(),
            if cfg!(feature = "lab-emulation") {
                "StandardError=journal"
            } else {
                "StandardError=null"
            }
            .to_owned(),
            "KillMode=control-group".to_owned(),
            "TimeoutStopSec=5".to_owned(),
            "SendSIGKILL=yes".to_owned(),
            "Restart=no".to_owned(),
            "TasksMax=128".to_owned(),
            format!("MemoryMax={}M", self.memory_mib + 512),
            "MemorySwapMax=0".to_owned(),
        ];
        args.extend(
            properties
                .into_iter()
                .map(|property| format!("--property={property}")),
        );
        if matches!(self.accelerator, Accelerator::Kvm) {
            args.extend([
                "--property=DeviceAllow=/dev/kvm rw".to_owned(),
                "--property=SupplementaryGroups=kvm".to_owned(),
            ]);
        }
        let (binary, machine, console) = if cfg!(target_arch = "aarch64") {
            ("/usr/bin/qemu-system-aarch64", "virt", "ttyAMA0")
        } else {
            ("/usr/bin/qemu-system-x86_64", "q35", "ttyS0")
        };
        let (accelerator, cpu) = match self.accelerator {
            Accelerator::Kvm => ("kvm", "host"),
            #[cfg(feature = "lab-emulation")]
            Accelerator::Tcg => (
                "tcg",
                if cfg!(target_arch = "aarch64") {
                    "cortex-a72"
                } else {
                    "max"
                },
            ),
        };
        args.extend([
            binary.to_owned(), "-machine".to_owned(), format!("{machine},dump-guest-core=off,mem-merge=off"),
            "-accel".to_owned(), accelerator.to_owned(), "-cpu".to_owned(), cpu.to_owned(),
            "-m".to_owned(), self.memory_mib.to_string(), "-smp".to_owned(), self.cpus.to_string(),
            "-nodefaults".to_owned(), "-no-reboot".to_owned(), "-display".to_owned(), "none".to_owned(),
            "-monitor".to_owned(), "none".to_owned(), "-nic".to_owned(), "none".to_owned(),
            "-object".to_owned(), "rng-random,filename=/dev/urandom,id=rng".to_owned(),
            "-device".to_owned(), "virtio-rng-pci,rng=rng".to_owned(),
            "-sandbox".to_owned(), "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny".to_owned(),
            "-qmp".to_owned(), "unix:/run/oblivion-session/qmp.sock,server=on,wait=off".to_owned(),
            "-serial".to_owned(), "unix:/run/oblivion-session/console.sock,server=on,wait=off".to_owned(),
            "-kernel".to_owned(), "/kernel".to_owned(), "-initrd".to_owned(), "/initrd".to_owned(),
            "-append".to_owned(), format!("root=/dev/vda ro rootfstype=squashfs console={console} quiet loglevel=0 systemd.show_status=false panic=-1"),
            "-drive".to_owned(), "file=/root.squashfs,format=raw,if=none,id=root,readonly=on".to_owned(),
            "-device".to_owned(), "virtio-blk-pci,drive=root,serial=oblivion-root".to_owned(),
            "-drive".to_owned(), "file=/dev/oblivion-session,format=raw,if=none,id=vault,cache=none".to_owned(),
            "-device".to_owned(), "virtio-blk-pci,drive=vault,serial=oblivion-vault".to_owned(),
        ]);
        execute("/usr/bin/systemd-run", &args)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if stopped()? {
                return Err(Error("vm_start_failed"));
            }
            if PathBuf::from(QMP).exists() {
                qmp("query-status")?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(Error("vm_start_timeout"))
    }
}

pub fn state() -> Result<String> {
    let output = Command::new("/usr/bin/systemctl")
        .args(["show", UNIT, "--property=ActiveState", "--value"])
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| Error("vm_state_unavailable"))?;
    require(output.status.success(), "vm_state_unavailable")?;
    let value = String::from_utf8(output.stdout).map_err(|_| Error("invalid_vm_state"))?;
    let state = value.trim();
    require(
        matches!(
            state,
            "active" | "inactive" | "failed" | "activating" | "deactivating"
        ),
        "invalid_vm_state",
    )?;
    Ok(state.to_owned())
}

pub fn stopped() -> Result<bool> {
    Ok(matches!(state()?.as_str(), "inactive" | "failed"))
}

pub fn stop(force: bool) -> Result<()> {
    if stopped()? {
        return Ok(());
    }
    if force {
        execute("/usr/bin/systemctl", &["stop".to_owned(), UNIT.to_owned()])?;
        require(stopped()?, "vm_still_running")?;
        return Ok(());
    }
    qmp("system_powerdown")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if stopped()? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // A forced stop is a separate explicit action; a timeout is not a locked vault.
    Err(Error("vm_shutdown_timeout"))
}

fn execute(program: &str, args: &[String]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| Error("vm_command_failed"))?;
    require(status.success(), "vm_command_failed")
}

fn qmp(operation: &str) -> Result<Value> {
    let stream = UnixStream::connect(QMP).map_err(|_| Error("vm_control_unavailable"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| Error("vm_control_failed"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| Error("vm_control_failed"))?;
    let mut reader = BufReader::new(stream);
    require(
        read_qmp(&mut reader)?.get("QMP").is_some(),
        "invalid_vm_greeting",
    )?;
    for command in ["qmp_capabilities", operation] {
        reader
            .get_mut()
            .write_all(format!("{}\n", json!({"execute": command})).as_bytes())
            .map_err(|_| Error("vm_control_failed"))?;
        loop {
            let response = read_qmp(&mut reader)?;
            if response.get("event").is_some() {
                continue;
            }
            require(
                response.get("error").is_none() && response.get("return").is_some(),
                "vm_control_rejected",
            )?;
            if command == operation {
                return Ok(response);
            }
            break;
        }
    }
    Err(Error("vm_control_failed"))
}

fn read_qmp(reader: &mut BufReader<UnixStream>) -> Result<Value> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|_| Error("vm_control_failed"))?;
        require(!available.is_empty(), "vm_control_closed")?;
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        require(bytes.len() + count <= 65536, "vm_response_too_large")?;
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }
    serde_json::from_slice(&bytes).map_err(|_| Error("invalid_vm_response"))
}
