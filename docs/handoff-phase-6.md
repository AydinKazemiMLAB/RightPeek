# Handoff: Phase 6 (macOS)

Everything a new agent session on the maintainer's Mac needs to start
Phase 6. Agent memory does not travel between machines, so the maintainer's
standing instructions are repeated here. Delete this file in the Phase 6 PR.

## Kickoff prompt

The maintainer pastes this into a new Claude Code session opened in the
cloned repo on the Mac:

```text
Read CLAUDE.md, docs/PLAN.md (§2 budgets, §3 architecture, §8 Phase 6) and
docs/handoff-phase-6.md, then docs/dev-notes.md. Use graft for navigation
(run `graft build` first if graft/ is missing).

Rules:
- Reply to me in Persian. Everything in the repo stays in English.
- I don't know Rust. If anything needs installing, guide me step by step and
  wait for my OK before each install.
- Do NOT start Phase 6 until I write "فاز 6 رو شروع کن".
- Never use the superpowers:* skills (too many tokens).
- Routine git and GitHub work is pre-authorized: branch phase-6, commit, push,
  PR, Auto-fix, and merge after CI is green; delete the branch after merge.
  Tags, GitHub releases, code signing, notarization and anything paid still
  need my approval. I publish releases by hand.
- Save these rules to your memory on this machine.
- At the end of the phase, run the full phase gate, give me a Persian summary
  and stop. Delete docs/handoff-phase-6.md in the Phase 6 PR.

For now: check the Mac setup in the handoff (tell me what is missing and how
to install it), then post a Persian checklist of Phase 6 and the open
questions from the handoff.
```

## Where the project stands (2026-10-14)

- Version **0.1.1** (Windows NSIS installer + portable exe, Linux `.deb`/`.rpm`).
  Phases 0–5 are done (PLAN.md §8). The repository is public, MIT licensed.
- macOS has never been built. Today it would compile against the fallbacks:
  `platform::selection_provider()` returns `Unsupported` outside Windows/Linux,
  `run_with_timeout` is gated to Windows/Linux, and sign-in autostart already
  uses `tauri-plugin-autostart` with `MacosLauncher::LaunchAgent` (every OS
  except Windows, see `src-tauri/src/app/autostart.rs` and `lib.rs`).
  `src-tauri/icons/icon.icns` exists.
- CI: `ci.yml` (fmt, clippy, tests, build on `windows-latest` and
  `ubuntu-22.04`) and `release.yml` (build + install smoke tests; files are
  kept as run artifacts, no GitHub release is created). The maintainer wants
  both kept as they are; add macOS jobs in the same style.
- Tests: 79 on Windows, 81 on Linux (`core/` is shared and test-first).
- No auto-updater (set aside by the maintainer). Code signing deferred.

## Standing decisions to respect

- Never install low-level input hooks; never widen Tauri capabilities beyond
  need; ask before any dependency not named in PLAN.md, architecture changes,
  or anything irreversible (CLAUDE.md).
- `core/` changes are additive and test-first; OS code lives in
  `platform/macos.rs`, wiring in `app/`.
- Forwarded second launches: `core::settings::second_launch` (`--show` →
  capture, `--autostart` → nothing, otherwise → Settings).
- The dev-only `capture-probe` binary needs `--features probe` (the bundler
  ships every binary target).

## Mac setup to check

Ask before installing each item; give one command per step.

