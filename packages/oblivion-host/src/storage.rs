use oblivion_host::{egress, keys, network};
use oblivion_transport::policy::{Policy, Session, Tools};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
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
const RECOVERY: &str = "/run/oblivion/recovery-required";
const MOUNT: &str = "/run/oblivion/general";
const PROFILE: &str = "/run/oblivion/general/profile";
const EPHEMERAL: &str = "/run/oblivion/ephemeral.luks";
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
    vm: Option<crate::vm::Config>,
    gateway: Option<crate::vm::Config>,
    #[serde(default)]
    gateway_network: bool,
}

#[derive(Serialize)]
struct Status {
    general_present: bool,
    general_incomplete: bool,
    general_open: bool,
    general_mounted: bool,
    general_readonly: bool,
    session_open: bool,
    sessions: Vec<String>,
    incomplete_sessions: Vec<String>,
    available_bytes: Option<u64>,
    session_bytes: u64,
    vm_state: Option<String>,
    laboratory_emulation: bool,
    recovery_required: bool,
    api_key_present: Option<bool>,
    api_keys: Option<keys::Status>,
    egress: Option<egress::Status>,
    network_policy: Option<network::Status>,
    gateway_state: Option<String>,
    gateway_vm_state: Option<String>,
    gateway_network: bool,
}

