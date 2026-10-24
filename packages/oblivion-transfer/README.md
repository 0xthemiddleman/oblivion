# Oblivion file copy engine

Bounded offers and streaming copies of explicitly selected regular files and
directories. Receives into a new private transaction, verifies SHA-256, syncs
before publication, and never overwrites an existing transaction. No network,
USB mounting, shell execution, logging or archive extraction in this library.

The `oblivion-copy` endpoint uses an inherited private descriptor, a monotonic
transport deadline, approval bound to the offered manifest and a verified
receipt. Separate send/receive processes have been exercised through VM
virtio-serial ports with a readonly source and LUKS destination.

This is not the completed USB integration. The controller still owns
authorization, isolated source/destination environments, storage placement,
revocation, filesystem-stall recovery and release to the chat.

See [the protocol and integration boundary](../../docs/oblivion/FILE-COPY.md).

From this directory:

```sh
cargo test --locked
cargo clippy --all-targets -- -D warnings
```

The repository's `script/oblivion/Dockerfile.transfer` runs the same tests in
Debian without network access during test execution. Its CI jobs target ARM64
and x86-64. All test files and names are synthetic.
