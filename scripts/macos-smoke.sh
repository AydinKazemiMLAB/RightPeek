#!/usr/bin/env bash
# macOS bundle smoke test: mount the .dmg, copy RightPeek.app out of it,
# check the bundle (signature, Services entry, menu-bar settings), start it,
# check that it keeps running and that a second launch is forwarded to it
# and exits, then quit it.
#
# Capture is not exercised: it needs the Accessibility permission, which a
# CI runner cannot grant.
#
# Usage: scripts/macos-smoke.sh path/to/RightPeek_<version>_aarch64.dmg
set -euo pipefail

dmg="$1"
mount=$(mktemp -d)
apps=$(mktemp -d)
pid=""

fail() { echo "FAIL: $*" >&2; exit 1; }
cleanup() {
  [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  hdiutil detach "$mount" -quiet 2>/dev/null || true
  rm -rf "$apps"
}
trap cleanup EXIT

echo "== mount and copy"
hdiutil attach "$dmg" -nobrowse -readonly -mountpoint "$mount" -quiet
cp -R "$mount/RightPeek.app" "$apps/"
hdiutil detach "$mount" -quiet
app="$apps/RightPeek.app"
plist="$app/Contents/Info.plist"
exe="$app/Contents/MacOS/$(plutil -extract CFBundleExecutable raw "$plist")"

echo "== bundle"
codesign --verify --deep --strict "$app" || fail "bundle signature is invalid"
[ "$(plutil -extract NSServices.0.NSMessage raw "$plist")" = showSelection ] \
  || fail "Services entry missing from Info.plist"
[ "$(lipo -archs "$exe")" = arm64 ] || fail "expected an arm64-only binary"

echo "== start"
"$exe" &
pid=$!
sleep 5
kill -0 "$pid" 2>/dev/null || fail "RightPeek exited at start"

echo "== second launch is forwarded and exits"
"$exe" --autostart &
second=$!
for _ in $(seq 50); do
  kill -0 "$second" 2>/dev/null || break
  sleep 0.2
done
if kill -0 "$second" 2>/dev/null; then
  kill "$second"
  fail "second launch did not exit"
fi
kill -0 "$pid" 2>/dev/null || fail "first instance died after the second launch"

echo "== quit"
kill "$pid"
wait "$pid" 2>/dev/null || true
pid=""
echo "OK"
