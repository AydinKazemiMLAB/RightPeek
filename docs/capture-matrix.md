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

Run 1 on the user's machine (Windows 11). Elapsed = default mode / `--clipboard` mode.
Firefox and Outlook are not installed; the user approved PotPlayer and WinRAR as
substitutes. Edge was tested as the PDF reader.

**Result: 11/11 correct, clipboard restored in all 22 runs, Win+V clean in all runs. Phase 1 criteria met.**

| # | App | Default method | Text correct | Clipboard restored | Win+V clean | Elapsed (ms) | Notes |
|---|---|---|---|---|---|---|---|
| 1 | Chrome | Clipboard | ✅ | ✅ | ✅ | 90 / 17 | ZWNJ preserved |
| 2 | Edge (PDF reader) | Clipboard | ✅ | ✅ | ✅ | 105 / 39 | PDF text layer uses Arabic presentation forms (U+FB50–U+FEFF); normalized in Phase 2 |
| 3 | PotPlayer settings (substitute for Firefox) | Clipboard | ✅ | ✅ | ✅ | 37 / 16 | |
| 4 | Word | Uia | ✅ | ✅ | ✅ | 170 / 67 | |
| 5 | Notepad | Uia | ✅ | ✅ | ✅ | 31 / 22 | |
| 6 | VS Code | Clipboard | ✅ | ✅ | ✅ | 110 / 26 | |
| 7 | Windows Terminal | Uia | ✅ | ✅ | ✅ | 47 / 14 | Captured mojibake matches the screen (the shell printed UTF-8 as cp1252) |
| 8 | Telegram Desktop | Clipboard | ✅ | ✅ | ✅ | 79 / 21 | |
| 9 | Claude desktop (Electron) | Clipboard | ✅ | ✅ | ✅ | 77 / 26 | |
| 10 | WinRAR (substitute for Outlook) | Clipboard | ✅ | ✅ | ✅ | 56 / 16 | |
| 11 | File Explorer rename box | Clipboard | ✅ | ✅ | ✅ | 75 / 18 | |

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
