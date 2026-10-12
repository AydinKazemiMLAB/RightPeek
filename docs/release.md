# Release process

## Installer

- Tauri NSIS bundle (`src-tauri/tauri.conf.json` → `bundle`): per-user install
  to `%LOCALAPPDATA%\RightPeek`, no UAC prompt. WebView2 is downloaded silently
  by the bootstrapper only when missing. No MSI (NSIS only, decided in Phase 4).
- `src-tauri/windows/hooks.nsh` removes the "Start with Windows" Run entry on a
  real uninstall (kept when an update reinstalls). The template's own
  "Delete the application data" box removes `%APPDATA%` / `%LOCALAPPDATA%`
  `com.rightpeek.app` (settings, logs, WebView2 cache).
- The template checks whether RightPeek is running before installing or
  uninstalling and asks to close it.
- Icons come from `app-icon.png` (1024 px master): `npm run tauri icon app-icon.png`,
  then delete the generated `android/` and `ios/` folders.

## Smoke test

`scripts/installer-smoke.ps1` installs silently, launches the app, uninstalls
silently and checks that the exe, the Installed apps entry, the Start menu
shortcut and the Run entry are gone. CI runs it on `windows-2022` and
`windows-2025` (closest hosted images to Windows 10 and 11) for every release
and for pull requests that touch packaging. Run it locally after
`npm run tauri build` with RightPeek closed.

## Cutting a release

1. Bump `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   `package.json`; merge to `main`.
2. Tag `vX.Y.Z` on `main` and push the tag (ask the maintainer first: it is
   irreversible).
3. `.github/workflows/release.yml` builds, smoke-tests and creates a **draft**
   release with the installer. Review it and publish by hand.

No auto-updater for now (decided in Phase 4): the repository is private, so
clients could not download updates from its releases.

## Code signing (SignPath Foundation, planned)

Free signing for open-source projects. The certificate is issued to
"SignPath Foundation", which then appears as the publisher. Prerequisites,
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
