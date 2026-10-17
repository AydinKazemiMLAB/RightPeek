# Development notes

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

## Linux / WSL

- Linux is built and tested in Ubuntu 24.04 on WSL2 (WSLg). Build in a
  Linux-native copy (`rsync` the checkout to `~/rightpeek`, `npm ci` there):
  Windows `node_modules` holds Windows-only binaries.
- WSL behind the Windows proxy: set `networkingMode=mirrored` and
  `autoProxy=true` in `%USERPROFILE%\.wslconfig`, then `wsl --shutdown`.
- WSLg's compositor (Weston) has no primary selection: Wayland-native
  selections cannot be tested there. X11 apps (or GTK apps with
  `GDK_BACKEND=x11`) work; `xclip -selection primary` sets a test selection.
- With systemd on, WSLg's Wayland socket may be missing from
  `$XDG_RUNTIME_DIR`; link `/mnt/wslg/runtime-dir/wayland-0` there.
- `xdg-desktop-portal(-gtk)` cannot start without a desktop session; GTK then
  waits the 25 s DBus timeout at every launch (the single-instance name is
  taken late, so `--show` looks hung). Remove those packages in WSL.
- GTK ignores `resize()` on non-resizable windows and sizes them to the
  webview's natural height (the whole screen); on Linux the settings window is
  resizable with its width pinned by min/max size.
- The single-instance plugin uses the DBus session bus; headless runs need
  `dbus-run-session`.

## macOS

- Built and tested on the maintainer's Mac (Apple Silicon). Local release
  bundles: `CI=true npm run tauri build` (without `CI` the dmg step drives
  Finder through AppleScript and times out unless the terminal may control
  Finder).
- TCC (Accessibility) checks the *responsible* process: a binary started
  from a terminal is judged by the terminal's permission, not RightPeek's.
  Start the app with `open` (scripts do: `open -g --env VAR=... -a App`).
- Every rebuild of an ad-hoc signed app changes its code identity; macOS may
  keep showing RightPeek as allowed while denying it. Remove it from
  Privacy & Security > Accessibility and add it again. Avoid rebuilding in
  the middle of a manual test session.
- The Services entry only appears for an app in `/Applications` or
  `~/Applications`; refresh with `/System/Library/CoreServices/pbs -update`
  and check with `pbs -dump_pboard`. Test the handler without clicking:
  `NSPerformService("Show in RightPeek", pasteboard)` from a Swift script.
- `NSApplication.servicesProvider` is not retained; the provider is leaked
  on purpose so it lives as long as the app.
- Electron and Qt apps (VS Code, Telegram) build their own context menus
  without Services; browsers and Electron also lack `AXSelectedText`, so
  capture there goes through Cmd+C.
- Agent sessions cannot screenshot (no Screen Recording permission) or post
  keys to other apps; list windows with `CGWindowListCopyWindowInfo` from a
  Swift script and ask the maintainer to check visuals.
