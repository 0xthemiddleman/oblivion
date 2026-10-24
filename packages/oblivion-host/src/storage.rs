use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use zeroize::Zeroizing;

const GENERAL: &str = "/dev/mapper/oblivion-general";
const SESSION: &str = "/dev/mapper/oblivion-session";
const RUN: &str = "/run/oblivion";
const MOUNT: &str = "/run/oblivion/general";
const MIB: u64 = 1024 * 1024;

pub struct Error(pub &'static str);
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    general_image: PathBuf,
    general_mib: u64,
    session_mib: u64,
    pbkdf_memory_kib: u32,
    manager_uid: u32,
}

#[derive(Serialize)]
struct Status {
    general_open: bool,
    general_mounted: bool,
    general_readonly: bool,
    session_open: bool,
    sessions: Vec<String>,
}

pub fn main() -> Result<()> {
    secure_process()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    require(args.len() >= 3 && args[0] == "--config", "usage")?;
    let config = Config::read(Path::new(&args[1]))?;
    if args.len() == 3 && args[2] == "serve" {
        return crate::ipc::serve(Path::new(&args[1]), config.manager_uid);
    }
    let _lock = lock()?;
    let op = args[2].as_str();
    require(args.len() == if op == "open" { 4 } else { 3 }, "usage")?;
    match op {
        "init-general" => config.initialize(&password(true)?),
        "unlock" => config.unlock(&password(false)?, false),
        "unlock-ephemeral" => config.unlock(&password(false)?, true),
        "create" => config.create(&password(true)?),
        "open" => config.open(&args[3], &password(false)?),
        "close" => close_session(),
        "lock" => close_general(),
        "status" => {
            let mode = mount_mode()?;
            let status = Status {
                general_open: mapping_open("oblivion-general")?,
                general_mounted: mode != MountMode::Closed,
                general_readonly: mode == MountMode::ReadOnly,
                session_open: mapping_open("oblivion-session")?,
                sessions: if mode == MountMode::ReadWrite {
                    sessions()?
                } else {
                    Vec::new()
                },
            };
            println!(
                "{}",
                serde_json::to_string(&status).map_err(|_| Error("encode_failed"))?
            );
            Ok(())
        }
        _ => Err(Error("unknown_operation")),
    }
}

impl Config {
    fn read(path: &Path) -> Result<Self> {
        let file = trusted_file(path)?;
        require(
            file.metadata()
                .map_err(|_| Error("config_unavailable"))?
                .len()
                <= 4096,
            "config_too_large",
        )?;
        let config: Self = serde_json::from_reader(file).map_err(|_| Error("invalid_config"))?;
        require(config.general_image.is_absolute(), "invalid_image_path")?;
        require(config.manager_uid > 0, "manager_must_not_be_root")?;
        require(
            config.general_mib >= 512 && config.general_mib <= 1024 * 1024,
            "invalid_general_size",
        )?;
        require(
            config.session_mib >= 128 && config.session_mib < config.general_mib,
            "invalid_session_size",
        )?;
        require(
            (65536..=1048576).contains(&config.pbkdf_memory_kib),
            "invalid_kdf_memory",
        )?;
        Ok(config)
    }

    fn initialize(&self, secret: &[u8]) -> Result<()> {
        require(
            !mapping_open("oblivion-general")? && !mapping_open("oblivion-session")?,
            "volume_busy",
        )?;
        // Only a NEW regular file can be initialized. Physical devices and existing
        // files cannot be formatted through this interface, even with a bad password.
        create_image(&self.general_image, self.general_mib)?;
        self.format(&self.general_image, secret)?;
        crypt_open(&self.general_image, "oblivion-general", secret, false)?;
        let formatted = run(
            "/usr/sbin/mkfs.ext4",
            &[
                "-q",
                "-m",
                "0",
                "-E",
                "lazy_itable_init=0,lazy_journal_init=0",
                GENERAL,
            ],
            None,
        );
        let closed = crypt_close("oblivion-general");
        formatted?;
        closed?;
        Ok(())
    }