1. Xcode Command Line Tools: `xcode-select -p` (install: `xcode-select --install`).
2. Rust via rustup (https://rustup.rs), stable toolchain. For a universal
   build also `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.
3. Node.js 20 and npm; then `npm ci` in the repo.
4. GitHub CLI: `gh auth login`; clone `AydinKazemiMLAB/RightPeek`.
5. graft: `npm install -g @nanonets/graft`, then `graft build` (`graft/` is
   git-ignored; `.mcp.json`, `.claude/settings.json`, the graft hooks and all
   project skills under `.claude/skills/` are committed).
6. Smoke check: `npm run tauri dev` opens Settings and puts an icon in the menu bar.

On Windows, downloads (rustup, crates) needed the maintainer's local proxy.
If downloads time out on the Mac, ask about a proxy before retrying.

## Phase 6 scope and proposals (confirm with the maintainer first)

From PLAN.md §8: Services menu entry + Accessibility `AXSelectedText` for the
hotkey, permission onboarding, notarization (Apple Developer account: ask first).

1. **Capture (`platform/macos.rs`).** Accessibility first:
   `AXUIElementCreateSystemWide` → `kAXFocusedUIElementAttribute` →
   `kAXSelectedTextAttribute` (method `Uia`-like; add a `CaptureMethod`
   variant only if needed, test-first). Fallback: wait for hotkey modifiers to
   be released, snapshot `NSPasteboard` (all items/types), post Cmd+C with
   `CGEvent`, wait for `changeCount` to change (≤ 500 ms), read text, restore
   the snapshot marked `org.nspasteboard.TransientType` /
   `org.nspasteboard.ConcealedType` so clipboard managers skip it. Run it
   under `run_with_timeout` (extend its cfg gate to macOS).
   New crates are needed (e.g. `objc2`, `objc2-app-kit`, `objc2-foundation`,
   `core-graphics`/`core-foundation` or `accessibility-sys`): **ask first**;
   Tauri already pulls some `objc2` crates in, prefer those versions.
2. **Permission onboarding.** Both AX reads and synthetic Cmd+C need the
   Accessibility permission (`AXIsProcessTrustedWithOptions`). Settings shows
   the status and a button that opens System Settings → Privacy & Security →
   Accessibility. The capture error explains it when missing.
   Gotcha: every unsigned/ad-hoc rebuild changes the code identity, so macOS
   may drop the grant; remove and re-add the app in that list.
3. **Services menu (riskiest, spike early).** `NSServices` entry in the
   bundle `Info.plist` (Tauri 2 merges `src-tauri/Info.plist`) plus a
   provider object registered with `NSApp.setServicesProvider` (objc2). The
   handler gets the selected text on a pasteboard and opens the viewer
   directly (no capture needed). Refresh during testing:
   `/System/Library/CoreServices/pbs -update`. If it cannot work cleanly in
   Tauri, report it and agree a fallback with the maintainer.
4. **App behaviour.** Menu-bar only (`ActivationPolicy::Accessory`, no Dock
   icon); tray icon as a template (monochrome) image; viewer and Settings must
   come to the front when opened; check the Settings height fit.
5. **Global hotkey.** The global-shortcut plugin works on macOS without
   Accessibility; check the default `Ctrl+Shift+Space` (macOS uses it for
   input-source switching in some setups; maybe propose `Cmd+Shift+Space` on
   macOS — ask). `--show` matters less here; `open -a` does not forward
   arguments to a running app.
6. **Autostart.** LaunchAgent via the plugin; verify the `--autostart`
   argument arrives (quiet start). There is no uninstaller on macOS; document
   removing `~/Library/LaunchAgents/RightPeek.plist` (check the real name).
7. **Bundling.** `.app` + `.dmg`. Ask: universal binary
   (`--target universal-apple-darwin`) or Apple Silicon only. Without an Apple
   Developer account ($99/year) builds are ad-hoc signed: Gatekeeper blocks
   the first open (right-click → Open, or System Settings → Open Anyway).
   Notarization only with the maintainer's approval and account.
8. **CI.** Add `macos-latest` to the `ci.yml` matrix; add `build-macos` and a
   smoke test (mount the dmg, copy the app, launch, check it keeps running,
   quit) to `release.yml`, artifacts only.
9. **Budgets (PLAN.md §2).** `scripts/measure-budgets.ps1` is Windows-only;
   measure on the Mac (e.g. `footprint`/`vmmap --summary` or `ps -o rss`,
   WebKit helper processes included for the viewer-open figure) and record a
   macOS section in `docs/budgets.md`. Installer budget ≤ 8 MB (dmg).
10. **Capture matrix.** macOS table in `docs/capture-matrix.md`: TextEdit,
    Notes, Safari, Chrome, Firefox, VS Code (Electron), Terminal, Pages or
    Word, Preview (PDF), Telegram; note which path (AX or Cmd+C) worked.
11. **Docs.** README.md + README.fa.md (macOS install, Gatekeeper, permission,
    uninstall), `docs/release.md` (macOS bundle), `docs/dev-notes.md` (a
    macOS section), PLAN.md Phase 6 ticks and decisions.

## Open questions for the maintainer

- macOS version and chip (Apple Silicon / Intel) of the test Mac.
- Universal binary or Apple Silicon only?
- Apple Developer account for signing/notarization now, or ad-hoc for now?
- Default hotkey on macOS: keep `Ctrl+Shift+Space` or use `Cmd+Shift+Space`?
- Approve the native-API crates for capture and the Services provider.

## Phase gate (CLAUDE.md)

`cargo fmt --manifest-path src-tauri/Cargo.toml --check`,
`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`,
`cargo test --manifest-path src-tauri/Cargo.toml`, `npm run build`,
`graft check`; budgets measured on the Mac and recorded in `docs/budgets.md`.
CI must stay green on Windows and Linux too.