pub fn main() -> Result<()> {
    secure_process()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    require(args.len() >= 3 && args[0] == "--config", "usage")?;
    let config = Config::read(Path::new(&args[1]))?;
    if args.len() == 3 && args[2] == "serve" {
        return crate::ipc::serve(Path::new(&args[1]), config.manager_uid);
    }
    let op = args[2].as_str();
    if args.len() == 3 && op == "gateway-host-run" {
        require(!recovery_required()?, "recovery_required")?;
        require(mount_mode()? != MountMode::Closed, "general_not_open")?;
        let gateway = config
            .gateway
            .as_ref()
            .ok_or(Error("gateway_not_configured"))?;
        let (saved, selected) = transport_profiles()?;
        return crate::gateway::serve(
            gateway,
            read_api_key()?,
            selected.unwrap_or(saved).into_transport(),
            config.gateway_network,
        );
    }
    require(
        args.len()
            == if matches!(op, "open" | "enter" | "discard-session-pending") {
                4
            } else {
                3
            },
        "usage",
    )?;
    // Latch before taking the operation lock: a worker may still be completing
    // when the manager dies. Its completion must not permit another opening.
    if op == "recover" {
        runtime()?;
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(RECOVERY)
            .map_err(|_| Error("recovery_marker_failed"))?;
    }
    let _lock = lock()?;
    if matches!(
        op,
        "init-general"
            | "unlock"
            | "unlock-ephemeral"
            | "create"
            | "open"
            | "enter"
            | "enter-ephemeral"
            | "discard-general-pending"
            | "discard-session-pending"
            | "set-api-key"
            | "clear-api-key"
            | "api-keys"
            | "egress"
            | "network-policy"
    ) {
        require(!recovery_required()?, "recovery_required")?;
    }
    match op {
        "init-general" => config.initialize(&password(true)?),
        "discard-general-pending" => {
            require(
                !config
                    .general_image
                    .try_exists()
                    .map_err(|_| Error("image_check_failed"))?,
                "general_already_exists",
            )?;
            require(
                !mapping_open("oblivion-general")? && mount_mode()? == MountMode::Closed,
                "volume_busy",
            )?;
            discard_pending(&config.pending_general())
        }
        "discard-session-pending" => {
            validate_id(&args[3])?;
            require(
                mount_mode()? == MountMode::ReadWrite,
                "general_not_writable",
            )?;
            discard_pending(&PathBuf::from(format!(
                "{MOUNT}/sessions/{}.pending",
                args[3]
            )))
        }
        "unlock" | "unlock-ephemeral" => {
            config.unlock(&password(false)?, op == "unlock-ephemeral")?;
            clear_key_selection()?;
            clear_key_file("api-keys.json")?;
            clear_transport_selection()?;
            clear_network_selection()?;
            config.start_gateway()
        }
        "create" => config.create(&password(true)?),
        "set-api-key" | "clear-api-key" => {
            require(
                mount_mode()? == MountMode::ReadWrite,
                "general_not_writable",
            )?;
            let status = read_keyring()?.status(key_selection()?);
            let command = if op == "clear-api-key" {
                status.selected.map(|id| keys::Command::Remove { id })
            } else {
                let bytes = password(false)?;
                let key = Zeroizing::new(
                    std::str::from_utf8(&bytes)
                        .map_err(|_| Error("invalid_api_key"))?
                        .to_owned(),
                );
                Some(if let Some(id) = status.selected {
                    keys::Command::Replace { id, key }
                } else {
                    require(status.entries.is_empty(), "api_key_missing")?;
                    keys::Command::Add {
                        name: "Predefinita".to_owned(),
                        key,
                    }
                })
            };
            require(
                !mapping_open("oblivion-session")? && crate::vm::stopped()?,
                "session_must_be_closed",
            )?;
            if let Some(command) = command {
                config.change_keys(command)?;
            }
            Ok(())
        }
        "api-keys" => {
            let mut bytes = Zeroizing::new(Vec::new());
            std::io::stdin()
                .take(4097)
                .read_to_end(&mut bytes)
                .map_err(|_| Error("secret_read_failed"))?;
            require(bytes.len() <= 4096, "invalid_secret_length")?;
            let command =
                serde_json::from_slice(&bytes).map_err(|_| Error("invalid_key_command"))?;
            config.change_keys(command)
        }
        "egress" => {
            let mut bytes = Zeroizing::new(Vec::new());
            std::io::stdin()
                .take(egress::LIMIT as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error("secret_read_failed"))?;
            require(bytes.len() <= egress::LIMIT, "invalid_secret_length")?;
            let command =
                serde_json::from_slice(&bytes).map_err(|_| Error("invalid_transport_command"))?;
            config.change_transport(command)
        }
        "network-policy" => {
            let mut bytes = Zeroizing::new(Vec::new());
            std::io::stdin()
                .take(4097)
                .read_to_end(&mut bytes)
                .map_err(|_| Error("secret_read_failed"))?;
            require(bytes.len() <= 4096, "invalid_secret_length")?;
            let command =
                serde_json::from_slice(&bytes).map_err(|_| Error("invalid_network_command"))?;
            config.change_network_policy(command)
        }
        "open" => config.open(&args[3], &password(false)?),
        "enter" | "enter-ephemeral" => {
            let vm = config.vm.as_ref().ok_or(Error("vm_not_configured"))?;
            vm.validate()?;
            require(crate::vm::stopped()?, "vm_already_running")?;
            if config.gateway.is_some() {
                crate::gateway::ready()?;
            }
            let id = if op == "enter-ephemeral" {
                random_id()?
            } else {
                args[3].clone()
            };
            let tools = if config.gateway.is_some() {
                let (saved, selected) = policy_profiles()?;
                selected
                    .or(saved)
                    .map(|policy| Session::new(&id).map(|session| Tools { session, policy }))
                    .transpose()
                    .map_err(Error)?
            } else {
                None
            };
            if op == "enter-ephemeral" {
                config.open_ephemeral()?;
            } else {
                config.open(&id, &password(false)?)?;
            }
            // Failed startup keeps the vault interlocked until explicit leave/lock.
            let result = (|| {
                let token = if config.gateway.is_some() {
                    Some(crate::gateway::prepare()?)
                } else {
                    None
                };
                vm.start(token.as_deref().map(String::as_str), tools)
            })();
            if result.is_err() && config.gateway.is_some() {
                crate::gateway::revoke()?;
            }
            result
        }
        "close" | "leave" | "force-leave" => {
            if config.gateway.is_some() {
                crate::gateway::revoke()?;
            }
            if config.vm.is_some() && op != "close" {
                crate::vm::stop(op == "force-leave")?;
            }
            close_session()
        }
        "lock" | "recover" => {
            // Remove gateway memory and every delegated connection before the
            // session/general mappings can be reported locked.
            if config.gateway.is_some() {
                crate::gateway::stop()?;
            }
            if config.vm.is_some() {
                crate::vm::stop(false)?;
            }
            close_general()?;
            clear_key_selection()?;
            clear_key_file("api-keys.json")?;
            clear_transport_selection()?;
            clear_network_selection()?;
            if recovery_required()? {
                fs::remove_file(RECOVERY).map_err(|_| Error("recovery_clear_failed"))?;
            }
            Ok(())
        }
        "status" => {
            let mode = mount_mode()?;
            let api_keys = if mode == MountMode::Closed {
                None
            } else {
                Some(read_keyring()?.status(key_selection()?))
            };
            let status = Status {
                general_present: config
                    .general_image
                    .try_exists()
                    .map_err(|_| Error("image_check_failed"))?,
                general_incomplete: config
                    .pending_general()
                    .try_exists()
                    .map_err(|_| Error("image_check_failed"))?,
                general_open: mapping_open("oblivion-general")?,
                general_mounted: mode != MountMode::Closed,
                general_readonly: mode == MountMode::ReadOnly,
                session_open: mapping_open("oblivion-session")?,
                sessions: if mode == MountMode::ReadWrite {
                    sessions(".luks")?
                } else {
                    Vec::new()
                },
                incomplete_sessions: if mode == MountMode::ReadWrite {
                    sessions(".pending")?
                } else {
                    Vec::new()
                },
                available_bytes: if mode == MountMode::ReadWrite {
                    Some(available_bytes()?)
                } else {
                    None
                },
                session_bytes: config.session_mib * MIB,
                vm_state: if config.vm.is_some() {
                    Some(crate::vm::state()?)
                } else {
                    None
                },
                laboratory_emulation: cfg!(feature = "lab-emulation"),
                gateway_network: config.gateway_network,
                recovery_required: recovery_required()?,
                gateway_state: if config.gateway.is_some() {
                    Some(crate::gateway::state()?)
                } else {
                    None
                },
                gateway_vm_state: if config.gateway.is_some() {
                    Some(crate::vm::state_role(crate::vm::Role::Gateway)?)
                } else {
                    None
                },
                api_key_present: api_keys.as_ref().map(|keys| keys.selected.is_some()),
                api_keys,
                network_policy: if mode == MountMode::Closed {
                    None
                } else {
                    let (saved, selected) = policy_profiles()?;
                    Some(network::Status::new(saved, selected))
                },
                egress: if mode == MountMode::Closed {
                    None
                } else {
                    let (saved, selected) = transport_profiles()?;
                    Some(egress::Status {
                        saved: saved.metadata(),
                        selected: selected.as_ref().unwrap_or(&saved).metadata(),
                        temporary: selected.is_some(),
                    })
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
    fn change_network_policy(&self, command: network::Command) -> Result<()> {
        let mode = mount_mode()?;
        require(mode != MountMode::Closed, "general_not_open")?;
        require(
            !mapping_open("oblivion-session")? && crate::vm::stopped()?,
            "session_must_be_closed",
        )?;
        let (saved, selected) = policy_profiles()?;
        let mut policy = selected.or(saved).unwrap_or_default();
        let save = match command {
            network::Command::Saved {} => return clear_network_selection(),
            network::Command::Put { rule, save } => {
                require(
                    !save || mode == MountMode::ReadWrite,
                    "general_not_writable",
                )?;
                policy.put(rule).map_err(Error)?;
                save
            }
            network::Command::Remove { rule, save } => {
                require(
                    !save || mode == MountMode::ReadWrite,
                    "general_not_writable",
                )?;
                policy.remove(&rule).map_err(Error)?;
                save
            }
        };
        let path = Path::new(if save { PROFILE } else { RUN }).join(if save {
            "network-policy.json"
        } else {
            "network-policy-selection.json"
        });
        write_network_policy(&path, &policy, save)?;
        // Publish the complete saved policy before discarding a RAM selection.
        // A failed write must preserve both the previous file and selection.
        if save {
            clear_network_selection()?;
        }
        Ok(())
    }

    fn change_transport(&self, command: egress::Command) -> Result<()> {
        let mode = mount_mode()?;
        require(mode != MountMode::Closed, "general_not_open")?;
        let save = matches!(command, egress::Command::Save { .. });
        let temporary = matches!(command, egress::Command::Select { .. });
        require(
            !save || mode == MountMode::ReadWrite,
            "general_not_writable",
        )?;
        require(
            !mapping_open("oblivion-session")? && crate::vm::stopped()?,
            "session_must_be_closed",
        )?;
        let (saved, _) = transport_profiles()?;
        let profile = match command {
            egress::Command::Save { transport } | egress::Command::Select { transport } => {
                egress::Profile::new(transport).map_err(Error)?
            }
            egress::Command::Saved {} => saved,
        };
        require(
            self.gateway_network
                || matches!(profile.metadata(), oblivion_transport::Metadata::Tor {}),
            "external_proxy_requires_network",
        )?;
        // Close delegated connections before changing either stored or volatile
        // credentials. Any failed write leaves the gateway stopped.
        if self.gateway.is_some() {
            crate::gateway::stop()?;
        }
        if temporary {
            write_transport_profile(
                &Path::new(RUN).join("transport-selection.json"),
                &profile,
                false,
            )?;
        } else {
            clear_transport_selection()?;
            if save {
                write_transport_profile(
                    &Path::new(PROFILE).join("transport.json"),
                    &profile,
                    true,
                )?;
            }
        }
        self.start_gateway()
    }

    fn change_keys(&self, command: keys::Command) -> Result<()> {
        let mode = mount_mode()?;
        require(mode != MountMode::Closed, "general_not_open")?;
        let mut keyring = read_keyring()?;
        let temporary = match &command {
            keys::Command::Temporary { .. } => true,
            keys::Command::Replace { id, .. }
            | keys::Command::Rename { id, .. }
            | keys::Command::Remove { id } => keyring.temporary.key(id).is_some(),
            _ => false,
        };
        require(
            mode == MountMode::ReadWrite
                || temporary
                || matches!(command, keys::Command::Select { .. }),
            "general_not_writable",
        )?;
        require(
            !mapping_open("oblivion-session")? && crate::vm::stopped()?,
            "session_must_be_closed",
        )?;
        let before = keyring.status(key_selection()?);
        let selecting = matches!(command, keys::Command::Select { .. });
        let selection = match command {
            keys::Command::Select { id } => {
                require(keyring.key(&id).is_some(), "api_key_missing")?;
                Some(id)
            }
            keys::Command::Temporary { name, key } => {
                keyring
                    .temporary
                    .change(keys::Command::Add { name, key })
                    .map_err(Error)?;
                let id = keyring
                    .temporary
                    .status(None)
                    .entries
                    .pop()
                    .ok_or(Error("api_key_missing"))?
                    .id;
                require(keyring.saved.key(&id).is_none(), "key_id_exists")?;
                Some(id)
            }
            command => {
                if temporary {
                    keyring.temporary.change(command).map_err(Error)?;
                } else {
                    keyring.saved.change(command).map_err(Error)?;
                }
                None
            }
        };
        // Retrying an opening with the same key must not restart Tor bootstrap.
        // Still record the explicit choice, independently of the saved default.
        if let Some(id) = selection.as_deref() {
            if before.selected.as_deref() == Some(id)
                && (self.gateway.is_none()
                    || (crate::gateway::state()? == "active"
                        && crate::vm::state_role(crate::vm::Role::Gateway)? == "active"))
            {
                return write_key_selection(Some(id));
            }
        }
        // No old credential remains delegated across a profile/selection change,
        // including a failed write. Restart requires the selected key to exist.
        if self.gateway.is_some() {
            crate::gateway::stop()?;
        }
        if temporary {
            write_key_file("api-keys.json", &keyring.temporary.encode().map_err(Error)?)?;
        } else if !selecting {
            save_api_keys(&keyring.saved)?;
        }
        if let Some(id) = selection {
            write_key_selection(Some(&id))?;
        } else if before.entries.is_empty() {
            clear_key_selection()?;
        } else if before
            .selected
            .as_ref()
            .is_some_and(|id| keyring.key(id).is_none())
        {
            write_key_selection(None)?;
        }
        self.start_gateway()
    }

    fn start_gateway(&self) -> Result<()> {
        if self.gateway.is_some() && api_key_present()? {
            crate::gateway::start()?;
        }
        Ok(())
    }

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
        require(
            !self
                .general_image
                .try_exists()
                .map_err(|_| Error("image_check_failed"))?,
            "general_already_exists",
        )?;
        let path = self.pending_general();
        create_image(&path, self.general_mib)?;
        self.format(&path, secret)?;
        crypt_open(&path, "oblivion-general", secret, false)?;
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
        publish(&path, &self.general_image)
    }

    fn pending_general(&self) -> PathBuf {
        let mut path = self.general_image.as_os_str().to_owned();
        path.push(".pending");
        PathBuf::from(path)
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
        let formatted = format_session();
        let closed = crypt_close("oblivion-session");
        formatted?;
        closed?;
        publish(&path, &PathBuf::from(format!("{MOUNT}/sessions/{id}.luks")))?;
        println!("{id}");
        Ok(())
    }

    fn open_ephemeral(&self) -> Result<()> {
        require(
            mount_mode()? == MountMode::ReadOnly && mapping_open("oblivion-general")?,
            "ephemeral_boot_required",
        )?;
        require(!mapping_open("oblivion-session")?, "session_already_open")?;
        // runtime() checks tmpfs; secure_process() rejects swap. Reuse the same
        // block mapping/interlock, with a new random secret that is never saved.
        runtime()?;
        let mut secret = Zeroizing::new(vec![0; 64]);
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut secret))
            .map_err(|_| Error("random_failed"))?;
        create_image(Path::new(EPHEMERAL), self.session_mib)?;
        self.format(Path::new(EPHEMERAL), &secret)?;
        crypt_open(Path::new(EPHEMERAL), "oblivion-session", &secret, false)?;
        drop(secret);
        format_session()
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

fn profile_directory() -> Result<bool> {
    let directory = match fs::symlink_metadata(PROFILE) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(Error("profile_unavailable")),
        Ok(directory) => directory,
    };
    require(
        directory.is_dir() && directory.uid() == 0 && directory.mode() & 0o077 == 0,
        "unsafe_profile",
    )?;
    Ok(true)
}

fn read_api_keys() -> Result<keys::Archive> {
    if !profile_directory()? {
        return Ok(keys::Archive::empty());
    }
    let path = Path::new(PROFILE).join("abliteration.keys.json");
    if let Some(archive) = read_key_archive(&path)? {
        return Ok(archive);
    }
    let path = Path::new(PROFILE).join("abliteration.key");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(keys::Archive::empty());
        }
        Err(_) => return Err(Error("profile_unavailable")),
        Ok(_) => {}
    }
    let mut key = Zeroizing::new(String::new());
    trusted_file(&path)?
        .take(1025)
        .read_to_string(&mut key)
        .map_err(|_| Error("invalid_api_key"))?;
    keys::Archive::legacy(key).map_err(Error)
}