    fn unlock(&self, secret: &[u8], readonly: bool) -> Result<()> {
        require(
            !mapping_open("oblivion-general")? && mount_mode()? == MountMode::Closed,
            "general_already_open",
        )?;
        trusted_file(&self.general_image)?;
        crypt_open(&self.general_image, "oblivion-general", secret, readonly)?;
        fs::create_dir_all(MOUNT).map_err(|_| Error("mount_directory_failed"))?;
        let result = run(
            "/usr/bin/mount",
            &[
                "-t",
                "ext4",
                "-o",
                if readonly {
                    "ro,noload,nodev,nosuid,noexec"
                } else {
                    "nodev,nosuid,noexec"
                },
                GENERAL,
                MOUNT,
            ],
            None,
        );
        if result.is_err() {
            crypt_close("oblivion-general")?;
            return result;
        }
        if readonly {
            return Ok(());
        }
        fs::set_permissions(MOUNT, fs::Permissions::from_mode(0o700))
            .map_err(|_| Error("permissions_failed"))?;
        fs::create_dir_all(format!("{MOUNT}/sessions")).map_err(|_| Error("directory_failed"))?;
        fs::create_dir_all(format!("{MOUNT}/profile")).map_err(|_| Error("directory_failed"))?;
        Ok(())
    }

    fn create(&self, secret: &[u8]) -> Result<()> {
        require(
            mount_mode()? == MountMode::ReadWrite && mapping_open("oblivion-general")?,
            "general_not_writable",
        )?;
        require(!mapping_open("oblivion-session")?, "session_already_open")?;
        let id = random_id()?;
        let path = PathBuf::from(format!("{MOUNT}/sessions/{id}.pending"));
        create_image(&path, self.session_mib)?;
        self.format(&path, secret)?;
        crypt_open(&path, "oblivion-session", secret, false)?;
        // Formatting is a block operation. The session filesystem is NEVER mounted
        // by the controller; only its VM will mount it.
        let formatted = run(
            "/usr/sbin/mkfs.ext4",
            &[
                "-q",
                "-m",
                "0",
                "-E",
                "lazy_itable_init=0,lazy_journal_init=0",
                SESSION,
            ],
            None,
        );
        let closed = crypt_close("oblivion-session");
        formatted?;
        closed?;
        trusted_file(&path)?
            .sync_all()
            .map_err(|_| Error("sync_failed"))?;
        fs::rename(&path, format!("{MOUNT}/sessions/{id}.luks"))
            .map_err(|_| Error("publish_failed"))?;
        sync_directory(Path::new(&format!("{MOUNT}/sessions")))?;
        println!("{id}");
        Ok(())
    }

    fn open(&self, id: &str, secret: &[u8]) -> Result<()> {
        validate_id(id)?;
        require(
            mount_mode()? == MountMode::ReadWrite && mapping_open("oblivion-general")?,
            "general_not_writable",
        )?;
        // Kernel mapping is the interlock, not a JSON state file that can become stale.
        require(!mapping_open("oblivion-session")?, "session_already_open")?;
        let path = PathBuf::from(format!("{MOUNT}/sessions/{id}.luks"));
        trusted_file(&path)?;
        crypt_open(&path, "oblivion-session", secret, false)
    }

    fn format(&self, path: &Path, secret: &[u8]) -> Result<()> {
        run(
            "/usr/sbin/cryptsetup",
            &[
                "luksFormat",
                "--batch-mode",
                "--type",
                "luks2",
                "--cipher",
                "aes-xts-plain64",
                "--key-size",
                "512",
                "--pbkdf",
                "argon2id",
                "--pbkdf-memory",
                &self.pbkdf_memory_kib.to_string(),
                "--pbkdf-parallel",
                "1",
                "--iter-time",
                "1000",
                "--key-file",
                "-",
                path.to_str().ok_or(Error("invalid_path"))?,
            ],
            Some(secret),
        )
    }
}

