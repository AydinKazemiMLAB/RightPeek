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