fn read_key_archive(path: &Path) -> Result<Option<keys::Archive>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error("profile_unavailable")),
        Ok(_) => {}
    }
    let mut bytes = Zeroizing::new(Vec::new());
    trusted_file(path)?
        .take(keys::LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error("profile_unavailable"))?;
    keys::Archive::decode(&bytes).map(Some).map_err(Error)
}

fn read_keyring() -> Result<keys::Keyring> {
    keys::Keyring::new(
        read_api_keys()?,
        read_key_archive(&Path::new(RUN).join("api-keys.json"))?
            .unwrap_or_else(keys::Archive::empty),
    )
    .map_err(Error)
}

fn api_key_present() -> Result<bool> {
    Ok(read_keyring()?.status(key_selection()?).selected.is_some())
}

fn read_api_key() -> Result<Zeroizing<Vec<u8>>> {
    let archive = read_keyring()?;
    let id = archive
        .status(key_selection()?)
        .selected
        .ok_or(Error("api_key_missing"))?;
    Ok(Zeroizing::new(
        archive
            .key(&id)
            .ok_or(Error("api_key_missing"))?
            .as_bytes()
            .to_vec(),
    ))
}

// Absence uses the saved default. An empty selection explicitly disables it;
// removing a selected key must never silently select another account.
fn key_selection() -> Result<Option<Option<String>>> {
    let path = Path::new(RUN).join("api-key-selection");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error("selection_unavailable")),
        Ok(_) => {}
    }
    let mut id = String::new();
    trusted_file(&path)?
        .take(33)
        .read_to_string(&mut id)
        .map_err(|_| Error("selection_unavailable"))?;
    require(
        id.is_empty() || keys::valid_id(&id),
        "invalid_key_selection",
    )?;
    Ok(Some(if id.is_empty() { None } else { Some(id) }))
}

