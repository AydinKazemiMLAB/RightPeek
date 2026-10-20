# RightPeek design

RightPeek is a small tray app: select text in any app, press a hotkey, and a
window shows that text laid out correctly right-to-left. Stack: **Tauri 2 +
Rust** backend, **vanilla TypeScript + Vite** frontend. Platforms: Windows,
Linux and macOS (Apple Silicon).

## Goals

- Grab the current selection from any app with one hotkey and show it in an
  RTL viewer.
- Feel instant; cost almost nothing while idle.
- Keep platform code isolated, so the shared logic never touches the OS.

**Non-goals:** entries in other apps' right-click menus on Windows and Linux
(not possible system-wide; macOS has a Services entry), a browser extension,
a floating button near the cursor, translation, OCR, AI features, cloud sync,
and reading windows that run as administrator on Windows.

## Budgets

| Metric | Target |
|---|---|
| Idle RAM, no window open (app process only) | ≤ 20 MB private working set |
| RAM with the viewer open (including WebView processes) | ≤ 120 MB |
| Hotkey → text visible (warm) | ≤ 300 ms |
| Hotkey → text visible (cold webview) | ≤ 600 ms |
| Installer size | ≤ 8 MB |
| CPU while idle | ~0% (no polling loops) |

Measured figures per platform are in [`budgets.md`](budgets.md).

## Architecture

```
┌────────────────────── Tauri app (Rust) ───────────────────────┐
│ app/        tray, hotkey, single-instance, autostart, settings │
│   │  calls                                                      │
│   ▼                                                             │
│ core/       pure logic: text normalize, direction detection,    │
│             settings model  (no OS calls → fully unit-tested)   │
│   ▲                                                             │
│   │ trait SelectionProvider                                     │
│ platform/   windows.rs (UI Automation + clipboard fallback)     │
│             linux.rs (PRIMARY selection)                        │
│             macos.rs (Accessibility + Cmd+C fallback, Services) │
└───────────────┬─────────────────────────────────────────────────┘
                │ invoke("take_pending_text")  (pull, no race)
┌───────────────▼──────────── Viewer (WebView) ──────────────────┐
│ index.html + main.ts + styles.css — no framework                │
│ paragraphs rendered with dir from word majority + bidi isolate  │
└─────────────────────────────────────────────────────────────────┘
```

- **Tray-resident, windowless when idle.** The viewer is created on demand
  and destroyed on close (not hidden); the process stays alive in the tray.
- **Pull, don't push.** Rust keeps the captured text in managed state and the
  page calls `take_pending_text` when it loads, which avoids
  event-before-listener races.
- **Platform trait.** `SelectionProvider::capture(timeout)` returns
  `Captured { text, method }` or a `CaptureError` with a user-facing title
  and hint. Every capture runs on a worker thread with a hard timeout.
- **Second launch** goes through the single-instance plugin: `--show`
  captures, `--autostart` (the sign-in entry) does nothing, anything else
  opens Settings.
- **No low-level input hooks** (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`): on Windows
  and macOS the hotkey uses the Tauri global-shortcut plugin; on Linux
  `platform::x11_hotkey` grabs it on the X11 root window and blocks on X
  events (the plugin's X11 backend polls every 50 ms). Tauri capabilities are
  kept to what each window needs.

Layout: `src/` (viewer and Settings pages), `src-tauri/src/core/` (pure logic,
unit-tested), `src-tauri/src/platform/` (OS backends), `src-tauri/src/app/`
(tray, hotkey, windows, settings), `src-tauri/src/lib.rs` (wiring).

## Capture per platform

### Windows

1. **UI Automation:** focused element → `TextPattern` → selection → text of
   each range, on a COM (MTA) worker thread.
2. **Clipboard fallback** when UI Automation has no text:
   1. wait until the hotkey's modifier keys are released (otherwise Ctrl+C
      becomes e.g. Ctrl+Shift+C);
   2. snapshot the clipboard (text, HTML, RTF, file lists; others best-effort)
      and its sequence number;
   3. send Ctrl+C with `SendInput` and wait for the sequence number to change
      (≤ 500 ms; no change → "nothing selected");
   4. read the text and restore the snapshot, marked so it stays out of the
      Win+V history and cloud clipboard.
3. **Limits:** windows that run as administrator (UIPI), apps that block
   copying, delayed-rendered clipboard formats.

### Linux

The X11/Wayland **PRIMARY selection** is read directly: no copy is simulated
and the clipboard is never touched. Wayland compositors without the
data-control protocol fall back to `wl-paste --primary`.

The hotkey is a passive key grab on the X11 root window (`GrabKey`, also
with Caps Lock and Num Lock on; XKB detectable auto-repeat so a held key
captures once), served by a thread that waits for X events. On Wayland it
goes through Xwayland and only fires while an X11 window has focus, so users
bind a desktop shortcut to `rightpeek --show`.

### macOS

1. **Accessibility:** focused element → `AXSelectedText`.
2. **Cmd+C fallback** for apps without Accessibility text (browsers,
   Electron): modifiers released first, the pasteboard snapshot is restored
   and marked transient.
3. **Services menu:** "Show in RightPeek" receives the selected text from the
   app itself and needs no permission.

Both 1 and 2 need the Accessibility permission; Settings shows its status.
RightPeek is a menu-bar app without a Dock icon.

Per-app results: [`capture-matrix.md`](capture-matrix.md).

## Viewer (RTL rules)

- Text is split into paragraphs; each paragraph's `dir` comes from the
  majority of its words (tie → RTL), not from the first strong character:
  Persian technical text often starts with a Latin term.
- CSS: `unicode-bidi: isolate` per paragraph, `text-align: start`, page
  default `dir="rtl"`.
- Numbers after a Latin word: the bidi algorithm glues a European number
  (ASCII or Persian digits) to the previous strong character (UAX #9, rule
  W7), so "CPU، ۲ گیگابایت" reads "۲ ،CPU گیگابایت" in an RTL paragraph.
  `core::text::detached_numbers` marks runs separated from the Latin word by
  punctuation and not followed by Latin text; the viewer wraps each in a
  span that is an RTL isolate inside RTL paragraphs only. "Windows 11"
  (space only) and "CPU, 2 GB" (Latin after) are left alone. No characters
  are inserted, so Copy and selections stay clean.
- Bundled **Vazirmatn** font (Latin and Arabic subsets); no network fonts.
- Normalization: CRLF → LF; zero-width characters removed except the ZWNJ
  (U+200C, needed in Persian); trailing spaces trimmed; Arabic presentation
  forms (common in PDF text) turned back into base letters with NFKC applied
  to those characters only.
- Controls: Copy, Edit, font size, force RTL/LTR, Justify; Esc closes. Size
  and position are remembered.
- Edit is plain-text editing in place (`contenteditable="plaintext-only"`),
  not an editor: Enter adds a line break inside the paragraph. Leaving edit
  mode (the button or Esc) sends the text through `prepare_text`, the same
  normalize/paragraph/direction path as a capture, and renders the result.
  Edits live in the window only; "Show last" keeps the capture.
- Theme: System (default, `prefers-color-scheme`), Light or Dark, chosen in
  Settings. The window theme colors the native frame; pages also set
  `data-theme` so the choice holds where the webview ignores the window
  theme. An open viewer follows a change through the `rightpeek://theme`
  event.

## Packaging

Windows: per-user NSIS installer and a portable exe. Linux: `.deb` and
`.rpm` built on Ubuntu 22.04. macOS: `.app` in a `.dmg`, ad-hoc signed. The
release process is in [`release.md`](release.md); building and testing are in
[`development.md`](development.md).
