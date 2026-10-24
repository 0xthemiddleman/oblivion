mod support;

use oblivion_transfer::channel::Channel;
use oblivion_transfer::endpoint::{
    Approval, OfferRequest, ReceiveRequest, read_message, write_message,
};
use oblivion_transfer::{Limits, Manifest, Receipt, Selection};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use support::Tree;

const ID: &str = "0123456789abcdef0123456789abcdef";
const NAME: &str = "selected-secret-document.txt";
const CONTENT: &[u8] = b"OBLIVION_COPY_CANARY_f81d";
const LIMITS: Limits = Limits {
    entries: 64,
    bytes: 1024 * 1024,
};

struct Endpoint {
    child: Child,
    channel: Channel,
}

impl Endpoint {
    fn start(role: &str, root: &Tree, budget: u64) -> Self {
        let (stream, peer) = UnixStream::pair().unwrap();
        let fd = peer.as_raw_fd();
        let mut command = Command::new(env!("CARGO_BIN_EXE_oblivion-copy"));
        command
            .args([role, root.0.to_str().unwrap(), &budget.to_string()])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        drop(peer);
        Self {
            child,
            channel: Channel::new(File::from(OwnedFd::from(stream)), Duration::from_secs(5))
                .unwrap(),
        }
    }

    fn finish(&mut self, success: bool) {
        assert_eq!(self.child.wait().unwrap().success(), success);
        let mut stdout = Vec::new();
        self.child
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut stdout)
            .unwrap();
        let mut stderr = Vec::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        assert!(stdout.is_empty());
        assert_eq!(
            stderr,
            if success {
                b"".as_slice()
            } else {
                b"transfer_failed\n".as_slice()
            }
        );
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn request() -> OfferRequest {
    OfferRequest {
        id: ID.into(),
        limits: LIMITS,
        paths: vec![NAME.into()],
    }
}

fn approval(manifest: &Manifest) -> Approval {
    Approval {
        id: ID.into(),
        manifest_sha256: manifest.sha256(),
    }
}

#[test]
fn compiled_endpoints_copy_only_after_approval_and_verify_the_receipt() {
    let source = Tree::new();
    let target = Tree::new();
    fs::write(source.0.join(NAME), CONTENT).unwrap();
    fs::write(source.0.join("unselected"), b"do not copy").unwrap();
    let mut sender = Endpoint::start("send", &source, 5000);
    let mut receiver = Endpoint::start("receive", &target, 5000);
    write_message(&mut sender.channel, &request()).unwrap();
    let manifest = Manifest::read(&mut sender.channel, LIMITS).unwrap();
    assert!(fs::read_dir(&target.0).unwrap().next().is_none());

    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string(format!("/proc/{}/status", sender.child.id())).unwrap();
        assert!(status.lines().any(|line| line == "NoNewPrivs:\t1"));
        let cmdline = fs::read(format!("/proc/{}/cmdline", sender.child.id())).unwrap();
        assert!(!String::from_utf8_lossy(&cmdline).contains(NAME));
        assert!(!String::from_utf8_lossy(&cmdline).contains(std::str::from_utf8(CONTENT).unwrap()));
    }

    write_message(
        &mut receiver.channel,
        &ReceiveRequest {
            approval: approval(&manifest),
            limits: LIMITS,
        },
    )
    .unwrap();
    manifest.write(&mut receiver.channel).unwrap();
    let mut ready = [0; 8];
    receiver.channel.read_exact(&mut ready).unwrap();
    assert_eq!(&ready, b"receive\n");
    write_message(&mut sender.channel, &approval(&manifest)).unwrap();
    let count = std::io::copy(
        &mut Read::by_ref(&mut sender.channel).take(manifest.bytes() + 8),
        &mut receiver.channel,
    )
    .unwrap();
    assert_eq!(count, manifest.bytes() + 8);
    let receipt: Receipt = read_message(&mut receiver.channel, 1024).unwrap();
    assert_eq!(receipt.manifest_sha256, manifest.sha256());
    assert_eq!(receipt.bytes, CONTENT.len() as u64);
    write_message(&mut sender.channel, &receipt).unwrap();
    let mut done = [0; 7];
    sender.channel.read_exact(&mut done).unwrap();
    assert_eq!(&done, b"copied\n");
    sender.finish(true);
    receiver.finish(true);
    assert_eq!(
        fs::read(target.0.join(ID).join("files").join(NAME)).unwrap(),
        CONTENT
    );
    assert!(!target.0.join(ID).join("files/unselected").exists());
}

