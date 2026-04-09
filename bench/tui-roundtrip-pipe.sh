#!/bin/bash
# Benchmark: same TUI roundtrip but using pipe mode (single process, single connection).
#
# Usage:
#   hyperfine --warmup 2 --runs 10 './bench/tui-roundtrip-pipe.sh'

set -euo pipefail

SESSION="bench-$$-$RANDOM"

frameterm pipe <<EOF
{"command":"spawn","name":"$SESSION","cmd":"bash","args":[],"cwd":null,"cols":80,"rows":24,"fps":null,"no_record":true}
{"command":"wait_for","session":"$SESSION","pattern":"$","regex":false,"not":false,"timeout":5000}
{"command":"type","session":"$SESSION","text":"echo hello-from-benchmark"}
{"command":"key","session":"$SESSION","keys":"Enter","delay":null}
{"command":"wait_for","session":"$SESSION","pattern":"hello-from-benchmark","regex":false,"not":false,"timeout":5000}
{"command":"snapshot","session":"$SESSION","format":"compact","await_change":null,"settle":null,"timeout":null}
{"command":"key","session":"$SESSION","keys":"Up","delay":null}
{"command":"key","session":"$SESSION","keys":"Down","delay":null}
{"command":"key","session":"$SESSION","keys":"Escape","delay":null}
{"command":"key","session":"$SESSION","keys":"Ctrl+C","delay":null}
{"command":"wait_for","session":"$SESSION","pattern":"$","regex":false,"not":false,"timeout":5000}
{"command":"type","session":"$SESSION","text":"echo roundtrip-done"}
{"command":"key","session":"$SESSION","keys":"Enter","delay":null}
{"command":"wait_for","session":"$SESSION","pattern":"roundtrip-done","regex":false,"not":false,"timeout":5000}
{"command":"snapshot","session":"$SESSION","format":"compact","await_change":null,"settle":null,"timeout":null}
{"command":"kill","session":"$SESSION"}
EOF
