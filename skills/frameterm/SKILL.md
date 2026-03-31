---
name: frameterm
description: Terminal automation CLI for AI agents with video recording. Spawn TUI apps (vim, htop, k9s, lazygit), control them via keyboard/mouse, capture screen state, and export sessions as MP4 videos. Use when the user needs to interact with terminal apps, automate TUI workflows, or record terminal sessions for review.
allowed-tools: Bash(frameterm:*)
---

# Terminal Automation with frameterm

## CRITICAL: Argument Positioning

**All flags (`--name`, `-s`, `--format`, etc.) MUST come BEFORE positional arguments:**

```bash
# CORRECT - flags before command/arguments
frameterm spawn --name myapp vim file.txt
frameterm key -s myapp Enter
frameterm snapshot -s myapp --format text

# WRONG - flags after command
frameterm spawn vim file.txt --name myapp   # FAILS: --name goes to vim
frameterm key Enter -s myapp                # FAILS
```

---

## Quick start

```bash
frameterm spawn vim file.txt        # Start TUI app in managed session
frameterm wait-for "file.txt"       # Wait for app to be ready
frameterm snapshot                  # Get screen state with UI elements
frameterm key i                     # Enter insert mode
frameterm type "Hello, World!"      # Type text
frameterm key Escape                # Exit insert mode
frameterm key ": w q Enter"         # Save and quit (key sequence)
frameterm record export             # Export session as MP4 video
frameterm kill                      # End session
```

## Core workflow

1. **Spawn**: `frameterm spawn <command>` starts the app in a background PTY
2. **Wait**: `frameterm wait-for <text>` ensures the app is ready
3. **Snapshot**: `frameterm snapshot` returns screen state with detected UI elements
4. **Interact**: Use `key`, `type`, `click`, `scroll` to navigate
5. **Re-snapshot**: Check `content_hash` to detect screen changes
6. **Record**: `frameterm record export` saves the session as MP4

## Commands

### Session management

```bash
frameterm spawn <command>              # Start TUI app
frameterm spawn --name myapp <cmd>     # Start with custom session name
frameterm spawn --cwd /path <cmd>      # Start in specific directory
frameterm spawn --cols 120 --rows 40 <cmd>  # Custom terminal size
frameterm spawn --fps 30 <cmd>         # Custom recording frame rate
frameterm spawn --no-record <cmd>      # Disable recording for this session
frameterm kill                         # Kill default session
frameterm kill -s myapp                # Kill specific session
frameterm list-sessions                # List all active sessions
frameterm stop                         # Stop daemon and all sessions
frameterm daemon                       # Manually start daemon (usually auto-starts)
```

### Screen capture

```bash
frameterm snapshot                     # Full JSON with text, elements, hash
frameterm snapshot --format compact    # JSON without text field
frameterm snapshot --format text       # Plain text with cursor indicator
frameterm snapshot -s myapp            # Snapshot specific session

# Wait for screen to change (no more guessing sleep durations)
HASH=$(frameterm snapshot | jq -r '.content_hash')
frameterm key Enter
frameterm snapshot --await-change $HASH             # Block until hash differs
frameterm snapshot --await-change $HASH --settle 100  # Wait for stability too
```

### Input

```bash
frameterm type "hello"                 # Type text at cursor
frameterm type -s myapp "text"         # Type in specific session

frameterm key Enter                    # Press Enter
frameterm key Ctrl+C                   # Send interrupt
frameterm key Escape                   # Send Escape
frameterm key Tab                      # Send Tab
frameterm key F1                       # Function key
frameterm key Alt+F                    # Alt combination
frameterm key Up                       # Arrow key
frameterm key -s myapp Ctrl+S          # Key in specific session

# Key sequences (space-separated, sent in order)
frameterm key "Ctrl+X m"              # Emacs chord
frameterm key "Escape : w q Enter"    # vim :wq sequence
frameterm key "a b c" --delay 50      # Send with 50ms between keys
```

### Mouse interaction

```bash
frameterm click 5 10                   # Click at row 5, col 10
frameterm click -s myapp 10 20         # Click in specific session
frameterm scroll up                    # Scroll up 1 line
frameterm scroll down 5                # Scroll down 5 lines
```

### Terminal control

```bash
frameterm resize 120 40                # Resize terminal
frameterm wait-for "Ready"             # Wait for text to appear (30s default)
frameterm wait-for "Loading" --not     # Wait for text to disappear
frameterm wait-for "Error" --regex     # Wait for regex pattern
frameterm wait-for "Done" --timeout 5000  # Wait with 5s timeout
```

### Recording

```bash
frameterm record export                # Export default session as MP4
frameterm record export -s myapp       # Export specific session
frameterm record export --all          # Export all sessions
frameterm record export --no-overlay   # Without keystroke overlay
frameterm record export --output /tmp  # To specific directory
frameterm record export --width 1920   # Scale to specific width
```

Sessions are recorded automatically from spawn. The MP4 includes:
- Natural terminal dimensions (scalable with `--width`)
- Anti-aliased text (JetBrains Mono + Noto Emoji)
- Full ANSI color support (Catppuccin Mocha palette)
- Real-time pacing matching actual interaction duration
- Two-row input overlay footer with keystroke badges and wait-for status
- Per-process CPU/MEM sparkline graphs

