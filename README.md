# frameterm

Get video proof of what your agents did in TUIs.

Here's [opencode](https://github.com/sst/opencode) driven by frameterm — spawned, waited on, typed into, and recorded as video:

![demo](https://github.com/stefanmatar/frameterm/raw/main/.github/assets/demo.gif)

The footer is burned into the exported MP4 automatically:
- Keystroke overlay showing what was typed, like [KeyCastr](https://github.com/keycastr/keycastr)
- Live CPU and memory sparklines for the running process
- Wait-for badge visible while frameterm blocks on a screen state

## Use cases

### Let your agent drive k9s

Your AI agent navigates Kubernetes, and you get a video of exactly what it did.

```bash
frameterm spawn --name k9s --cols 120 --rows 40 k9s
frameterm wait-for -s k9s "Context:"          # wait for cluster connection
frameterm key -s k9s ":"
frameterm type -s k9s "pods"
frameterm key -s k9s Enter
frameterm wait-for -s k9s "NAME"              # wait for pods to load
frameterm snapshot -s k9s --format text        # read what's on screen
frameterm key -s k9s Down                      # navigate
frameterm key -s k9s Down
frameterm key -s k9s Enter                     # describe a pod
frameterm record export -s k9s --output .      # get the video
```

### Let your agent test your TUI

Point frameterm at your own app. Wait for it to start. Drive it. Record the result.

```bash
frameterm spawn --name app ./my-tui-app
frameterm wait-for -s app "Ready"              # wait for your app to boot
frameterm key -s app Tab                        # navigate
frameterm key -s app Space                      # toggle
frameterm key -s app Enter                      # confirm
frameterm snapshot -s app                       # check screen state as JSON
frameterm record export -s app --output .       # export the test as video
```

The exported MP4 has anti-aliased text (JetBrains Mono + Noto Emoji), full ANSI colors, and a footer showing every keystroke, wait state, and resource usage.

## Install

```bash
brew install stefanmatar/tap/frameterm
```

Or build from source:

```bash
cargo install --path crates/frameterm
```

Requires `ffmpeg` for video export.

## CLI

### Sessions

```bash
frameterm spawn <command>                # Start in background PTY
frameterm spawn --name myapp <cmd>       # Named session
frameterm spawn --cols 120 --rows 40 <cmd>  # Custom terminal size
frameterm spawn --fps 30 <cmd>           # Custom recording FPS
frameterm spawn --no-record <cmd>        # No recording
frameterm kill                           # Kill default session
frameterm kill -s myapp                  # Kill named session
frameterm list-sessions                  # List active sessions
frameterm stop                           # Stop daemon and all sessions
```

### Screen

```bash
frameterm snapshot                       # JSON with text and content hash
frameterm snapshot --format text          # Plain text
frameterm snapshot --format compact       # JSON without text field
frameterm snapshot -s myapp              # Specific session
```

### Input

```bash
frameterm type "hello"                   # Type text
frameterm key Enter                      # Single key
frameterm key Ctrl+C                     # Key combo
frameterm key Up                         # Arrow keys
frameterm key "Escape : w q Enter"       # Key sequence
frameterm click 10 5                     # Click at row, col
frameterm scroll down 5                  # Scroll
```

### Waiting

```bash
frameterm wait-for "Ready"               # Block until text appears
frameterm wait-for "Loading" --not       # Block until text disappears
frameterm wait-for "Error" --regex       # Regex pattern
frameterm wait-for "Done" --timeout 5000 # Custom timeout

# Wait for screen to change (no more guessing sleep durations)
HASH=$(frameterm snapshot | jq -r '.content_hash')
frameterm key Enter
frameterm snapshot --await-change $HASH
```

### Recording

```bash
frameterm record export                  # Export as MP4
frameterm record export -s myapp         # Specific session
frameterm record export --all            # All sessions
frameterm record export --no-overlay     # Without input overlay
frameterm record export --output /tmp    # To directory
frameterm record export --width 1920     # Scale to width
```

## How it works

frameterm runs a background daemon that manages PTY sessions. Each CLI call communicates with the daemon over a Unix socket, so sessions persist between commands.

```
CLI ──── Unix socket ──── Daemon
                            ├── Session (k9s)    [PTY + vt100 + recording]
                            ├── Session (vim)     [PTY + vt100 + recording]
                            └── Session (htop)    [PTY + vt100 + recording]
```

## Platforms

| Platform | Status |
|----------|--------|
| macOS (arm64, x86_64) | Supported |
| Linux (x86_64, arm64) | Supported |
| Windows | Not supported |

## Development

```bash
devbox shell          # Rust, ffmpeg, lefthook
cargo build           # Build
cargo test            # Run tests
cargo clippy          # Lint
cargo fmt             # Format
```

## License

MIT
