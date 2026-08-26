#!/usr/bin/env bash
# Run the client under gdb with linker logging; produce a symbolized crash dump.
#   tools/catch_crash.sh [gamedir]
# Output: /tmp/mc-run.log (full session) — paste the === FAULT / BACKTRACE sections.
set -uo pipefail
cd "$(dirname "$0")/.."

GAMEDIR="${1:-$HOME/.local/MinecraftLauncher/extracted/26.33/}"
export MC_CRASH_LOG=/tmp/mc-run.log
export MC_CRASH_GAMEDIR="$GAMEDIR/lib/x86_64"

: > "$MC_CRASH_LOG"
RUST_LOG="${RUST_LOG:-linker=info}" gdb -q -x tools/catch_crash.gdb --args ./target/debug/client -dg "$GAMEDIR" 2>&1 | tee "$MC_CRASH_LOG"