fn clear_key_selection() -> Result<()> {
    clear_key_file("api-key-selection")
}

fn clear_key_file(name: &str) -> Result<()> {
    for name in [name.to_owned(), format!("{name}.pending")] {
        let path = Path::new(RUN).join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error("selection_unavailable")),
            Ok(_) => {}
        }
        trusted_file(&path)?;
        fs::remove_file(path).map_err(|_| Error("selection_write_failed"))?;
    }
    Ok(())
}

fn write_key_selection(id: Option<&str>) -> Result<()> {
    write_key_file("api-key-selection", id.unwrap_or("").as_bytes())
}

fn write_key_file(name: &str, bytes: &[u8]) -> Result<()> {
    let path = Path::new(RUN).join(name);
    let pending = Path::new(RUN).join(format!("{name}.pending"));
    for existing in [&path, &pending] {
        match fs::symlink_metadata(existing) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error("selection_unavailable")),
            Ok(_) => {
                trusted_file(existing)?;
            }
        }
    }
    if pending.exists() {
        fs::remove_file(&pending).map_err(|_| Error("selection_write_failed"))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&pending)
        .map_err(|_| Error("selection_write_failed"))?;
    file.write_all(bytes)
        .map_err(|_| Error("selection_write_failed"))?;
    fs::rename(pending, path).map_err(|_| Error("selection_write_failed"))
}

