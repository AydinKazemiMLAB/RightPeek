#!/usr/bin/env bash
# macOS budget measurement (docs/design.md), the counterpart of
# measure-budgets.ps1. Starts RightPeek.app, measures idle memory and CPU,
# requests two captures (cold viewer, then warm), measures memory with the
# viewer open (the WebKit helper processes it starts included), and quits.
#
# Memory is the physical footprint (`footprint`), the macOS counterpart of
# the private working set. Captures are requested with `--show`, which the
# single-instance plugin forwards to the running app; the timing starts when
# the running app receives it, like the hotkey timing on Windows.
#
# The capture target is scripts/macos-test-target.swift (a window with a
# selected Persian sentence). RightPeek needs the Accessibility permission;
# without it the viewer shows a notice, which is timed the same way. The app
# is started through `open` so macOS checks RightPeek's own permission, not
# the terminal's.
#
# Usage: scripts/measure-budgets-macos.sh path/to/RightPeek.app [idle-seconds]
set -euo pipefail

app="$1"
idle_seconds="${2:-30}"
exe="$app/Contents/MacOS/$(plutil -extract CFBundleExecutable raw "$app/Contents/Info.plist")"
timing_log=$(mktemp)

footprint_mb() {
  # Sum of "Footprint: N KB|MB" over the given pids.
  footprint "$@" 2>/dev/null | awk '
    /Footprint:/ { for (i = 1; i < NF; i++) if ($i == "Footprint:") {
      v = $(i + 1); u = $(i + 2); total += (u == "MB" ? v : u == "GB" ? v * 1024 : v / 1024) } }
    END { printf "%.1f", total }'
}
webkit_pids() { pgrep -f 'com.apple.WebKit\.' | sort || true; }
timings() { grep -Eo '[0-9]+' "$timing_log" || true; }

measure_capture() {
  local before
  kill -USR1 "$target"
  sleep 1
  before=$(timings | wc -l)
  "$exe" --show
  for _ in $(seq 100); do
    [ "$(timings | wc -l)" -gt "$before" ] && break
    sleep 0.1
  done
  timings | tail -1
}

pgrep -x "$(basename "$exe")" >/dev/null && { echo "RightPeek is already running; quit it first." >&2; exit 1; }

open -g --env RIGHTPEEK_TIMING_LOG="$timing_log" -a "$app" --args --autostart
for _ in $(seq 50); do
  pid=$(pgrep -x "$(basename "$exe")" || true)
  [ -n "$pid" ] && break
  sleep 0.2
done
[ -n "$pid" ] || { echo "RightPeek did not start." >&2; exit 1; }
target=""
trap 'kill $pid $target 2>/dev/null || true; rm -f "$timing_log" "$timing_log.target"' EXIT
sleep 5

idle_ram=$(footprint_mb -p "$pid")
cpu_start=$(ps -o time= -p "$pid")
sleep "$idle_seconds"
cpu_end=$(ps -o time= -p "$pid")

swiftc -o "$timing_log.target" "$(dirname "$0")/macos-test-target.swift"
"$timing_log.target" 60 &
target=$!
sleep 4

webkit_before=$(webkit_pids)
cold=$(measure_capture)
sleep 2
webkit_new=$(comm -13 <(echo "$webkit_before") <(webkit_pids))
pid_args=(-p "$pid")
for helper in $webkit_new; do pid_args+=(-p "$helper"); done
viewer_ram=$(footprint_mb "${pid_args[@]}")
warm=$(measure_capture)

dmg=$(ls "$(dirname "$app")"/../dmg/*.dmg 2>/dev/null | head -1 || true)
dmg_mb=$([ -n "$dmg" ] && echo "scale=2; $(stat -f %z "$dmg") / 1048576" | bc || echo "-")

echo "Idle RAM, app only (MB footprint, <= 20):        $idle_ram"
echo "CPU time while idle ${idle_seconds}s (start -> end):   $cpu_start -> $cpu_end"
echo "RAM with viewer incl. WebKit (MB, <= 120):        $viewer_ram ($(echo $webkit_new | wc -w | tr -d ' ') WebKit helpers)"
echo "Capture -> visible, cold (ms, <= 600):            $cold"
echo "Capture -> visible, warm (ms, <= 300):            $warm"
echo "dmg size (MB, <= 8):                              $dmg_mb"
echo "Binary (MB):                                      $(echo "scale=2; $(stat -f %z "$exe") / 1048576" | bc)"
