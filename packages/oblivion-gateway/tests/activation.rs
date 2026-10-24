#![cfg(feature = "lab-emulation")]

use serde_json::json;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

const KEY: &str = "synthetic-provider-key-from-private-channel";
const TOKEN: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

struct Process(Child);

impl Process {
    fn finish(&mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "broker did not terminate");
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        self.0
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut stdout)
            .unwrap();
        self.0
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        assert!(!String::from_utf8_lossy(&stdout).contains(KEY));
        assert!(!String::from_utf8_lossy(&stderr).contains(KEY));
        assert!(!String::from_utf8_lossy(&stderr).contains(TOKEN));
        Output {
            status,
            stdout,
            stderr,
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn launch() -> (Process, UnixStream, SocketAddr) {
    let (control, inherited) = UnixStream::pair().unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    // Duplicate above the destination range, avoiding 3/4 aliasing in pre_exec.
    let duplicate = |fd| {
        let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 10) };
        assert!(copy >= 10);
        unsafe { OwnedFd::from_raw_fd(copy) }
    };
    let inherited = duplicate(inherited.as_raw_fd());
    let listener = duplicate(listener.as_raw_fd());
    let mut command = Command::new(env!("CARGO_BIN_EXE_oblivion-api"));
    command
        .env_clear()
        .env("HTTPS_PROXY", "http://192.0.2.1:9")
        .env("ALL_PROXY", "http://192.0.2.1:9")
        .env("NO_PROXY", "*")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(inherited.as_raw_fd(), 3) != 3 || libc::dup2(listener.as_raw_fd(), 4) != 4
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    (Process(command.spawn().unwrap()), control, address)
}

fn ready(control: &mut UnixStream, process: &mut Process) {
    let config = json!({"api_key":KEY,"session_token":TOKEN,"socks":"127.0.0.1:1"});
    writeln!(control, "{config}").unwrap();
    let mut line = String::new();
    BufReader::new(control).read_line(&mut line).unwrap();
    if line.is_empty() {
        let output = process.finish();
        panic!(
            "startup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(line, "{\"ready\":true}\n");
}

fn request(address: SocketAddr, token: &str) -> String {
    let mut socket = TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let body = r#"{"model":"abliterated-model","messages":[{"role":"user","content":"test"}],"stream":true}"#;
    write!(socket, "POST /v1/chat/completions HTTP/1.1\r\nHost: gateway\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(!response.contains(KEY));
    response
}

#[test]
fn inherited_control_grants_once_and_eof_revokes_without_logs() {
    let (mut process, mut control, address) = launch();
    ready(&mut control, &mut process);
    assert!(request(address, "invalid").starts_with("HTTP/1.1 401"));
    // NO_PROXY=* and a competing HTTPS_PROXY must not bypass the closed SOCKS
    // endpoint supplied by the controller. There is no DNS/direct fallback.
    assert!(request(address, TOKEN).starts_with("HTTP/1.1 502"));
    drop(control);
    let output = process.finish();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert!(TcpStream::connect(address).is_err());
}

#[test]
fn control_byte_and_sigterm_both_revoke_the_process() {
    for signal in [false, true] {
        let (mut process, mut control, address) = launch();
        ready(&mut control, &mut process);
        if signal {
            assert_eq!(
                unsafe { libc::kill(process.0.id() as i32, libc::SIGTERM) },
                0
            );
        } else {
            control.write_all(b"revoke").unwrap();
        }
        let output = process.finish();
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        assert!(TcpStream::connect(address).is_err());
    }
}

#[test]
fn invalid_startup_has_no_credential_or_payload_diagnostics() {
    for (field, value) in [
        ("origin", json!("https://example.com")),
        ("session_token", json!("short")),
        ("socks", json!("192.0.2.1:9050")),
        ("api_key", json!("secret\r\nInjected: header")),
    ] {
        let (mut process, mut control, _) = launch();
        let mut config = json!({"api_key":KEY,"session_token":TOKEN,"socks":"127.0.0.1:1"});
        config[field] = value;
        writeln!(control, "{config}").unwrap();
        let output = process.finish();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"oblivion-api: service_failed\n");
    }
}

#[test]
fn missing_inherited_sockets_fail_before_runtime_allocates_descriptors() {
    let output = Command::new(env!("CARGO_BIN_EXE_oblivion-api"))
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"oblivion-api: service_failed\n");
}