#[test]
fn sender_does_not_release_file_bytes_for_a_different_approval() {
    let source = Tree::new();
    fs::write(source.0.join(NAME), CONTENT).unwrap();
    for wrong_id in [true, false] {
        let mut sender = Endpoint::start("send", &source, 5000);
        write_message(&mut sender.channel, &request()).unwrap();
        let manifest = Manifest::read(&mut sender.channel, LIMITS).unwrap();
        let mut selected = approval(&manifest);
        if wrong_id {
            selected.id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
        }
        if !wrong_id {
            selected.manifest_sha256 = "0".repeat(64);
        }
        write_message(&mut sender.channel, &selected).unwrap();
        let mut bytes = Vec::new();
        sender.channel.read_to_end(&mut bytes).unwrap();
        assert!(bytes.is_empty());
        sender.finish(false);
    }
}

#[test]
fn receiver_rejects_unapproved_offer_and_quota_before_creating_files() {
    let source = Tree::new();
    fs::write(source.0.join(NAME), CONTENT).unwrap();
    let selection = Selection::prepare(&source.0, &[NAME.into()], LIMITS).unwrap();
    for wrong_hash in [true, false] {
        let target = Tree::new();
        let mut receiver = Endpoint::start("receive", &target, 5000);
        let mut selected = approval(selection.manifest());
        if wrong_hash {
            selected.manifest_sha256 = "0".repeat(64);
        }
        write_message(
            &mut receiver.channel,
            &ReceiveRequest {
                approval: selected,
                limits: if wrong_hash {
                    LIMITS
                } else {
                    Limits {
                        entries: 64,
                        bytes: 1,
                    }
                },
            },
        )
        .unwrap();
        selection.manifest().write(&mut receiver.channel).unwrap();
        let mut bytes = Vec::new();
        receiver.channel.read_to_end(&mut bytes).unwrap();
        assert!(bytes.is_empty());
        receiver.finish(false);
        assert_eq!(fs::read_dir(&target.0).unwrap().count(), 0);
    }
}

#[test]
fn forged_receipt_never_reports_success_to_the_sender() {
    let source = Tree::new();
    fs::write(source.0.join(NAME), CONTENT).unwrap();
    let mut sender = Endpoint::start("send", &source, 5000);
    write_message(&mut sender.channel, &request()).unwrap();
    let manifest = Manifest::read(&mut sender.channel, LIMITS).unwrap();
    write_message(&mut sender.channel, &approval(&manifest)).unwrap();
    let mut payload = vec![0; manifest.bytes() as usize + 8];
    sender.channel.read_exact(&mut payload).unwrap();
    assert_eq!(&payload[..CONTENT.len()], CONTENT);
    write_message(
        &mut sender.channel,
        &Receipt {
            id: ID.into(),
            manifest_sha256: manifest.sha256(),
            entries: 1,
            bytes: 0,
        },
    )
    .unwrap();
    let mut done = Vec::new();
    sender.channel.read_to_end(&mut done).unwrap();
    assert!(done.is_empty());
    sender.finish(false);
}

#[test]
fn receiver_deadline_removes_partial_data_even_when_peer_stays_connected() {
    let source = Tree::new();
    let target = Tree::new();
    fs::write(source.0.join(NAME), CONTENT).unwrap();
    let selection = Selection::prepare(&source.0, &[NAME.into()], LIMITS).unwrap();
    let mut receiver = Endpoint::start("receive", &target, 500);
    write_message(
        &mut receiver.channel,
        &ReceiveRequest {
            approval: approval(selection.manifest()),
            limits: LIMITS,
        },
    )
    .unwrap();
    selection.manifest().write(&mut receiver.channel).unwrap();
    let mut ready = [0; 8];
    receiver.channel.read_exact(&mut ready).unwrap();
    receiver.channel.write_all(&CONTENT[..1]).unwrap();
    let mut result = Vec::new();
    receiver.channel.read_to_end(&mut result).unwrap();
    assert!(result.is_empty());
    receiver.finish(false);
    assert_eq!(fs::read_dir(&target.0).unwrap().count(), 0);
}

#[test]
fn oversized_control_frames_and_unknown_fields_fail_without_path_disclosure() {
    let target = Tree::new();
    let mut receiver = Endpoint::start("receive", &target, 5000);
    receiver.channel.write_all(&u32::MAX.to_be_bytes()).unwrap();
    let mut bytes = Vec::new();
    receiver.channel.read_to_end(&mut bytes).unwrap();
    receiver.finish(false);
    let mut sender = Endpoint::start("send", &target, 5000);
    let mut invalid = serde_json::to_value(request()).unwrap();
    invalid["root"] = serde_json::json!("/unapproved");
    write_message(&mut sender.channel, &invalid).unwrap();
    sender.channel.read_to_end(&mut bytes).unwrap();
    sender.finish(false);
    assert!(bytes.is_empty());
    assert_eq!(fs::read_dir(&target.0).unwrap().count(), 0);
}