## Global options

| Option | Description |
|--------|-------------|
| `-s, --session <name>` | Target specific session (default: "default") |
| `--format <fmt>` | Snapshot format: json (default), compact, text |
| `--timeout <ms>` | Timeout for wait-for (default: 30000) |
| `--regex` | Treat wait-for pattern as regex |
| `--name <name>` | Session name for spawn command |

## Snapshot output

```json
{
  "content_hash": "a1b2c3...",
  "size": { "cols": 80, "rows": 24 },
  "cursor": { "row": 5, "col": 10, "visible": true },
  "text": "Settings:\n  [x] Notifications  [ ] Dark mode\n  [Save]  [Cancel]",
  "elements": [
    { "type": "toggle", "row": 1, "col": 2, "width": 3, "text": "[x]", "confidence": 1.0, "checked": true },
    { "type": "button", "row": 2, "col": 2, "width": 6, "text": "[Save]", "confidence": 0.9 }
  ]
}
```

## Element detection

frameterm detects interactive UI elements from terminal screen content:

| Kind | Patterns | Confidence |
|------|----------|------------|
| **toggle** | `[x]`, `[ ]`, `[*]` | 1.0 |
| **button** | `[OK]`, `[Cancel]`, `<Save>` | 0.8-0.9 |
| **input** | Cursor position, `____` underscores | 0.6-1.0 |

Elements are **read-only context**. Use **keyboard navigation** for reliable interaction:

```bash
# Understand UI
frameterm snapshot | jq '.elements'

# Navigate with keyboard (reliable)
frameterm key Tab          # Next element
frameterm key Space        # Toggle checkbox
frameterm key Enter        # Activate button
```

## Screen change detection

Use `content_hash` and `--await-change` instead of `sleep`:

```bash
# Capture baseline
HASH=$(frameterm snapshot | jq -r '.content_hash')

# Perform action
frameterm key Enter

# Wait for screen to actually change
frameterm snapshot --await-change $HASH

# For apps that render progressively (streaming AI responses, etc.)
frameterm snapshot --await-change $HASH --settle 3000 --timeout 60000
```

---

## Example: Edit file with vim

```bash
frameterm spawn --name editor vim /tmp/hello.txt
frameterm wait-for -s editor "hello.txt"
frameterm key -s editor i
frameterm type -s editor "Hello from frameterm!"
frameterm key -s editor Escape
frameterm key -s editor ": w q Enter"
frameterm record export -s editor --output /tmp
```

## Example: Navigate k9s

```bash
frameterm spawn --name k9s --cols 120 --rows 40 k9s
sleep 3
frameterm key -s k9s ":"
frameterm type -s k9s "pods"
frameterm key -s k9s Enter
sleep 2
frameterm snapshot -s k9s --format text
frameterm key -s k9s j    # Navigate down
frameterm key -s k9s j
frameterm record export -s k9s --output /tmp
frameterm kill -s k9s
```

## Example: Dialog checklist

```bash
frameterm spawn --name opts dialog --checklist "Select:" 12 50 4 \
    "notifications" "Push notifications" on \
    "darkmode" "Dark mode" off
sleep 0.5
frameterm snapshot -s opts | jq '.elements[] | select(.type == "toggle")'
frameterm key -s opts Down
frameterm key -s opts Space     # Toggle darkmode on
frameterm key -s opts Enter     # Confirm
frameterm record export -s opts
frameterm kill -s opts
```

## Example: Monitor with htop

```bash
frameterm spawn --name monitor htop
frameterm wait-for -s monitor "CPU"
frameterm snapshot -s monitor --format text
frameterm key -s monitor F9    # Kill menu
frameterm key -s monitor q     # Quit
frameterm record export -s monitor
```

---

## Sessions

Each session is isolated with its own PTY, screen buffer, child process, and recording.

```bash
# Multiple sessions
frameterm spawn --name monitor htop
frameterm spawn --name editor vim file.txt

# Target specific session
frameterm snapshot -s monitor
frameterm key -s editor Ctrl+S

# List all
frameterm list-sessions

# Kill specific
frameterm kill -s editor
```

The first session spawned without `--name` is named `default`.

## Daemon architecture

frameterm uses a background daemon for persistent sessions:

- **Auto-start**: Daemon starts on first command
- **Auto-stop**: Shuts down after 5 minutes with no sessions
- **Shared state**: Multiple CLI calls share sessions
- **Recording**: Sessions recorded from spawn to kill

## Error handling

Errors include actionable suggestions:

```json
{
  "code": "SESSION_NOT_FOUND",
  "message": "Session 'abc' not found",
  "suggestion": "Run frameterm list-sessions to see active sessions"
}
```

## Common patterns

### Wait then act

```bash
frameterm spawn my-app
frameterm wait-for "Ready"
frameterm snapshot
```

### Await screen change (preferred over sleep)

```bash
HASH=$(frameterm snapshot | jq -r '.content_hash')
frameterm key Enter
frameterm snapshot --await-change $HASH --settle 50
```

### Record a workflow for review

```bash
frameterm spawn --name demo my-app
# ... interact ...
frameterm record export -s demo --output ./recordings
open ./recordings/frameterm-demo-*.mp4
```
