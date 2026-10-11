# RightPeek — Project Plan

> Working name: **RightPeek**.
> A tiny tray app: the user selects text anywhere, presses a hotkey (later: right-click in the browser), and a window opens showing that text laid out correctly right-to-left.
> Stack: **Tauri 2 + Rust** backend, **vanilla TypeScript + Vite** frontend. Windows first; macOS/Linux later.

---

## 1. How to use this file

1. Put this file at the repo root as `PLAN.md`.
2. Run the setup commands in §6 and §7 once (skills + graft).
3. Start a coding-agent session (Claude Code) and paste the **Kickoff Prompt** from §11.
4. At the end of each phase, review, then send the **Next-Phase Prompt** (§11).

---

## 2. Goals, non-goals, budgets

**Goals**
- Grab the current selection from any app with one hotkey and show it in an RTL viewer.
- Feel instant; cost almost nothing while idle.
- Keep platform code isolated so macOS/Linux can be added without touching the core.

**Non-goals (for v1)**
- Injecting items into other apps' right-click menus (not possible system-wide on Windows).
- Translation, OCR, AI features, cloud sync.
- Admin/elevated target windows (UIPI blocks input from a normal process). Document it; don't work around it.

**Budgets (measure every phase from Phase 2 on)**
| Metric | Target |
|---|---|
| Idle RAM, no window open (app process only) | ≤ 20 MB private working set |
| RAM with viewer open (incl. WebView2 processes) | ≤ 120 MB |
| Hotkey → text visible (warm) | ≤ 300 ms |
| Hotkey → text visible (cold webview) | ≤ 600 ms |
| Installer size | ≤ 8 MB |
| CPU while idle | ~0% (no polling loops) |

---

## 3. Architecture

```
┌────────────────────── Tauri app (Rust) ───────────────────────┐
│ app/        tray, hotkey, single-instance, autostart, settings │
│   │  calls                                                      │
│   ▼                                                             │
│ core/       pure logic: text normalize, direction detection,    │
│             settings model  (no OS calls → fully unit-tested)   │
│   ▲                                                             │
│   │ trait SelectionProvider                                     │
│ platform/   windows.rs  (UIA + clipboard fallback)              │
│             macos.rs / linux.rs   (Phase 7)                     │
└───────────────┬─────────────────────────────────────────────────┘
                │ invoke("take_pending_text")  (pull, no race)
┌───────────────▼──────────── Viewer (WebView) ──────────────────┐
│ index.html + main.ts + style.css — no framework                 │
│ paragraphs rendered with dir="auto" + unicode-bidi: plaintext   │
└─────────────────────────────────────────────────────────────────┘
```

Key decisions:
- **Tray-resident, windowless when idle.** Create the viewer with `WebviewWindowBuilder` on demand; **destroy** it on close (not hide). Keep the process alive with `RunEvent::ExitRequested { api, .. } => api.prevent_exit()`.
- **Pull, don't push.** Rust stores the captured text in managed state; the page calls `take_pending_text` on load. Avoids event-before-listener races.
- **Platform trait.** `trait SelectionProvider { fn capture(&self, timeout: Duration) -> Result<Captured, CaptureError>; }` with `Captured { text, method: Uia | Clipboard }`.
- **Second launch / extension input** goes through the single-instance plugin (args forwarded to the running instance).
- **Frontend has no framework.** Vanilla TS keeps the bundle tiny.

### Repo layout
```
/
├─ PLAN.md
├─ CLAUDE.md                     # short: conventions + pointer to PLAN.md
├─ .claude/skills/windows-text-capture/SKILL.md   # custom skill (Phase 0)
├─ src/                          # frontend: index.html, main.ts, style.css, fonts/
├─ src-tauri/
│  ├─ src/main.rs, lib.rs        # app wiring only
│  ├─ src/core/                  # pure logic + tests
│  ├─ src/platform/{mod,windows}.rs
│  ├─ capabilities/              # Tauri v2 permissions (least privilege)
│  └─ tauri.conf.json
└─ extension/                    # Phase 5 (MV3 + native messaging host)
```

---

## 4. Windows text-capture spec (source for the custom skill)

Order of attempts, each on a worker thread with a hard timeout (UIA can hang on unresponsive apps):

1. **UI Automation** — `windows` crate, `Win32_UI_Accessibility`.
   - COM init (MTA) on a dedicated thread → `IUIAutomation::GetFocusedElement` → `GetCurrentPattern(UIA_TextPatternId)` → `GetSelection()` → each range `GetText(-1)`.
   - Empty or unsupported → fall back.