fn transport_profiles() -> Result<(egress::Profile, Option<egress::Profile>)> {
    let saved = if profile_directory()? {
        read_transport_profile(&Path::new(PROFILE).join("transport.json"))?.unwrap_or_default()
    } else {
        egress::Profile::default()
    };
    // A corrupt saved profile is never hidden by a volatile override.
    Ok((
        saved,
        read_transport_profile(&Path::new(RUN).join("transport-selection.json"))?,
    ))
}

fn policy_profiles() -> Result<(Option<Policy>, Option<Policy>)> {
    let saved = if profile_directory()? {
        read_network_policy(&Path::new(PROFILE).join("network-policy.json"))?
    } else {
        None
    };
    Ok((
        saved,
        read_network_policy(&Path::new(RUN).join("network-policy-selection.json"))?,
    ))
}

fn read_network_policy(path: &Path) -> Result<Option<Policy>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error("network_policy_unavailable")),
        Ok(_) => {}
    }
    let mut bytes = Zeroizing::new(Vec::new());
    trusted_file(path)?
        .take(network::LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error("network_policy_unavailable"))?;
    require(bytes.len() <= network::LIMIT, "invalid_network_policy")?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| Error("invalid_network_policy"))
}

fn clear_network_selection() -> Result<()> {
    for name in [
        "network-policy-selection.json",
        "network-policy-selection.json.pending",
    ] {
        let path = Path::new(RUN).join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error("selection_unavailable")),
            Ok(_) => {}
        }
        trusted_file(&path)?;
        fs::remove_file(path).map_err(|_| Error("selection_write_failed"))?;
    }
    Ok(())
}