fn close_session() -> Result<()> {
    if !mapping_open("oblivion-session")? {
        return Ok(());
    }
    // No deferred removal and no lazy unmount: an attached guest/block handle must
    // cause an error, preserving the interlock until the real user is gone.
    crypt_close("oblivion-session")?;
    require(!mapping_open("oblivion-session")?, "session_still_open")
}

fn close_general() -> Result<()> {
    close_session()?;
    if mount_mode()? != MountMode::Closed {
        run("/usr/bin/umount", &[MOUNT], None)?;
    }
    if mapping_open("oblivion-general")? {
        crypt_close("oblivion-general")?;
    }
    require(
        mount_mode()? == MountMode::Closed && !mapping_open("oblivion-general")?,
        "general_still_open",
    )
}

fn crypt_open(path: &Path, name: &str, secret: &[u8], readonly: bool) -> Result<()> {
    let mut args = vec![
        "open",
        "--type",
        "luks2",
        "--disable-external-tokens",
        "--key-file",
        "-",
        path.to_str().ok_or(Error("invalid_path"))?,
        name,
    ];
    if readonly {
        args.push("--readonly");
    }
    run("/usr/sbin/cryptsetup", &args, Some(secret))
}

fn crypt_close(name: &str) -> Result<()> {
    run("/usr/sbin/cryptsetup", &["close", name], None)
}

fn run(program: &str, args: &[&str], secret: Option<&[u8]>) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C")
        .stdin(if secret.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Error("command_start_failed"))?;
    let input = match secret {
        Some(bytes) => child
            .stdin
            .take()
            .ok_or(Error("secret_pipe_failed"))?
            .write_all(bytes),
        None => Ok(()),
    };
    let status = child.wait().map_err(|_| Error("command_wait_failed"))?;
    input.map_err(|_| Error("secret_pipe_failed"))?;
    require(status.success(), "operation_failed")
}

fn secure_process() -> Result<()> {
    // These settings apply only to this process and its children, never the host OS.
    unsafe {
        require(libc::geteuid() == 0, "root_required")?;
        libc::umask(0o077);
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        require(
            libc::setrlimit(libc::RLIMIT_CORE, &limit) == 0,
            "core_limit_failed",
        )?;
        require(
            libc::prctl(libc::PR_SET_DUMPABLE, 0) == 0,
            "dump_protection_failed",
        )?;
    }
    let swaps = fs::read_to_string("/proc/swaps").map_err(|_| Error("swap_check_failed"))?;
    require(swaps.lines().count() == 1, "swap_must_be_disabled")
}

fn lock() -> Result<File> {
    lock_named("storage.lock")
}

pub(crate) fn lock_named(name: &str) -> Result<File> {
    fs::create_dir_all(RUN).map_err(|_| Error("runtime_directory_failed"))?;
    let meta = fs::symlink_metadata(RUN).map_err(|_| Error("runtime_directory_failed"))?;
    require(
        meta.is_dir() && meta.uid() == 0 && meta.mode() & 0o077 == 0,
        "unsafe_runtime_directory",
    )?;
    let runtime = File::open(RUN).map_err(|_| Error("runtime_directory_failed"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    unsafe {
        require(
            libc::fstatfs(runtime.as_raw_fd(), stats.as_mut_ptr()) == 0,
            "runtime_check_failed",
        )?;
        require(
            stats.assume_init().f_type == libc::TMPFS_MAGIC,
            "runtime_must_be_tmpfs",
        )?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(format!("{RUN}/{name}"))
        .map_err(|_| Error("lock_failed"))?;
    unsafe {
        require(
            libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0,
            "controller_busy",
        )?;
        // A killed controller must not release the interlock while cryptsetup or
        // mkfs is still running. Trusted children retain the same locked open file
        // description until they exit; never pass this FD to a session guest.
        require(
            libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) == 0,
            "lock_inheritance_failed",
        )?;
    }
    Ok(file)
}

fn trusted_file(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| Error("file_unavailable"))?;
    let meta = file.metadata().map_err(|_| Error("metadata_failed"))?;
    require(
        meta.is_file() && meta.uid() == 0 && meta.mode() & 0o077 == 0 && meta.nlink() == 1,
        "unsafe_file",
    )?;
    Ok(file)
}

fn create_image(path: &Path, mib: u64) -> Result<()> {
    let parent = path.parent().ok_or(Error("invalid_image_path"))?;
    let meta = fs::symlink_metadata(parent).map_err(|_| Error("image_parent_unavailable"))?;
    require(
        meta.is_dir() && meta.uid() == 0 && meta.mode() & 0o022 == 0,
        "unsafe_image_parent",
    )?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| Error("image_exists_or_unavailable"))?;
    file.set_len(mib * MIB)
        .map_err(|_| Error("image_size_failed"))?;
    // Reserve space up front: a sparse inner vault must not exhaust the general
    // filesystem while the guest believes it still has free disk blocks.
    unsafe {
        require(
            libc::posix_fallocate(file.as_raw_fd(), 0, (mib * MIB) as libc::off_t) == 0,
            "image_allocation_failed",
        )?;
    }
    file.sync_all().map_err(|_| Error("sync_failed"))?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error("sync_failed"))
}

