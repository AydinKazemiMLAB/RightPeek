---
name: windows-text-capture
description: Use when implementing, debugging, or reviewing RightPeek's Windows selection capture (src-tauri/src/platform/windows.rs, the capture-probe binary, docs/capture-matrix.md) - UI Automation TextPattern, SendInput Ctrl+C clipboard fallback, clipboard snapshot/restore, Win+V history pollution, UIPI/elevated windows, capture hangs or timeouts.
---

# Windows text capture (RightPeek)

## Overview
Read the focused app's selection without the user noticing: UI Automation first,
simulated Ctrl+C second, clipboard always restored. Source spec: docs/PLAN.md §4.
PLAN.md wins over this file if they ever disagree.

## Contract
Implement `core::capture::SelectionProvider` in `platform/windows.rs`:
`fn capture(&self, timeout: Duration) -> Result<Captured, CaptureError>`.
Return `Captured { text, method: CaptureMethod::Uia | Clipboard }`. Map failures:

| Situation | `CaptureError` |
|---|---|
| UIA empty/unsupported and clipboard sequence never changed | `NothingSelected` |
| Worker missed its deadline | `Timeout` |
| Target is elevated (UIPI: `SendInput` silently dropped, UIA `E_ACCESSDENIED`) | `AccessDenied` |
| Any other HRESULT / Win32 error | `Platform(msg)` |

Never panic; never `unwrap()` an OS call.

## Sequence
Every step runs on a worker thread; the caller waits on a channel with the overall
`timeout` and abandons a hung worker (do not reuse its COM objects).

1. **UIA** - worker: `CoInitializeEx(None, COINIT_MULTITHREADED)` ->
   `CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)` ->
   `GetFocusedElement` -> `GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)` ->
   `GetSelection` -> each range `GetText(-1)`; join. Empty / pattern missing -> step 2.
   If the element has `UIA_IsPasswordPropertyId` = true, stop: return `NothingSelected`
   (never copy from password fields).
2. **Wait for modifier release** - poll `GetAsyncKeyState` for Ctrl, Alt, Shift, Win
   (~10 ms steps, max ~400 ms). Otherwise Ctrl+C becomes Ctrl+Alt+C etc.
3. **Snapshot** - `GetClipboardSequenceNumber()`, then save at least
   `CF_UNICODETEXT`, `"HTML Format"`, `"Rich Text Format"`, `CF_HDROP`; other
   HGLOBAL formats best-effort. Skip GDI-handle formats (`CF_BITMAP`, `CF_ENHMETAFILE`, ...).
4. **Copy** - one `SendInput` with 4 `INPUT`s: Ctrl down, C down, C up, Ctrl up.
5. **Wait** - poll the sequence number (≤ 500 ms). No change -> `NothingSelected`.
6. **Read** - `OpenClipboard` (retry a few times, it is contended) -> `CF_UNICODETEXT`.
7. **Restore** - `EmptyClipboard`, write the snapshot back, and also set the
   registered formats `ExcludeClipboardContentFromMonitorProcessing` and
   `CanIncludeInClipboardHistory` (DWORD 0) so nothing lands in Win+V or cloud clipboard.
   If the sequence number moved past our own copy, someone else wrote the
   clipboard - do not overwrite it.

Make restore a drop guard created right after the snapshot, so every early return
and error path restores. If `OpenClipboard` still fails after retries during
restore, keep the captured text as the result and log the failure; formats that
could not be snapshotted are simply lost (document in the matrix).

## Hard rules (project decisions)
- Hotkey comes from the Tauri global-shortcut plugin. Do not call `RegisterHotKey` yourself.
- Never install `WH_KEYBOARD_LL` / `WH_MOUSE_LL` hooks.
- Elevated targets are a non-goal: return `AccessDenied` with a clear message. No
  `uiAccess=true` manifests, no self-elevation, no workaround.
- No polling loops outside an active capture (idle CPU must stay ~0%).

## Rust gotchas
- The module `platform::windows` has the same name as the `windows` crate: anywhere
  under `platform/`, write `use ::windows::Win32::...` with the leading `::`.
- Crate features needed: `Win32_Foundation`, `Win32_System_Com`,
  `Win32_UI_Accessibility`, `Win32_System_DataExchange`, `Win32_System_Memory`,
  `Win32_System_Ole`, `Win32_UI_Input_KeyboardAndMouse`. Adding the crate needs user approval.
- COM objects are `!Send`: create and drop them on the worker thread.

## Known risks to verify in the Phase 1 matrix
- Terminals: Ctrl+C with no selection sends SIGINT to the running program. No
  mitigation is decided yet; if the matrix confirms it, propose one (e.g. skip the
  fallback for console window classes) to the user before implementing.
- Chromium/Electron expose TextPattern only once accessibility is active; expect
  clipboard fallback there.
- Delay-rendered formats may not restore exactly; document, don't fight.

## Validation
`cargo run --manifest-path src-tauri/Cargo.toml --bin capture-probe` (waits 3 s,
prints method + text). Record each app in `docs/capture-matrix.md`: app, method,
correct text (y/n), clipboard restored (y/n), Win+V clean (y/n), notes.
