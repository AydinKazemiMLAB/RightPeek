# Budget measurements

Budgets from docs/PLAN.md §2, measured with `scripts/measure-budgets.ps1` on a
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
WebView2, about 90 MB; that is why it is opt-in). The installer was not
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
