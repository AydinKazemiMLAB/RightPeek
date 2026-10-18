# Budget measurements

Budgets from docs/design.md, measured with `scripts/measure-budgets.ps1` on a
release build (`npm run tauri build -- --no-bundle`). Machine: the user's
Windows 11 laptop. Hotkey under test: Ctrl+Alt+Q for runs 1-2 (Ctrl+Alt+Space is
taken by another app on this machine), Ctrl+Shift+Space (the new default) for run 3.

## Phase 2 (2026-10-11)

| Metric | Target | Run 1 | Run 2 | Run 3 | Status |
|---|---|---|---|---|---|
| Idle RAM, app only (private working set) | ≤ 20 MB | 1.6 MB | 1.6 MB | 1.6 MB | ✅ |
| RAM with viewer open (incl. WebView2) | ≤ 120 MB | 92.8 MB | 88.8 MB | 89.5 MB | ✅ |
| Hotkey → text visible, warm | ≤ 300 ms | 50 ms | 24 ms | 20 ms | ✅ |
| Hotkey → text visible, cold webview | ≤ 600 ms | 846 ms | 439 ms | 510 ms | ⚠️ first run after build misses |
| CPU while idle | ~0% | 0% | 0% | 0% | ✅ |
| Installer size (NSIS) | ≤ 8 MB | — | — | 1.59 MB | ✅ (release exe is 4.74 MB) |

Notes:
- Run 1 was the first launch after the build (WebView2 runtime not in the OS
  file cache); run 2 is a typical cold viewer. Cross-check for idle memory:
  working set 12.5 MB including shared pages, private bytes 2.1 MB.
- Cold-start mitigation is planned for Phase 3 (optional pre-warm viewer).

## Phase 3 (2026-10-12)

Default settings (pre-warm off, logging off), hotkey Ctrl+Shift+Space.

| Metric | Target | Result | Status |
|---|---|---|---|
| Idle RAM, app only (private working set) | ≤ 20 MB | 1.6 MB | ✅ |
| RAM with viewer open (incl. WebView2) | ≤ 120 MB | 91 MB | ✅ |
| Hotkey → text visible, warm | ≤ 300 ms | 44 ms | ✅ |
| Hotkey → text visible, cold webview | ≤ 600 ms | 412 ms | ✅ |
| CPU while idle | ~0% | 0% | ✅ |
| Release exe | — | 4.85 MB | — |

With pre-warm on, the hidden viewer exists from startup and the first capture
showed text in 63 ms (the viewer then stays alive, so idle RAM includes
WebView2, about 90 MB; that is why it is opt-in). The pre-warm setting was
removed in Phase 4 for that RAM cost. The installer was not
rebuilt in this phase; Phase 2 measured 1.59 MB.

### 30-minute soak

`scripts/soak.ps1 -Minutes 30` on the release build, default settings (pre-warm
off, so every round creates and destroys the viewer). Each round captures text
from one test window, closes the viewer with Esc, then presses the hotkey over
an empty window to get the "No text selected" notice.

| Check | Result |
|---|---|
| Rounds (2 hotkey presses each) | 197 |
| Viewer did not appear | 0 |
| Panics / app died | none, still alive |
| App private working set, start → end | 1.6 MB → 10 MB |

The "Done when" criterion (no panics, budgets met) holds. Memory is not flat:
the app process grows about 9 KB per viewer opened (6.9 MB at round 30,
9.7 MB at round 190). Removing a window reference cycle from the close handler
did not change the trend (an earlier run went 1.6 → 9.7 MB), so the growth
is not that cycle.

A 10-minute soak with pre-warm on (the viewer is hidden, never destroyed) stayed
flat: 66 rounds, 66 captures, 66 notices, no other warnings, 4.6 MB at round 10
and 4.7 MB at round 60 (pre-warm off grew about 1.1 MB over the same 60
rounds). So the growth comes from creating and destroying the viewer window,
not from capture or rendering. Our per-window code holds no references after
close, so it is likely inside Tauri/WebView2 window setup. At about 9 KB per
open it would take roughly 2,000 opens without a restart to reach the 20 MB
idle budget; accepted for now, worth re-checking when Tauri is upgraded.

Soak harness fixes from this phase: find the viewer by its title, stop if the
screen locks, keep both test windows open for the whole run (closing the
active window left keyboard focus unreliable).

## Phase 4 (2026-10-13)

| Metric | Result |
|---|---|
| NSIS installer | 1.70 MB |
| Release / portable exe | 4.87 MB |
| CI smoke test (install, run, uninstall, leftovers) | passes on `windows-2022` and `windows-2025` |

