# Windows/Tauri development notes

Gotchas learned in Phases 0–4. Check these before debugging something that
looks like one of them.

- Creating a window inside a **synchronous** Tauri command deadlocks IPC on
  Windows → make such commands `async`.
- Never capture a clone of a window in its own `on_window_event` closure
  (reference cycle); look it up by label.
- New app commands need `build.rs` (`AppManifest` list) + the window's
  capability file; new window APIs need explicit `core:window:allow-*`.
- UI tests: use the **release** build (debug loads the Vite dev server).
  UIA on WebView2: retry `FindFirst` (tree builds lazily); plain buttons use
  `InvokePattern`, `aria-pressed` buttons and checkboxes use `TogglePattern`.
  Screenshots: `PrintWindow(hwnd, dc, 2)`.
- Synthetic keys fail behind the lock screen; scripts check for `LogonUI`.
  Do not drive Microsoft Edge for tests (it signed into the user's account).
- PowerShell 5.1: `FindWindow` needs `[NullString]::Value` for a null class;
  `Set-Content -Encoding utf8` writes a BOM (the app now tolerates it);
  read files with `-Encoding UTF8`; quotes inside here-strings break native
  args — write git commit messages through Bash `git commit -F - <<'EOF'`.
- `npm run tauri icon app-icon.png` also creates `icons/android` and
  `icons/ios`; delete them.
- Clean CI profiles may lack registry keys (e.g. `HKCU\...\CurrentVersion\Run`);
  create keys before writing values in scripts.
