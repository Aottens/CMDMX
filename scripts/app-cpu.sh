#!/usr/bin/env bash
# UI-70: the app's CPU budget. Starts the release daemon (temp show folder, no sACN/serial) and the release app in
# console mode, waits 12 s, then averages the app's %cpu over 20 s, for two loads: idle (budget 5 %) and --demo
# (budget 15 %). Exit 1 if either is over budget. Opens the app window for ~70 s and uses the owner's UserDefaults
# (no isolation). Usage: scripts/app-cpu.sh [--no-build]
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD

if [[ "${1:-}" != "--no-build" ]]; then
  (cd subprojects/engine && cargo build --release -p cmdmx-daemon --quiet)
  (cd subprojects/app && swift build -c release --quiet)
fi

if lsof -iTCP:7319 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "app-cpu: something already listens on 127.0.0.1:7319; stop it first (the app would talk to the wrong engine)" >&2
  exit 1
fi

TMP=$(mktemp -d)
DP=""; AP=""
cleanup() { kill $DP $AP 2>/dev/null || true; wait 2>/dev/null || true; rm -rf "$TMP"; }
trap cleanup EXIT

measure() { # label budget [daemon args]
  local label=$1 budget=$2; shift 2
  "$ROOT/subprojects/engine/target/release/cmdmx-daemon" --show-dir "$TMP" --no-sacn --no-serial "$@" >"$TMP/daemon-$label.log" 2>&1 & DP=$!
  sleep 2
  "$ROOT/subprojects/app/.build/release/CMDMX" -consoleMode YES >"$TMP/app-$label.log" 2>&1 & AP=$!
  sleep 12
  kill -0 $AP 2>/dev/null || { echo "app-cpu: app died during $label" >&2; exit 1; }
  local avg
  avg=$(for _ in $(seq 20); do ps -o %cpu= -p $AP; sleep 1; done | awk '{s+=$1} END {printf "%.1f", s/NR}')
  echo "$label: app $avg % (budget $budget %)" >&2
  kill $DP $AP 2>/dev/null || true; wait 2>/dev/null || true; DP=""; AP=""
  RESULT=$avg
}

measure idle 5; IDLE=$RESULT
measure demo 15 --demo; DEMO=$RESULT
echo "app-cpu: idle $IDLE %, demo $DEMO %"
awk -v i="$IDLE" -v d="$DEMO" 'BEGIN {exit !(i <= 5 && d <= 15)}'