Runtime budgets were not re-measured: Phase 4 only removed the pre-warm path
and added toolbar/settings controls; capture and rendering are unchanged
since the Phase 3 numbers above.

## Phase 5 — Linux (2026-10-13)

Measured on Ubuntu 24.04 in WSL2/WSLg (software rendering through Mesa, no
desktop session), release `.deb`. Memory is `Private_Clean + Private_Dirty`
from `/proc/<pid>/smaps_rollup` (closest to Windows' private working set).
WSLg is not representative for memory; re-measure on a real Linux desktop.

| Metric | Budget | Result |
|---|---|---|
| `.deb` / `.rpm` package | ≤ 8 MB | 3.22 MB each ✅ (installed size 7.8 MB) |
| Idle RAM, no window (app process) | ≤ 20 MB | 29–32 MB ❌ (GTK + WebKitGTK libraries) |
| RAM with viewer open (app + WebKit processes) | ≤ 120 MB | ~285 MB ❌ (app ~78 MB, WebKitWebProcess + NetworkProcess ~205 MB) |
| Capture time (hotkey / `--show` → text read) | — | 2–5 ms |
| Start-up to tray-ready | — | ~1 s |
| CPU while idle | ~0% | 0.1% ✅ |

WebKitGTK runs the page in separate processes like WebView2; on WSLg they also
carry the software GL stack. The memory misses are open: decide Linux budgets
after measuring on a real desktop.

## Phase 6 — macOS (2026-10-17)

Measured with `scripts/measure-budgets-macos.sh` on the release bundle
(`npm run tauri build`, installed in `~/Applications`) on the maintainer's
Mac: Apple M5, macOS 26.6.2. Memory is the physical footprint (`footprint`),
the macOS counterpart of the private working set; "viewer open" adds the
three WebKit helper processes (WebContent, Networking, GPU) the viewer
starts. Captures are requested with `--show` against an AppKit test window
with a selected Persian sentence (read through Accessibility).

| Metric | Target | Run 1 | Run 2 | Status |
|---|---|---|---|---|
| Idle RAM, app only (footprint) | ≤ 20 MB | 14.0 MB | 14.0 MB | ✅ |
| RAM with viewer open (incl. WebKit helpers) | ≤ 120 MB | 114.5 MB | 124.5 MB | ⚠️ at the limit; accepted by the user |
| Capture → text visible, warm | ≤ 300 ms | 41 ms | 43 ms | ✅ |
| Capture → text visible, cold webview | ≤ 600 ms | 259 ms | 244 ms | ✅ |
| CPU while idle (30 s) | ~0% | 0 s CPU time | 0 s CPU time | ✅ |
| Installer size (dmg) | ≤ 8 MB | 3.80 MB | — | ✅ (binary 4.49 MB, arm64) |

Notes:
- Footprint counts memory the way Activity Monitor's "Memory" column does;
  WebKit's GPU process alone is a large share of the viewer-open figure.
- Real hotkey captures in the maintainer's test session took 0–8 ms
  (Accessibility) and 177–224 ms (Cmd+C fallback, mostly the wait for the
  copy) before the viewer was shown.

## Theme choice and viewer editing — Windows (2026-10-18)

Release build of the theme/edit change, same machine and script, hotkey
Ctrl+Shift+Space, "Open this window when RightPeek starts" off for the run
(otherwise Settings is open during the idle measurement).

| Metric | Target | Run 1 | Run 2 | Status |
|---|---|---|---|---|
| Idle RAM, app only (private working set) | ≤ 20 MB | 1.6 MB | 1.6 MB | ✅ |
| RAM with viewer open (incl. WebView2) | ≤ 120 MB | 89.3 MB | 87 MB | ✅ |
| Hotkey → text visible, warm | ≤ 300 ms | 56 ms | 57 ms | ✅ |
| Hotkey → text visible, cold webview | ≤ 600 ms | 535 ms | 511 ms | ✅ |
| CPU while idle | ~0% | 0% | 0% | ✅ |
| Release exe | — | 4.86 MB | 4.86 MB | — |

Notes:
- The feature adds no dependency and under 1 KB of script; RAM matches
  Phase 3 (91 MB). Cold start is within the range seen before (Phase 2:
  439–510 ms on typical runs, Phase 3: 412 ms); the WebView2 runtime has
  been updated since, so the runs are not strictly comparable.
- With the viewer and Settings both open the tree used 117–121 MB: the
  second window costs about 30 MB until it is closed (unchanged by this
  feature; Settings existed before).
