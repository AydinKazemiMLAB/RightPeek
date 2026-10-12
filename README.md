# RightPeek

Select text in any Windows app, press a shortcut, and read it with correct
right-to-left layout. RightPeek lives in the tray and shows the selection in a
small viewer where Persian, Arabic and Hebrew text (mixed with English, numbers
and parentheses) is laid out the way it should be.

## Install

Download `RightPeek_<version>_x64-setup.exe` from the Releases page and run it.
It installs for the current user only (no administrator rights needed).
Windows 10 and 11 are supported; if the Microsoft Edge WebView2 runtime is
missing (Windows 11 always has it), the installer downloads it from Microsoft.

**Portable:** `RightPeek_<version>_x64-portable.exe` runs without installing.
It needs WebView2 already present, has no Start menu entry or uninstaller, and
still keeps its settings in `%APPDATA%\com.rightpeek.app`. Turn off "Start
RightPeek when Windows starts" before deleting or moving the exe.

On start, RightPeek opens its Settings window so you can see it is running;
untick "Open this window when RightPeek starts" to start quietly in the tray.
Starting with Windows is always quiet.

## Use

1. Select text in any app.
2. Press **Ctrl+Shift+Space** (change it in Settings from the tray icon).
3. The viewer shows the text. Copy it, change the font size (Ctrl + / Ctrl −),
   force RTL/LTR, or justify it. Esc closes the viewer.

RightPeek cannot read windows that run as administrator; Windows blocks that
for normal apps.

## Uninstall

Settings → Apps → Installed apps → RightPeek → Uninstall. The uninstaller also
removes the "Start with Windows" entry and the records Windows keeps about the
exe in your user registry. `C:\Windows\Prefetch` files are managed by Windows
(admin only) and recycled automatically. Tick "Delete the application data" to
remove settings and logs as well.

## Privacy

RightPeek does not connect to the internet and collects no data. Captured text
stays in memory and is never written to disk or logs. Settings are stored in
`%APPDATA%\com.rightpeek.app`. The optional log file (off by default) lives in
`%LOCALAPPDATA%\com.rightpeek.app\logs` and records events and errors only.
The only network access is the installer downloading WebView2 from Microsoft
when it is missing.

## Build from source

Requirements: Rust (stable), Node.js 20, and the Tauri 2 prerequisites for
Windows (WebView2, MSVC build tools).

```bash
npm ci
npm run tauri build
```

The installer is written to `src-tauri/target/release/bundle/nsis/`.
`docs/PLAN.md` describes the design; `docs/release.md` the release process.
