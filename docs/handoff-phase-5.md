# Handoff: starting Phase 5 in a new session

State on 2026-10-13. Read this once at the start of Phase 5, then delete it in
the Phase 5 PR (it is not a permanent doc).

## Where the project is

- Phases 0–4 are merged to `main` (PRs #1–#5). `docs/PLAN.md` §8 has every
  phase as a checklist; 0–4 are ticked ✅.
- Phase 4 shipped: per-user NSIS installer (1.70 MB), portable exe (4.87 MB),
  uninstall cleanup hooks (`src-tauri/windows/hooks.nsh`), installer smoke test
  (`scripts/installer-smoke.ps1`), release workflow (`.github/workflows/release.yml`,
  draft release on `v*` tags), README, `docs/release.md`.
- No tag or release has been created yet. The repo is **private**, no license.
- 72 Rust tests pass. Budgets: `docs/budgets.md`.

## Deferred / open items (do not start without the user)

- Code signing (plan: SignPath Foundation; needs OSI license + public repo +
  first public release) and the updater plugin: deferred by the user.
- Viewer memory grows ~9 KB per viewer window created (Tauri/WebView2 side,
  far below budget); documented in `docs/budgets.md`.
- Runtime budgets were not re-measured in Phase 4 (no capture/render change).
- GitHub auto-merge is unavailable (private repo on the free plan).

## Phase 5 (PLAN §8) — what to settle before coding

Goal: Chrome/Edge MV3 extension with a "Show RTL" context-menu entry for the
selection → native messaging → host → running RightPeek shows the text; long
texts (≥ 100 KB) must arrive intact.

Questions to raise with the user at phase start (architecture/dependencies
need their approval per CLAUDE.md):

1. **Host binary:** a mode of `rightpeek.exe` (e.g. `--native-host`) vs a
   separate small exe. Chrome starts the host with the extension origin as an
   argument and talks over stdin/stdout (4-byte length + JSON; host→browser
   messages ≤ 1 MB, browser→host ≤ 64 MB).
2. **Forwarding to the running instance:** the single-instance plugin passes
   argv only, and the Windows command line is limited to 32K chars, so 100 KB
   cannot travel that way. Likely a per-user named pipe served by the running
   app (architecture change → ask). Start the app if it is not running.
3. **Registration:** host manifest JSON + `HKCU\Software\Google\Chrome\NativeMessagingHosts\<name>`
   and `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\<name>`; written by
   the installer (NSIS hook) and removed on uninstall; the portable exe would
   need to register itself. `allowed_origins` needs a fixed extension ID (a
   `key` in the extension manifest for unpacked/dev loading).
4. **Distribution of the extension:** unpacked load for development; Chrome Web
   Store / Edge Add-ons publishing is publishing → ask the user first.
5. Where the extension lives in the repo (e.g. `extension/`) and how CI checks it.

## Working rules that are easy to forget

- Chat in **Persian**; everything in the repo in English. The user does not
  know Rust: guide any install step by step and wait for "فاز x رو شروع کن".
- Post a Persian checklist at phase start; stop for approval at phase end.
- Git/GitHub routine (branch `phase-5`, commits, push, PR, Auto-fix, merge
  with a merge commit after green CI, delete the branch) is pre-authorized —
  do not ask. Tags, releases, making the repo public, signing: ask first.
- CI: do not poll. Use `ccd_pr` get_status; Auto-fix wakes the session on
  failures only, so after pushing ask the user to say "ادامه بده" when green.
- Phase gate: fmt, clippy `-D warnings`, `cargo test`, `npm run build`,
  `graft build` + `graft check`, budgets via `scripts/measure-budgets.ps1`
  (takes the keyboard; warn the user first) recorded in `docs/budgets.md`.

## Windows/Tauri gotchas learned so far

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