2. **Clipboard fallback**
   1. Wait until the user has **released all modifier keys** of the hotkey (`GetAsyncKeyState` poll, max ~400 ms). Otherwise Ctrl+C becomes e.g. Ctrl+Shift+C.
   2. Snapshot the clipboard (at least `CF_UNICODETEXT`, HTML, RTF, `CF_HDROP`; best-effort for others) and record `GetClipboardSequenceNumber()`.
   3. `SendInput` Ctrl down, C down/up, Ctrl up.
   4. Wait for the sequence number to change (poll ≤ 500 ms). No change → "nothing selected".
   5. Read `CF_UNICODETEXT`.
   6. Restore the snapshot. When writing it back, also set the registered formats `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory` (= 0) so the restore doesn't pollute Win+V history or cloud clipboard.
3. **Known limits:** elevated target windows (UIPI), apps that block copy, delayed-rendered clipboard formats (may not restore perfectly). Surface a clear message; never crash.

Hotkey: use the Tauri global-shortcut plugin (RegisterHotKey-based). **Never** install `WH_KEYBOARD_LL` / `WH_MOUSE_LL` hooks in v1 (AV false positives).

---

## 5. RTL rendering rules (viewer)

- Split text into paragraphs on blank lines/newlines; each paragraph is a `<p dir="auto">`.
- CSS: `unicode-bidi: plaintext;` per paragraph; `text-align: start;` page default `dir="rtl"`.
- Bundle **Vazirmatn** (woff2, Latin+Arabic subsets only) locally; no network fonts.
- Normalize: CRLF→LF, strip zero-width junk except ZWNJ (U+200C, needed in Persian), trim trailing spaces.
- Controls: Copy, Close (Esc), font size −/+, toggle RTL/LTR override. Remember size/position (window-state).
- Respect system light/dark theme.

---

## 6. Skills to install (Claude Code, project scope)

Review each `SKILL.md` before trusting it; these are third-party.

```bash
# 1) Tauri 2 foundations: commands/invoke, capabilities & permissions, IPC, troubleshooting
npx skills add https://github.com/nodnarbnitram/claude-code-extensions --skill tauri-v2 -a claude-code -y

# 2) Only the Tauri plugin skills this app needs
npx skills add full-stack-skills/tauri-skills -a claude-code -y \
  --skill tauri-app-global-shortcut --skill tauri-app-system-tray \
  --skill tauri-app-clipboard      --skill tauri-app-single-instance \
  --skill tauri-app-autostart      --skill tauri-window \
  --skill tauri-app-store          --skill tauri-security \
  --skill tauri-build

# 3) Idiomatic Rust
npx skills add https://github.com/leonardomso/rust-skills --skill rust-skills -a claude-code -y

# 4) Optional — RTL/bidi web patterns (Hebrew-focused; pick only the RTL skill)
npx skills add https://github.com/skills-il/localization --list

# 5) Phase 5 only — Chrome extension (MV3)
npx skills add https://github.com/samber/cc-skills --skill chrome-extension -a claude-code -y
```

Do **not** install: `full-statck-skills/*` (misspelled look-alike repo), `cruldra/skills tauri-v2` (failed a security audit).

**Custom skill (written in Phase 0):** `.claude/skills/windows-text-capture/SKILL.md`, built from §4. No public skill covers this.

---

## 7. Token economy with Graft