fn write_network_policy(path: &Path, policy: &Policy, durable: bool) -> Result<()> {
    let pending = path.with_extension("json.pending");
    for existing in [path, &pending] {
        match fs::symlink_metadata(existing) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error("network_policy_unavailable")),
            Ok(_) => {
                trusted_file(existing)?;
            }
        }
    }
    let bytes = Zeroizing::new(
        serde_json::to_vec(policy).map_err(|_| Error("network_policy_encode_failed"))?,
    );
    require(bytes.len() <= network::LIMIT, "invalid_network_policy")?;
    if pending.exists() {
        fs::remove_file(&pending).map_err(|_| Error("network_policy_write_failed"))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&pending)
        .map_err(|_| Error("network_policy_write_failed"))?;
    file.write_all(&bytes)
        .map_err(|_| Error("network_policy_write_failed"))?;
    if durable {
        file.sync_all()
            .map_err(|_| Error("network_policy_write_failed"))?;
    }
    fs::rename(pending, path).map_err(|_| Error("network_policy_write_failed"))?;
    if durable {
        sync_directory(path.parent().ok_or(Error("invalid_path"))?)?;
    }
    Ok(())
}

fn read_transport_profile(path: &Path) -> Result<Option<egress::Profile>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error("transport_profile_unavailable")),
        Ok(_) => {}
    }
    let mut bytes = Zeroizing::new(Vec::new());
    trusted_file(path)?
        .take(egress::LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error("transport_profile_unavailable"))?;
    Ok(Some(egress::Profile::decode(&bytes).map_err(Error)?))
}

