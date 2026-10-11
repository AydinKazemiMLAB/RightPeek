# Budget measurements

Budgets from docs/PLAN.md §2, measured with `scripts/measure-budgets.ps1` on a
release build (`npm run tauri build -- --no-bundle`). Machine: the user's
Windows 11 laptop. Hotkey under test: Ctrl+Alt+Q (Ctrl+Alt+Space is taken by
another app on this machine).

## Phase 2 (2026-10-11)

| Metric | Target | Run 1 | Run 2 | Status |
|---|---|---|---|---|
| Idle RAM, app only (private working set) | ≤ 20 MB | 1.6 MB | 1.6 MB | ✅ |
| RAM with viewer open (incl. WebView2) | ≤ 120 MB | 92.8 MB | 88.8 MB | ✅ |
| Hotkey → text visible, warm | ≤ 300 ms | 50 ms | 24 ms | ✅ |
| Hotkey → text visible, cold webview | ≤ 600 ms | 846 ms | 439 ms | ⚠️ first run after build misses |
| CPU while idle | ~0% | 0% | 0% | ✅ |
| Installer size | ≤ 8 MB | — | — | Not measured: needs the NSIS toolchain download (release exe is 4.74 MB) |

Notes:
- Run 1 was the first launch after the build (WebView2 runtime not in the OS
  file cache); run 2 is a typical cold viewer. Cross-check for idle memory:
  working set 12.5 MB including shared pages, private bytes 2.1 MB.
- Cold-start mitigation is planned for Phase 3 (optional pre-warm viewer).
