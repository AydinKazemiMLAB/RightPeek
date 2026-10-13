# RightPeek

Select text in any app on Windows or Linux, press a shortcut, and read it with
correct right-to-left layout. RightPeek lives in the tray and shows the selection in a
small viewer where Persian, Arabic and Hebrew text (mixed with English, numbers
and parentheses) is laid out the way it should be.

## Install

### Windows

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

### Linux

Download the package for your distribution from the Releases page and install
it (needs WebKitGTK 4.1, which the package manager pulls in):

```bash
sudo apt install ./RightPeek_<version>_amd64.deb
```

```bash
sudo dnf install ./RightPeek-<version>-1.x86_64.rpm
```

Built on Ubuntu 22.04; newer Debian/Ubuntu/Fedora releases work too. On
Wayland, install `wl-clipboard` as well (the `.deb` recommends it).

## Use

1. Select text in any app.
2. Press **Ctrl+Shift+Space** (change it in Settings from the tray icon).
3. The viewer shows the text. Copy it, change the font size (Ctrl + / Ctrl −),
   force RTL/LTR, or justify it. Esc closes the viewer.

RightPeek cannot read windows that run as administrator; Windows blocks that
for normal apps.

**Linux:** RightPeek reads the selection directly (the "primary selection"),
so your clipboard is never touched. On **Wayland** the shortcut only works
while an X11 app has focus; add a keyboard shortcut in your desktop settings
(GNOME: Settings → Keyboard → Custom Shortcuts; KDE: System Settings →
Shortcuts) that runs `rightpeek --show`. Settings shows the exact command.

## Uninstall

Settings → Apps → Installed apps → RightPeek → Uninstall. The uninstaller also
removes the "Start with Windows" entry and the records Windows keeps about the
exe in your user registry. `C:\Windows\Prefetch` files are managed by Windows
(admin only) and recycled automatically. Tick "Delete the application data" to
remove settings and logs as well.

**Linux:** `sudo apt remove right-peek` (or `sudo dnf remove right-peek`).
This also removes the "Start at sign-in" entry. Settings and logs stay in
`~/.local/share/com.rightpeek.app`; delete that folder to remove them.

## Privacy

RightPeek does not connect to the internet and collects no data. Captured text
stays in memory and is never written to disk or logs. Settings are stored in
`%APPDATA%\com.rightpeek.app`. The optional log file (off by default) lives in
`%LOCALAPPDATA%\com.rightpeek.app\logs` and records events and errors only.
The only network access is the installer downloading WebView2 from Microsoft
when it is missing. On Linux, settings and logs live in
`~/.local/share/com.rightpeek.app`.

## Build from source

Requirements: Rust (stable), Node.js 20, and the Tauri 2 prerequisites:
WebView2 and the MSVC build tools on Windows; on Debian/Ubuntu
`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev`.

```bash
npm ci
npm run tauri build
```

The installer is written to `src-tauri/target/release/bundle/nsis/`.
`docs/PLAN.md` describes the design; `docs/release.md` the release process.