fn clear_transport_selection() -> Result<()> {
    for name in [
        "transport-selection.json",
        "transport-selection.json.pending",
    ] {
        let path = Path::new(RUN).join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error("selection_unavailable")),
            Ok(_) => {}
        }
        trusted_file(&path)?;
        fs::remove_file(path).map_err(|_| Error("selection_write_failed"))?;
    }
    Ok(())
}

fn write_transport_profile(path: &Path, profile: &egress::Profile, durable: bool) -> Result<()> {
    let pending = path.with_extension("json.pending");
    for existing in [path, &pending] {
        match fs::symlink_metadata(existing) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error("transport_profile_unavailable")),
            Ok(_) => {
                trusted_file(existing)?;
            }
        }
    }
    if pending.exists() {
        fs::remove_file(&pending).map_err(|_| Error("transport_write_failed"))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&pending)
        .map_err(|_| Error("transport_write_failed"))?;
    file.write_all(&profile.encode().map_err(Error)?)
        .map_err(|_| Error("transport_write_failed"))?;
    if durable {
        file.sync_all()
            .map_err(|_| Error("transport_write_failed"))?;
    }
    fs::rename(pending, path).map_err(|_| Error("transport_write_failed"))?;
    if durable {
        sync_directory(path.parent().ok_or(Error("invalid_path"))?)?;
    }
    Ok(())
}

pub(crate) fn tor_state() -> Result<Option<oblivion_tor_state::State>> {
    let path = Path::new(PROFILE).join("tor-state");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error("profile_unavailable")),
        Ok(_) => {}
    }
    let mut text = String::new();
    trusted_file(&path)?
        .take(oblivion_tor_state::LIMIT as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| Error("profile_unavailable"))?;
    Ok(Some(
        oblivion_tor_state::State::parse(&text).map_err(|_| Error("invalid_tor_state"))?,
    ))
}

pub(crate) fn tor_state_writable() -> Result<bool> {
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    require(
        unsafe { libc::statvfs(c"/run/oblivion/general/profile".as_ptr(), &mut stat) } == 0,
        "profile_unavailable",
    )?;
    Ok(stat.f_flag & libc::ST_RDONLY == 0)
}

pub(crate) fn save_tor_state(state: &oblivion_tor_state::State) -> Result<()> {
    require(tor_state_writable()?, "general_not_writable")?;
    let path = Path::new(PROFILE).join("tor-state");
    if path
        .try_exists()
        .map_err(|_| Error("profile_unavailable"))?
    {
        trusted_file(&path)?;
    }
    let pending = Path::new(PROFILE).join("tor-state.pending");
    match fs::symlink_metadata(&pending) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(Error("profile_unavailable")),
        Ok(_) => {
            trusted_file(&pending)?;
            fs::remove_file(&pending).map_err(|_| Error("profile_write_failed"))?;
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&pending)
        .map_err(|_| Error("profile_write_failed"))?;
    file.write_all(state.as_str().as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|_| Error("profile_write_failed"))?;
    fs::rename(pending, path).map_err(|_| Error("profile_write_failed"))?;
    sync_directory(Path::new(PROFILE))
}

