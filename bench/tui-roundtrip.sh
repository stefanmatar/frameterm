#!/bin/bash
# Benchmark: realistic TUI interaction roundtrip via frameterm CLI.
#
# Measures the total latency of a typical e2e test flow:
#   spawn → wait-for prompt → type text → send keys → wait-for output →
#   snapshot → more typing → wait → snapshot → kill
#
# Usage:
#   hyperfine --warmup 2 --runs 10 './bench/tui-roundtrip.sh'

set -euo pipefail

SESSION="bench-$$-$RANDOM"

cleanup() { frameterm kill -s "$SESSION" 2>/dev/null || true; }
trap cleanup EXIT

# 1. Spawn a bash session
frameterm spawn --name "$SESSION" --no-record --cols 80 --rows 24 bash >/dev/null

# 2. Wait for the shell prompt
frameterm wait-for -s "$SESSION" --timeout 5000 '$' >/dev/null

# 3. Type a command and execute it
frameterm type -s "$SESSION" 'echo hello-from-benchmark' >/dev/null
frameterm key -s "$SESSION" Enter >/dev/null

# 4. Wait for the output
frameterm wait-for -s "$SESSION" --timeout 5000 'hello-from-benchmark' >/dev/null

# 5. Take a snapshot (compact)
frameterm snapshot -s "$SESSION" --format compact >/dev/null

# 6. Send individual keys (simulates TUI navigation — arrow keys, Escape, etc.)
frameterm key -s "$SESSION" Up >/dev/null
frameterm key -s "$SESSION" Down >/dev/null
frameterm key -s "$SESSION" Escape >/dev/null
frameterm key -s "$SESSION" 'Ctrl+C' >/dev/null

# 7. Wait for prompt again after Ctrl+C
frameterm wait-for -s "$SESSION" --timeout 5000 '$' >/dev/null

# 8. Type another command and execute
frameterm type -s "$SESSION" 'echo roundtrip-done' >/dev/null
frameterm key -s "$SESSION" Enter >/dev/null

# 9. Wait for final output
frameterm wait-for -s "$SESSION" --timeout 5000 'roundtrip-done' >/dev/null

# 10. Final snapshot (text format)
frameterm snapshot -s "$SESSION" --format text >/dev/null

# 11. Kill
frameterm kill -s "$SESSION" >/dev/null
trap - EXIT
