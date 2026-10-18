# Development

How to build, test and debug RightPeek. The design is in
[`design.md`](design.md); releases are in [`release.md`](release.md).

## Setup

- Rust (stable, via rustup) and Node.js 20.
- Tauri 2 prerequisites:
  - **Windows:** WebView2 and the MSVC build tools.
  - **Debian/Ubuntu:** `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev`.
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- `npm ci`, then `npm run tauri dev` (debug, loads the Vite dev server) or
  `npm run tauri build` (release bundles in `src-tauri/target/release/bundle/`).

## Checks

Run before every pull request (CI runs the same on Windows, Linux and macOS):

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

- Logic in `src-tauri/src/core/` is written test-first; it must not call the OS.
- OS code in `src-tauri/src/platform/` is checked by hand against the apps in
  [`capture-matrix.md`](capture-matrix.md).
- Commits follow Conventional Commits; code, comments and docs are in English
  (`README.fa.md` is the Persian user README, kept in sync with `README.md`).

## Scripts

| Script | Purpose |
|---|---|
| `scripts/installer-smoke.ps1` | Silent install, launch and uninstall of the Windows installer; checks nothing is left behind |
| `scripts/linux-smoke.sh` | Installs the `.deb`, captures a PRIMARY selection through `--show` on Xvfb, removes the package |
| `scripts/macos-smoke.sh` | Mounts the `.dmg`, checks the bundle, starts and quits the app |
| `scripts/measure-budgets.ps1` | Measures the [budgets](design.md#budgets) on Windows (uses the keyboard; leave the PC alone) |
| `scripts/measure-budgets-macos.sh` | The same on macOS |
| `scripts/soak.ps1` | Long hotkey loop on Windows; the app must not die and memory must stay flat |
| `scripts/test-target.ps1`, `scripts/macos-test-target.swift` | A window with selected Persian text for the measurements |
| `scripts/macos-tray-icon.swift` | Draws the macOS menu-bar template icon |

The dev-only capture diagnostic is built with
`cargo run --manifest-path src-tauri/Cargo.toml --bin capture-probe --features probe -- [--uia | --clipboard] [delay_secs]`
(Windows). It is behind a feature because the bundler ships every binary.

## Notes

### Tauri / Windows

- Creating a window inside a **synchronous** Tauri command deadlocks IPC on
  Windows; make such commands `async`.
- Never capture a clone of a window in its own `on_window_event` closure
  (reference cycle); look it up by label.
- New app commands need `build.rs` (`AppManifest` list) and the window's
  capability file; new window APIs need explicit `core:window:allow-*`.
- UI tests use the **release** build (debug loads the Vite dev server). UI
  Automation on WebView2 builds its tree lazily: retry `FindFirst`.
- Synthetic keys fail behind the lock screen.
- `npm run tauri icon app-icon.png` also creates `icons/android` and
  `icons/ios`; delete them.
- Clean CI profiles may lack registry keys (e.g. `HKCU\...\CurrentVersion\Run`);
  create keys before writing values in scripts.
- Sign-in autostart on Windows is always a per-user Run entry
  (`app/autostart.rs`): the autostart plugin writes HKLM whenever that key is
  writable, which starts the app for every user and twice when an HKCU entry
  exists too.

### Linux

- Building inside WSL: use a Linux-native copy of the checkout (Windows
  `node_modules` holds Windows-only binaries).
- WSLg's compositor (Weston) has no primary selection, so Wayland-native
  selections cannot be tested there. X11 apps (or GTK apps with
  `GDK_BACKEND=x11`) work; `xclip -selection primary` sets a test selection.
- With systemd on, WSLg's Wayland socket may be missing from
  `$XDG_RUNTIME_DIR`; link `/mnt/wslg/runtime-dir/wayland-0` there.
- Without a desktop session `xdg-desktop-portal(-gtk)` makes GTK wait for a
  25 s DBus timeout at every launch; remove those packages in test machines.
- GTK ignores `resize()` on non-resizable windows; on Linux the Settings
  window is resizable with its width pinned by min/max size.
- The single-instance plugin uses the DBus session bus; headless runs need
  `dbus-run-session`.

### macOS

- Local release bundles: `CI=true npm run tauri build` (without `CI` the dmg
  step drives Finder through AppleScript and can time out).
- Accessibility (TCC) checks the *responsible* process: a binary started from
  a terminal is judged by the terminal's permission. Start the app with
  `open` (the scripts use `open -g --env VAR=... -a App`).
- Every rebuild of an ad-hoc signed app changes its code identity; macOS may
  keep listing RightPeek as allowed while denying it. Remove it from Privacy
  & Security > Accessibility and add it again.
- The Services entry only appears for an app in `/Applications` or
  `~/Applications`; refresh with `/System/Library/CoreServices/pbs -update`
  and check with `pbs -dump_pboard`. `NSPerformService("Show in RightPeek",
  pasteboard)` from a Swift script tests the handler without clicking.
- `NSApplication.servicesProvider` is not retained; the provider is leaked on
  purpose so it lives as long as the app.
- Electron and Qt apps (VS Code, Telegram) build their own context menus
  without Services; browsers and Electron also lack `AXSelectedText`, so
  capture there goes through Cmd+C.
