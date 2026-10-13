#!/usr/bin/env bash
# Linux package smoke test: install the .deb, start RightPeek on a virtual X
# display, capture the PRIMARY selection through `--show`, remove the
# package, and check that nothing is left behind (binary, desktop file,
# "Start at sign-in" entry). Needs root (sudo) and the packages xvfb, xclip.
#
# The single-instance plugin needs a session bus; on CI run it under
# `dbus-run-session`.
#
# Usage: scripts/linux-smoke.sh path/to/RightPeek_<version>_amd64.deb
set -euo pipefail

deb=$(realpath "$1")
data="$HOME/.local/share/com.rightpeek.app"
log="$data/logs/rightpeek.log"
autostart="$HOME/.config/autostart/RightPeek.desktop"
sudo=$([ "$(id -u)" = 0 ] && echo "" || echo sudo)

fail() { echo "FAIL: $*" >&2; exit 1; }

echo "== install"
$sudo apt-get install -y -qq "$deb" >/dev/null
command -v rightpeek >/dev/null || fail "rightpeek not on PATH after install"

echo "== start on a virtual display"
Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &
xvfb=$!
trap 'pkill -x rightpeek || true; pkill -x xclip || true; kill $xvfb 2>/dev/null || true' EXIT
export DISPLAY=:99
unset WAYLAND_DISPLAY
sleep 2
mkdir -p "$data"
rm -f "$log"
# Log outcomes (never the text) and stay in the tray at start-up.
printf '{"settings":{"logging":true,"showSettingsOnStart":false}}' >"$data/settings.json"
rightpeek --autostart >/dev/null 2>&1 &
for _ in $(seq 30); do [ -s "$log" ] && break; sleep 0.5; done
sleep 2
pgrep -x rightpeek >/dev/null || fail "app exited at start-up"

echo "== capture via --show (second launch forwarded to the running app)"
printf 'سلام دنیا: smoke test' | xclip -selection primary
timeout 20 rightpeek --show
for _ in $(seq 20); do grep -q "via PrimarySelection" "$log" && break; sleep 0.5; done
grep -q "captured 21 chars via PrimarySelection" "$log" || {
    cat "$log" >&2
    fail "selection not captured"
}

echo "== remove"
# What the autostart plugin writes when "Start at sign-in" is on.
mkdir -p "$(dirname "$autostart")"
printf '[Desktop Entry]\nType=Application\nName=RightPeek\nExec=/usr/bin/rightpeek --autostart\n' >"$autostart"
pkill -x rightpeek || true
$sudo apt-get remove -y -qq right-peek >/dev/null
[ ! -e /usr/bin/rightpeek ] || fail "binary left behind"
[ ! -e /usr/share/applications/RightPeek.desktop ] || fail "desktop file left behind"
[ ! -e "$autostart" ] || fail "autostart entry left behind"

echo "PASS"
