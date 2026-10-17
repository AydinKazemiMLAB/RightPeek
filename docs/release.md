# Release process

## Installer

- Tauri NSIS bundle (`src-tauri/tauri.conf.json` → `bundle`): per-user install
  to `%LOCALAPPDATA%\RightPeek`, no UAC prompt. WebView2 is downloaded silently
  by the bootstrapper only when missing. No MSI (NSIS only, decided in Phase 4).
- `src-tauri/windows/hooks.nsh` removes, on a real uninstall (not on update),
  the "Start with Windows" Run entry and Windows' per-user usage records for
  the install path (Explorer `FeatureUsage\AppSwitched`, Program Compatibility
  Assistant `Store`). `C:\Windows\Prefetch` is not touched: admin only, and
  Windows recycles it. The template's own
  "Delete the application data" box removes `%APPDATA%` / `%LOCALAPPDATA%`
  `com.rightpeek.app` (settings, logs, WebView2 cache).
- The template checks whether RightPeek is running before installing or
  uninstalling and asks to close it.
- Icons come from `app-icon.png` (1024 px master): `npm run tauri icon app-icon.png`,
  then delete the generated `android/` and `ios/` folders.

- Portable build: the release exe itself, published as
  `RightPeek_<version>_x64-portable.exe` (needs WebView2 already installed).

## Linux packages

- `src-tauri/tauri.linux.conf.json` overrides the bundle targets on Linux:
  `.deb` and `.rpm` (package name `right-peek`, binary `/usr/bin/rightpeek`).
  No AppImage: it bundles its libraries (~80 MB, over the 8 MB budget).
- Built on `ubuntu-22.04` so the packages run on that glibc and newer.
  Dependencies (WebKitGTK 4.1, GTK 3, Ayatana AppIndicator) are detected by
  the bundler; `wl-clipboard` is recommended (Wayland primary-selection fallback).
- `src-tauri/linux/postrm-deb.sh` / `postun-rpm.sh` delete, on removal (not on
  upgrade), the per-user `~/.config/autostart/RightPeek.desktop` entries that
  point at `/usr/bin/rightpeek`. Settings and logs in
  `~/.local/share/com.rightpeek.app` are kept (Linux convention).

## macOS bundle

- `src-tauri/tauri.macos.conf.json` overrides the bundle targets on macOS:
  `RightPeek.app` and `RightPeek_<version>_aarch64.dmg`. Apple Silicon only
  (decided 2026-10-17); minimum macOS 11.
- Ad-hoc signed (`signingIdentity: "-"`, hardened runtime), not notarized:
  Gatekeeper blocks the first open of a downloaded copy (README: System
  Settings > Privacy & Security > Open Anyway). Signing and notarization need
  an Apple Developer account ($99/year) and the maintainer's approval.
- `src-tauri/Info.plist` is merged into the bundle's `Info.plist`: the
  `NSServices` entry "Show in RightPeek". The app sets the Accessory
  activation policy at start (menu bar only, no Dock icon).
- No uninstaller on macOS: users delete the app, and turn off "Start
  RightPeek when you sign in" first (or delete
  `~/Library/LaunchAgents/RightPeek.plist`).
- Locally, build with `CI=true npm run tauri build`: without `CI` the dmg
  script drives Finder through AppleScript to lay out the window, which
  times out unless the terminal may control Finder.
- The menu-bar icon is `src-tauri/icons/tray-macos.rgba` (raw RGBA template
  image, drawn by `scripts/macos-tray-icon.swift`).

## Smoke test

`scripts/installer-smoke.ps1` installs silently, launches the app, uninstalls
silently and checks that the exe, the Installed apps entry, the Start menu
shortcut, the Run entry and the usage records are gone. CI also checks that
the portable exe starts. CI runs it on `windows-2022` and
`windows-2025` (closest hosted images to Windows 10 and 11) for every release
and for pull requests that touch packaging. Run it locally after
`npm run tauri build` with RightPeek closed.

`scripts/linux-smoke.sh` installs the `.deb`, starts RightPeek on Xvfb,
captures a PRIMARY selection through `rightpeek --show` (checked in the log),
removes the package and checks that the binary, the desktop file and the
autostart entry are gone. CI runs it under `dbus-run-session` on
`ubuntu-22.04` and `ubuntu-24.04`.

`scripts/macos-smoke.sh` mounts the dmg, copies the app out, checks the
signature, the Services entry and the arm64 binary, starts the app, checks
that a second launch is forwarded and exits, and quits. CI runs it on
`macos-14` and `macos-15`. Capture is not covered there (CI cannot grant the
Accessibility permission); see the macOS capture matrix.

## Cutting a release

1. Bump `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   `package.json`; merge to `main`.
2. Run the Release workflow (push a `vX.Y.Z` tag, or start it by hand from
   the Actions tab). It builds and smoke-tests the installer, the portable exe,
   the Linux packages and the macOS dmg and keeps them as the run's artifacts
   (`installer`, `linux-packages`, `macos-dmg`). It creates no GitHub release.
3. The maintainer creates the GitHub release by hand and attaches the files.

No auto-updater for now: deferred in Phase 4 while the repository was
private, and set aside again by the maintainer on 2026-10-14 after it went
public. Users update by installing the new release over the old one.

## Code signing (deferred)

Not needed for now (decided 2026-10-12): releases are unsigned, so Windows
SmartScreen warns on first run ("More info" → "Run anyway"). When it is
picked up again, the plan is SignPath Foundation: free signing for open-source
projects. The certificate is issued to "SignPath Foundation", which then
appears as the publisher. Prerequisites,
from https://signpath.org/terms:

- [ ] An OSI-approved license for all code (no commercial dual licensing).
- [ ] Public source repository; builds automated and verifiable from it (this
      workflow).
- [ ] At least one public release in the form that will be signed.
- [ ] A "Code signing policy" section on the project page naming the team roles
      (authors, reviewers, approvers) and a privacy statement (README).
- [ ] Multi-factor authentication on GitHub and SignPath for every team member.
- [ ] Apply at https://signpath.org/apply, then add the SignPath GitHub action
      to `release.yml` with its API token stored as a repository secret. Each
      signing request needs a manual approval.