[Graft](https://github.com/NanoNets/Graft) (`@nanonets/graft`) builds a markdown graph of the codebase plus an MCP server, so the agent stops re-reading files to orient itself. TypeScript is full-fidelity; Rust is in its "broad" tier.

Setup (run **after** Phase 0 scaffolding, when there is code to index):
```bash
npm install -g @nanonets/graft
graft init --dry-run      # see which files it will touch
graft init                # builds graft/, writes .mcp.json, .claude/ hooks + skill; does not touch CLAUDE.md
```

Agent rules (also in the Kickoff Prompt):
- Orient with `graft_repo_map` / `graft map`, never by listing and reading whole directories.
- Find code with `graft_find_code` / `graft ask "<task>"` before any grep or full-file read.
- Use `graft_file_api` / `graft skeleton <file>` before opening a file; read only the needed line ranges.
- Before changing a symbol, run `graft_trace_calls` / `graft callers <symbol>`.
- End of each phase: `graft check` (freshness) and `graft blast` on the diff.
- Never read the `graft/` folder wholesale.
- General: don't echo file contents back in chat; short status updates; summarize test output instead of pasting it.

---

## 8. Phases

Each phase ends with: tests green, `cargo clippy -D warnings` clean, budgets measured (from Phase 2), a short Persian summary to the user, and a **stop for approval**.

### Phase 0 — Setup & scaffolding
- `npm create tauri-app@latest` → vanilla TS template; Tauri 2.
- Install skills (§6); write `CLAUDE.md` (≤ 40 lines: conventions, language rules, pointer to PLAN.md).
- Write the custom skill `windows-text-capture` from §4.
- Create `core/` and `platform/` modules + `SelectionProvider` trait (stub).
- `graft init` (§7). Add GitHub Actions: `cargo fmt --check`, `clippy`, `cargo test`, `npm run build` on `windows-latest`.
- **Done when:** app builds and runs an empty window; CI green; graft MCP visible in the session.

### Phase 1 — Capture spike (riskiest part first)
- Implement `platform/windows.rs`: UIA path + clipboard fallback exactly per §4.
- Small dev-only CLI (`cargo run --bin capture-probe`) that waits 3 s, captures, and prints method + text.
- Test matrix (record results in `docs/capture-matrix.md`): Chrome, Edge, Firefox, Word, Notepad, VS Code, Windows Terminal, Telegram Desktop, a PDF reader, Outlook, File Explorer rename box.
- **Done when:** ≥ 9/11 apps return correct text; clipboard verifiably restored; nothing appears in Win+V history.

### Phase 2 — MVP
- Tray icon + menu (Show last, Settings, Quit). Global hotkey (default `Ctrl+Alt+Space`, configurable).
- Single instance; autostart toggle; settings via store plugin.
- Viewer window created on demand, destroyed on close; RTL rendering per §5; bundled font.
- Least-privilege capabilities in `src-tauri/capabilities/`.
- Core unit tests: paragraph split, normalization, ZWNJ preserved, direction detection.
- **Done when:** full flow works end-to-end; budgets in §2 measured and reported.

### Phase 3 — Hardening & polish
- Clear UX for: nothing selected, elevated window, capture timeout.
- Hotkey conflict detection + UI to rebind. Window-state persistence. Light/dark.
- Logging (rotating file, off by default in release). Optional "pre-warm viewer" setting if cold start misses budget.
- **Done when:** no panics in a 30-minute manual soak; budgets still met.

### Phase 4 — Packaging & release
- Tauri bundler: NSIS installer (+ MSI optional). WebView2 bootstrapper config.
- **Code signing** (reduces SmartScreen/AV flags — important for an app that sends keystrokes and reads the clipboard).
- Optional updater plugin with signing keys. Release workflow in CI.
- **Done when:** signed installer from CI installs/uninstalls cleanly on a fresh Windows 10 and 11 VM.

### Phase 5 — Browser right-click (Chrome/Edge)
- MV3 extension: `contextMenus` with `contexts: ["selection"]` → native messaging → host binary (a tiny mode of the app or separate exe) → forwards text to the running instance.
- Register native-messaging host in the installer.
- **Done when:** right-click → "Show RTL" opens the viewer with long texts (≥ 100 KB) intact.

### Phase 6 — Optional floating button
- Opt-in only. Prefer UIA `TextSelectionChanged` events over mouse hooks. Show a small button near the cursor after a selection; click → same flow.
- **Done when:** no measurable idle CPU increase; can be fully disabled.

### Phase 7 — Cross-platform
- **macOS:** Services menu entry (true right-click on selected text in all apps) + Accessibility `AXSelectedText` for the hotkey; permission onboarding; notarization.
- **Linux X11:** read PRIMARY selection directly. **Wayland:** GlobalShortcuts portal or DE-bound shortcut running `app --show`; `wl-paste --primary` fallback.
- `core/` must not change; only `platform/*` and bundling.

---

## 9. Conventions
- Chat with the user: **Persian**. Code, identifiers, comments, commit messages, docs inside the repo: **English**.
- Small commits, Conventional Commits style. One phase = one branch/PR.
- Pure logic in `core/` gets tests first (TDD). Platform code gets the probe/matrix instead.
- No new dependency without one line of justification in the PR description.

## 10. Risks
| Risk | Mitigation |
|---|---|
| UIA unsupported in many apps | Clipboard fallback; matrix in Phase 1 decides early |
| Antivirus flags (SendInput + clipboard) | No low-level hooks, code signing, clear description |
| Clipboard restore imperfect | Snapshot common formats; document limits |
| Cold WebView2 start too slow | Measure; optional pre-warm setting |
| Wayland restrictions | Portal/DE shortcut path; document |

---

## 11. Prompts

### Next-Phase Prompt

```text
Phase <N> is approved. Re-read only PLAN.md §8 "Phase <N+1>" and anything it
references, then start it. Same rules as before: Persian in chat, English in the
repo, graft-first navigation, stop for approval at the end of the phase.
```

### Fix/Change Prompt

```text
Issue: <describe in one or two lines>.
Locate the relevant code with graft (graft_find_code / graft_trace_calls), propose
the smallest fix in Persian (2–4 lines), then implement it with a test if it touches
core/. Do not refactor unrelated code.
```
