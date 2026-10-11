# Capture matrix (Phase 1)

Results of `capture-probe` per target app (docs/PLAN.md §8, Phase 1).
**Done when:** ≥ 9/11 apps return correct text, the clipboard is verifiably restored,
and nothing appears in Win+V history.

## How to run

Build once from the repo root:

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin capture-probe
```

Per app:

1. Copy a known marker text (e.g. `marker-123`) to the clipboard.
2. Select a Persian sentence (ideally with a ZWNJ, e.g. «می‌خواهم») in the target app.
3. Run `src-tauri\target\debug\capture-probe.exe 5` and switch to the target app within 5 s
   without losing the selection.
4. Record `method`, whether the text is exact, the `clipboard:` line, and `elapsed`.
5. Run again with `--clipboard` (fallback only); then press **Win+V** and check that the
   selected text is **not** in the history (requires clipboard history to be on).
6. Optional: `--uia` prints why UI Automation fails for that app.

Legend: ✅ yes · ❌ no · — not applicable / not tested.

## Target apps

| # | App | Default method | Text correct | Clipboard restored | Win+V clean | Elapsed (ms) | Notes |
|---|---|---|---|---|---|---|---|
| 1 | Chrome | | | | | | |
| 2 | Edge | | | | | | |
| 3 | Firefox | | | | | | |
| 4 | Word | | | | | | |
| 5 | Notepad | | | | | | |
| 6 | VS Code | | | | | | |
| 7 | Windows Terminal | | | | | | Ctrl+C without selection sends SIGINT |
| 8 | Telegram Desktop | | | | | | |
| 9 | PDF reader | | | | | | |
| 10 | Outlook | | | | | | |
| 11 | File Explorer rename box | | | | | | |

## Reference targets (development self-test, not counted)

| Target | Default method | Text correct | Clipboard restored | Win+V clean | Elapsed (ms) | Notes |
|---|---|---|---|---|---|---|
| WinForms `TextBox` (Win32 EDIT) | Clipboard | ✅ (Persian + ZWNJ) | ✅ | — | 63 | No UIA TextPattern on this control |
| WinForms, `--clipboard` | Clipboard | ✅ | ✅ | ✅ | 6–13 | Unique token absent from Win+V history; positive control (a normal `Set-Clipboard`) present |
| WPF `TextBox` | Uia | ✅ (58 chars) | ✅ (untouched) | — | 38 | |

## Known limits

- Elevated (administrator) windows: `AccessDenied` by design (UIPI); no workaround.
- Apps that block copy or have no selection: `NothingSelected` after ~500 ms.
- Delay-rendered clipboard formats and GDI formats (bitmaps, metafiles) are not restored.