fn save_api_keys(archive: &keys::Archive) -> Result<()> {
    let directory = Path::new(PROFILE);
    let path = directory.join("abliteration.keys.json");
    let pending = directory.join("abliteration.keys.json.pending");
    let mut legacy = Vec::new();
    for name in ["abliteration.key", "abliteration.key.pending"] {
        let file = directory.join(name);
        match fs::symlink_metadata(&file) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error("profile_unavailable")),
            Ok(_) => {
                trusted_file(&file)?;
                legacy.push(file);
            }
        }
    }
    match fs::symlink_metadata(&pending) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(Error("profile_unavailable")),
        Ok(_) => {
            trusted_file(&pending)?;
            fs::remove_file(&pending).map_err(|_| Error("profile_write_failed"))?;
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&pending)
        .map_err(|_| Error("profile_write_failed"))?;
    file.write_all(&archive.encode().map_err(Error)?)
        .and_then(|()| file.sync_all())
        .map_err(|_| Error("profile_write_failed"))?;
    fs::rename(pending, path).map_err(|_| Error("profile_write_failed"))?;
    sync_directory(directory)?;
    // The versioned archive is authoritative even if cleanup is interrupted.
    // Publish and fsync it before removing any legacy credential.
    for file in legacy {
        fs::remove_file(file).map_err(|_| Error("profile_write_failed"))?;
    }
    sync_directory(directory)
}

fn close_session() -> Result<()> {
    if mapping_open("oblivion-session")? {
        // No deferred removal: a live guest/block handle keeps the interlock.
        crypt_close("oblivion-session")?;
    }
    require(!mapping_open("oblivion-session")?, "session_still_open")?;
    if Path::new(EPHEMERAL)
        .try_exists()
        .map_err(|_| Error("ephemeral_check_failed"))?
    {
        trusted_file(Path::new(EPHEMERAL))?;
        fs::remove_file(EPHEMERAL).map_err(|_| Error("ephemeral_cleanup_failed"))?;
    }
    Ok(())
}

fn format_session() -> Result<()> {
    // The host formats the block device but never mounts a session filesystem.
    run(
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
    )
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
    runtime()?;
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
        // Trusted workers inherit the lock so a killed parent cannot release it
        // while cryptsetup or mkfs is still modifying a volume.
        require(
            libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) == 0,
            "lock_inheritance_failed",
        )?;
    }
    Ok(file)
}

fn recovery_required() -> Result<bool> {
    Path::new(RECOVERY)
        .try_exists()
        .map_err(|_| Error("recovery_check_failed"))
}

fn runtime() -> Result<()> {
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
    Ok(())
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
    // posix_fallocate returns an error number directly; it does not set errno.
    match unsafe { libc::posix_fallocate(file.as_raw_fd(), 0, (mib * MIB) as libc::off_t) } {
        0 => {}
        libc::ENOSPC | libc::EDQUOT => return Err(Error("insufficient_space")),
        _ => return Err(Error("image_allocation_failed")),
    }
    file.sync_all().map_err(|_| Error("sync_failed"))?;
    sync_directory(parent)
}

fn publish(pending: &Path, destination: &Path) -> Result<()> {
    trusted_file(pending)?
        .sync_all()
        .map_err(|_| Error("sync_failed"))?;
    let source =
        CString::new(pending.as_os_str().as_encoded_bytes()).map_err(|_| Error("invalid_path"))?;
    let target = CString::new(destination.as_os_str().as_encoded_bytes())
        .map_err(|_| Error("invalid_path"))?;
    // Publication is atomic and must never replace an existing archive.
    require(
        unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                target.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } == 0,
        "publish_failed",
    )?;
    sync_directory(destination.parent().ok_or(Error("invalid_path"))?)
}

fn discard_pending(path: &Path) -> Result<()> {
    require(!mapping_open("oblivion-session")?, "volume_busy")?;
    trusted_file(path)?;
    fs::remove_file(path).map_err(|_| Error("discard_failed"))?;
    sync_directory(path.parent().ok_or(Error("invalid_path"))?)
}

fn available_bytes() -> Result<u64> {
    let file = File::open(MOUNT).map_err(|_| Error("space_check_failed"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    require(
        unsafe { libc::fstatvfs(file.as_raw_fd(), stats.as_mut_ptr()) } == 0,
        "space_check_failed",
    )?;
    let stats = unsafe { stats.assume_init() };
    Ok(stats.f_bavail.saturating_mul(stats.f_frsize))
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

fn sessions(extension: &str) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for entry in
        fs::read_dir(format!("{MOUNT}/sessions")).map_err(|_| Error("session_list_failed"))?
    {
        let entry = entry.map_err(|_| Error("session_list_failed"))?;
        let name = entry.file_name();
        if let Some(id) = name.to_str().and_then(|name| name.strip_suffix(extension)) {
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
