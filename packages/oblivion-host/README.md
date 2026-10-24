# Oblivion host controller

Linux-only Rust component for the trusted live system. This is an implementation
prototype, not an installed service or a finished live distribution. It delegates
cryptography to cryptsetup/LUKS2; it does not implement a cryptographic format.
It has no dependency on the OpenCode server, model or tool execution environment.

## Implemented boundary

- A general LUKS2 file contains an administrative profile and independent session
  LUKS2 files. Creation allocates space up front, uses Argon2id and never overwrites
  an existing file. Physical-device initialization is deliberately absent.
- The general filesystem is mounted only by the privileged controller, with
  `nodev,nosuid,noexec` and private permissions. The controller formats session
  filesystems but **never mounts them**.
- One fixed session device-mapper name and an exclusive operation lock serialize
  all opens. The lock survives controller death while a trusted child is running.
  Status is reconstructed from kernel mappings and mount state, not saved JSON.
- Close uses ordinary cryptsetup removal and ordinary unmount. Busy handles cause
  failure; there is no deferred removal, lazy unmount or false “locked” state.
- Ephemeral-profile unlock opens both device and filesystem read-only and disables
  ext4 journal replay. Existing session opens and creation are rejected in this
  mode. A RAM-backed ephemeral guest is still to be implemented.
- Password bytes travel through private stdin/socket channels, never argv, env or
  temporary key files. Owned buffers are zeroized; core dumps are disabled. This
  is not a claim that every kernel or library memory copy is erased immediately.
- The storage process requires root, disabled swap and a tmpfs runtime directory.
  The manager is a separate non-root UID, without access to the general filesystem.

`oblivion-storage --config /etc/oblivion/controller.json <operation>` is the
privileged internal interface. Configuration must be a root-owned regular file,
mode `0600`, without hard links; see [example](controller.example.json). Image
parents must be root-owned and not writable by other users. The configured paths
and their ancestors belong to the trusted system, never to an imported document.

| Operation | Result |
| --- | --- |
| `init-general` | Creates a new regular general-image file and returns with it locked |
| `unlock` | Opens/mounts the general and prepares profile/session directories |
| `unlock-ephemeral` | Opens the existing general read-only, without journal replay |
| `create` | Creates a session, prints its random opaque ID, leaves it locked |
| `open <id>` | Opens the one session block device; **does not launch a VM yet** |
| `close` | Closes the session mapping, or fails if it is still in use |
| `lock` | Closes the session, unmounts the general, closes the general mapping |
| `status` | Returns kernel-derived state and opaque IDs, no conversation titles |
| `serve` | Serves the restricted Unix control socket for the configured manager UID |

Secrets are binary stdin (1–1024 bytes; creation requires at least 16). A newline
is part of a secret. Do not type real passphrases into shell command lines.
The trusted TUI password prompt is not implemented yet. The included lab uses
only fixed, public synthetic passphrases.

## Local IPC

`/run/oblivion-control.sock` is mode `0600`, owned by `manager_uid` (nonzero).
The daemon additionally checks Linux `SO_PEERCRED` before reading any request.
Only the enumerated storage operations are accepted; paths, executable names,
mount options and shell commands cannot be supplied by a client.

Each connection carries exactly one request: a 4-byte big-endian header length
(1–1024), a JSON header with `operation`, optional `session` and `secret_len`, then
exactly that many secret bytes (maximum 1024). Unknown fields are rejected. The
JSON response contains `ok`, `output` and an optional static error code. Requests
and responses are not written to a log. The socket must never be exposed to a
session VM or forwarded over a network.

`oblivion-control` is a non-root command client for this protocol. It is a building
block for the manager, not a shell privilege escalation facility. The daemon
inherits its own root-owned config path and executes only its own storage binary.
Each storage command reacquires the independent operation lock.

## Verification and remaining work

See [storage laboratory](../../docs/oblivion/STORAGE-LAB.md). Cargo dependencies
are locked; build with `cargo build --locked --release`. Integration tests execute
the compiled Linux binaries with real cryptsetup and real device-mapper mappings
inside a disposable Debian VM, not mocks of encryption or process execution.

Still required: production service confinement, VM lifecycle and block-device
handoff, trusted TUI, quota UI, explicit recovery for interrupted initialization
and `.pending` session files, power-loss tests, profile settings/API broker,
full ephemeral guests, USB provisioning, integrity/backup decisions and release
hardening. A failed initialization retains its file and a failed creation may
retain a `.pending` file; neither is silently reused or overwritten.
