# Oblivion host controller

Linux-only Rust component for the trusted live system. This is an implementation
prototype with systemd units, not an installed or finished live distribution. It delegates
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
- Systemd supervises the non-root manager and root controller. Failed recovery
  latches a root-owned flag in RAM, which blocks creation and opening even across
  service restarts. Only verified closure of both volumes clears it.
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

| Operation          | Result                                                                                                                                          |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `init-general`     | Creates a new regular general-image file and returns with it locked                                                                             |
| `unlock`           | Opens/mounts the general and prepares profile/session directories                                                                               |
| `unlock-ephemeral` | Opens the existing general read-only, without journal replay                                                                                    |
| `create`           | Creates a session, prints its random opaque ID, leaves it locked                                                                                |
| `open <id>`        | Internal root-only storage primitive; opens only the block device                                                                               |
| `close`            | Internal root-only primitive; closes the mapping or fails if busy                                                                               |
| `enter <id>`       | Requires KVM, opens the vault and starts its confined QEMU service                                                                              |
| `leave`            | Requests guest shutdown, waits for QEMU termination, then closes the mapping                                                                    |
| `force-leave`      | Explicit forced QEMU stop, then closes the mapping; unsaved data may be lost                                                                    |
| `lock`             | Shuts down the guest, closes its mapping, unmounts and closes the general                                                                       |
| `recover`          | Root service operation; latches recovery before the operation lock, attempts ordinary shutdown and clears the latch only after verified closure |
| `status`           | Returns kernel-derived state and opaque IDs, no conversation titles                                                                             |
| `serve`            | Serves the restricted Unix control socket for the configured manager UID                                                                        |

Secrets are binary stdin (1–1024 bytes; creation requires at least 16). A newline
is part of a secret. Do not type real passphrases into shell command lines.
`oblivion-manager` now supplies a hidden password prompt outside the guest.
Its native Mac `--preview` mode never connects to storage or asks for a password.
The included lab uses only fixed, public synthetic passphrases.
The general image belongs on persistent storage (`/persist` in the example),
not the live system's RAM-backed `/var`.

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

The manager protocol's `open` invokes `enter`; `close` invokes `leave` and
`force-close` invokes `force-leave`. The manager cannot request the mapping-only
root primitives. Configuration without a VM remains usable for isolated storage
tests but rejects a manager session open.

The new `console` IPC operation accepts no secret or session argument and delegates
one connected serial socket using `SCM_RIGHTS`, followed by the ordinary JSON
success response. It never delegates QMP or arbitrary paths. The manager checks
that the controller's peer UID is root. See the [manager laboratory](../../docs/oblivion/MANAGER.md).
`recover` is deliberately absent from the manager IPC vocabulary; it is invoked
by trusted root services. The manager can retry `lock` or explicitly force-close
the session before locking the general. `status` includes `recovery_required`.

Service ordering, dedicated accounts, console behavior and Mac reproduction are
documented in [host services](../../docs/oblivion/HOST-SERVICES.md). Units must be
integrated into the live image; do not install their fixed UIDs on a normal
multiuser workstation. The privileged controller retains host-visible mounts.

## VM boundary

`vm` configuration selects root-owned public images, memory and CPUs, not an
arbitrary executable or command line. Normal builds require a usable `/dev/kvm`
before unlocking a session. There is no software-emulation fallback. The explicit
`lab-emulation` Cargo feature is for synthetic tests only and is reported in
`status`; never ship that binary as a privacy product.

QEMU runs as UID/GID 1100 in a transient systemd unit, without capabilities and
with a private device tree, filesystem root and network namespace. Its root holds
only executables and libraries. Only the public boot images and active session
block device are bound into it. No host or session directory is shared with the
guest; the guest receives a readonly system disk and its own writable vault disk.
This first prototype has **no network device**. QMP and console use private Unix
sockets in RAM; the first trusted console bridge renders guest cells below a
reserved host header and filters terminal control sequences. The initial
interactive TUI cycle is tested; extended paste and resize remain open gates.

The guest mounts its vault at `/vault`; the conversation runs as UID/GID 1000.
Home, workspace, state and configuration belong to that vault. `/tmp` and `/var`
are tmpfs; the system root is SquashFS. Journald storage, swap, hibernation and
core dumps are disabled. Closing waits for the guest to power off; a timeout
returns an error and leaves the volume interlocked, never reporting a false lock.
Forced close is a separate explicit request. No memory snapshots are made.

## Verification and remaining work

See [storage laboratory](../../docs/oblivion/STORAGE-LAB.md) and
[VM lifecycle laboratory](../../docs/oblivion/VM-LAB.md). Cargo dependencies
are locked; build with `cargo build --locked --release`. Integration tests execute
the compiled Linux binaries with real cryptsetup and real device-mapper mappings
inside a disposable Debian VM, not mocks of encryption or process execution.

Still required: full live-image integration, KVM/hardware validation,
guest TUI refinement, complete boot/shutdown lifecycle, quota UI, explicit recovery for interrupted initialization
and `.pending` session files, power-loss tests, profile settings/API broker,
full ephemeral guests, USB provisioning, integrity/backup decisions and release
hardening. A failed initialization retains its file and a failed creation may
retain a `.pending` file; neither is silently reused or overwritten.