fn mapping_open(name: &str) -> Result<bool> {
    // Device symlinks are managed asynchronously by userspace. Only the kernel's
    // device-mapper table can tell us that the mapping is really gone.
    for entry in fs::read_dir("/sys/block").map_err(|_| Error("mapping_check_failed"))? {
        let entry = entry.map_err(|_| Error("mapping_check_failed"))?;
        if !entry.file_name().to_string_lossy().starts_with("dm-") {
            continue;
        }
        let active = fs::read_to_string(entry.path().join("dm/name"))
            .map_err(|_| Error("mapping_check_failed"))?;
        if active.trim() == name {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(PartialEq)]
enum MountMode {
    Closed,
    ReadOnly,
    ReadWrite,
}

fn mount_mode() -> Result<MountMode> {
    let mounts =
        fs::read_to_string("/proc/self/mountinfo").map_err(|_| Error("mount_check_failed"))?;
    for line in mounts.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.get(4) == Some(&MOUNT) {
            let separator = fields
                .iter()
                .position(|field| *field == "-")
                .ok_or(Error("mount_check_failed"))?;
            require(
                fields.get(separator + 2) == Some(&GENERAL),
                "unexpected_general_mount",
            )?;
            let readonly = fields
                .get(5)
                .ok_or(Error("mount_check_failed"))?
                .split(',')
                .any(|option| option == "ro");
            return Ok(if readonly {
                MountMode::ReadOnly
            } else {
                MountMode::ReadWrite
            });
        }
    }
    Ok(MountMode::Closed)
}

fn sessions() -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for entry in
        fs::read_dir(format!("{MOUNT}/sessions")).map_err(|_| Error("session_list_failed"))?
    {
        let entry = entry.map_err(|_| Error("session_list_failed"))?;
        let name = entry.file_name();
        if let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".luks")) {
            validate_id(id)?;
            trusted_file(&entry.path())?;
            ids.push(id.to_owned());
        }
    }
    ids.sort();
    Ok(ids)
}

fn validate_id(id: &str) -> Result<()> {
    require(
        id.len() == 32
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid_session_id",
    )
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| Error("random_failed"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn password(creating: bool) -> Result<Zeroizing<Vec<u8>>> {
    // Binary stdin is a private pipe from the trusted UI, never argv, environment,
    // shell history or a temporary key file. Newline is part of the password.
    let mut bytes = Zeroizing::new(Vec::with_capacity(1025));
    std::io::stdin()
        .take(1025)
        .read_to_end(&mut bytes)
        .map_err(|_| Error("secret_read_failed"))?;
    require(
        bytes.len() <= 1024 && bytes.len() >= if creating { 16 } else { 1 },
        "invalid_password_length",
    )?;
    Ok(bytes)
}

pub(crate) fn require(condition: bool, code: &'static str) -> Result<()> {
    if condition {
        return Ok(());
    }
    Err(Error(code))
}
