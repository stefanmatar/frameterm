# Changelog

## v1.3.1

### Fixed

- **Homebrew tap sync**: Rewrote the release workflow's tap sync step to use
  `git clone` + `git push` instead of the GitHub Contents API, which was
  silently failing with HTTP 403. The step now emits a visible warning when
  `HOMEBREW_TAP_TOKEN` is missing instead of succeeding silently.

## v1.3.0

### Added

- **Multiplexed pipe mode**: Requests sent through `frameterm pipe` with an `id`
  field are now dispatched concurrently. The daemon echoes the `id` back in the
  response so clients can match responses to in-flight requests. Requests without
  an `id` are still processed serially (fully backward compatible).
- **Settle parameter for `snapshot --await-change`**: The `--settle` flag (and
  `settle` JSON field) now works — after the initial screen change is detected,
  frameterm continues polling until the screen stops changing for the specified
  duration. This lets callers wait for multi-frame transitions to stabilize.
- **`FRAMETERM_SOCKET_DIR` environment variable**: Point the daemon and all
  clients at an isolated socket directory. Useful for running multiple daemon
  instances in parallel (e.g. per-test-suite isolation in CI).
- **Benchmark scripts** (`bench/`): `tui-roundtrip.sh` (CLI) and
  `tui-roundtrip-pipe.sh` (pipe) for measuring per-command latency.
- **CI**: SHA-pinned job in the GitHub Action smoke test workflow.

### Changed

- **Daemon performance**: Removed double-locking in `SessionManager` —
  `Arc<Mutex<SessionManager>>` replaced with `Arc<SessionManager>` (interior
  mutability). All public methods changed from `&mut self` to `&self`.
- **Pipe mode is now fully concurrent**: The pipe relay uses two threads
  (stdin-to-daemon writer, daemon-to-stdout reader) so multiple requests can be
  in flight simultaneously.
- **Reduced latencies**: Spawn sleep 100 ms to 10 ms, poll intervals 50 ms to
  10 ms, removed unnecessary 50 ms sleep after `type_text`.
- **Robust daemon probe**: `ensure_daemon()` sends a ping/pong handshake instead
  of a bare `connect()`, eliminating a race with the daemon's accept loop.

### Fixed

- **`send_key` race condition**: Key parsing now happens outside the session
  lock. A single lock acquisition covers the existence check, PTY write, and
  recording — no gap where another thread could interleave.
- **Pipe exit on EOF**: Explicit `shutdown(Write)` on the daemon socket after
  stdin EOF so the reader thread drains remaining responses and the process exits
  cleanly instead of hanging.
- **120-second socket timeout removed from pipe mode**: Long-running commands
  like `wait-for` could exceed the timeout and silently kill the connection.

## v1.2.3

- GitHub Action for installing frameterm in CI (`stefanmatar/frameterm@v1`).
- SHA-256 checksum verification in the action before extracting tarballs.
- Homebrew formula synced to the `homebrew-frameterm` tap on each release.

## v1.2.2

- Initial public release.
