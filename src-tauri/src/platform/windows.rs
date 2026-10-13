//! Windows capture backend: UI Automation first, clipboard fallback second.
//! Follows docs/PLAN.md §4 and the `windows-text-capture` skill.

use std::ffi::c_void;
use std::thread;
use std::time::{Duration, Instant};

use ::windows::Win32::Foundation::{
    CloseHandle, E_ACCESSDENIED, GlobalFree, HANDLE, HGLOBAL, HWND,
};
use ::windows::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use ::windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use ::windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use ::windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use ::windows::Win32::System::Ole::CF_UNICODETEXT;
use ::windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use ::windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetForegroundWindow, GetWindowThreadProcessId, HWND_MESSAGE,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};
use ::windows::core::{Error as WinError, PCWSTR, w};

use super::run_with_timeout;
use crate::core::capture::{
    CaptureError, CaptureMethod, Captured, SelectionProvider, selection_text,
};

/// Max wait for the user to release the hotkey's modifier keys.
const MODIFIER_RELEASE_WAIT: Duration = Duration::from_millis(400);
/// Max wait for the target app to put the copied selection on the clipboard.
const COPY_WAIT: Duration = Duration::from_millis(500);
/// Max wait for `CF_UNICODETEXT` once the clipboard changed.
const TEXT_READY_WAIT: Duration = Duration::from_millis(300);
const POLL_STEP: Duration = Duration::from_millis(10);
const CLIPBOARD_OPEN_ATTEMPTS: u32 = 10;

pub struct WindowsSelectionProvider;

impl SelectionProvider for WindowsSelectionProvider {
    fn capture(&self, timeout: Duration) -> Result<Captured, CaptureError> {
        let deadline = Instant::now() + timeout;

        // A hung UIA provider must not eat the whole budget: the fallback
        // still needs time after it.
        match run_with_timeout(timeout / 3, uia_selection) {
            Ok(UiaOutcome::Text(text)) => {
                return Ok(Captured {
                    text,
                    method: CaptureMethod::Uia,
                });
            }
            Ok(UiaOutcome::Password) => return Err(CaptureError::NothingSelected),
            Ok(UiaOutcome::NoSelection | UiaOutcome::NoTextPattern) | Err(_) => {}
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CaptureError::Timeout);
        }
        capture_via_clipboard(remaining)
    }
}

/// The clipboard fallback alone. Public so the capture probe can exercise
/// it in apps where UI Automation already succeeds.
pub fn capture_via_clipboard(timeout: Duration) -> Result<Captured, CaptureError> {
    // SendInput into an elevated window is dropped silently (UIPI), so
    // detect it up front instead of reporting "nothing selected".
    if blocked_by_uipi() {
        return Err(CaptureError::AccessDenied);
    }
    run_with_timeout(timeout, clipboard_selection)
}

/// Current `CF_UNICODETEXT` content, if any. Used by the capture probe to
/// verify that the clipboard was restored.
pub fn clipboard_text() -> Option<String> {
    let window = MessageWindow::create().ok()?;
    let _open = ClipboardLock::open(window.0).ok()?;
    read_unicode_text()
}

fn os_error(error: WinError) -> CaptureError {
    if error.code() == E_ACCESSDENIED {
        CaptureError::AccessDenied
    } else {
        CaptureError::Platform(error.message())
    }
}

// ---------------------------------------------------------------------------
// UI Automation
// ---------------------------------------------------------------------------

enum UiaOutcome {
    Text(String),
    /// The element supports TextPattern but nothing visible is selected.
    NoSelection,
    /// The focused element does not expose TextPattern at all.
    NoTextPattern,
    Password,
}

/// UI Automation alone, with the reason it failed. Public so the capture
/// probe can show why an app falls back to the clipboard.
pub fn capture_via_uia(timeout: Duration) -> Result<Captured, CaptureError> {
    match run_with_timeout(timeout, uia_selection)? {
        UiaOutcome::Text(text) => Ok(Captured {
            text,
            method: CaptureMethod::Uia,
        }),
        UiaOutcome::NoSelection | UiaOutcome::Password => Err(CaptureError::NothingSelected),
        UiaOutcome::NoTextPattern => Err(CaptureError::Platform(
            "the focused element does not support UIA TextPattern".into(),
        )),
    }
}

/// Initializes COM (MTA) for the current thread; uninitializes on drop.
struct ComApartment;

impl ComApartment {
    fn init_mta() -> Result<Self, CaptureError> {
        // SAFETY: called once on a thread we own; balanced by Drop.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(os_error)?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: pairs with the successful CoInitializeEx in `init_mta`.
        unsafe { CoUninitialize() };
    }
}

fn uia_selection() -> Result<UiaOutcome, CaptureError> {
    let _com = ComApartment::init_mta()?;
    // SAFETY: plain COM calls on interfaces created and dropped on this
    // thread, inside the apartment held by `_com`.
    unsafe {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(os_error)?;
        let element = automation.GetFocusedElement().map_err(os_error)?;
        if element.CurrentIsPassword().is_ok_and(|flag| flag.as_bool()) {
            return Ok(UiaOutcome::Password);
        }
        let Ok(pattern) =
            element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
        else {
            return Ok(UiaOutcome::NoTextPattern);
        };
        let ranges = pattern.GetSelection().map_err(os_error)?;
        let count = ranges.Length().map_err(os_error)?;
        let mut parts = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for index in 0..count {
            let range = ranges.GetElement(index).map_err(os_error)?;
            parts.push(range.GetText(-1).map_err(os_error)?.to_string());
        }
        Ok(selection_text(parts).map_or(UiaOutcome::NoSelection, UiaOutcome::Text))
    }
}

// ---------------------------------------------------------------------------
// Elevation (UIPI)
// ---------------------------------------------------------------------------

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by us and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn process_is_elevated(process: HANDLE) -> Option<bool> {
    let mut token = HANDLE::default();
    // SAFETY: `token` is a valid out-pointer; the handle is wrapped right after.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.ok()?;
    let token = OwnedHandle(token);
    let mut elevation = TOKEN_ELEVATION::default();
    let mut written = 0u32;
    // SAFETY: the buffer is a TOKEN_ELEVATION and its exact size is passed.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some(std::ptr::from_mut(&mut elevation).cast::<c_void>()),
            u32::try_from(size_of::<TOKEN_ELEVATION>()).ok()?,
            &mut written,
        )
    }
    .ok()?;
    Some(elevation.TokenIsElevated != 0)
}

fn foreground_is_elevated() -> Option<bool> {
    // SAFETY: no preconditions; may return a null HWND, checked below.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out-pointer.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    // SAFETY: query-only access; the handle is wrapped and closed on drop.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    process_is_elevated(OwnedHandle(process).0)
}

/// True when the foreground app is elevated and we are not.
fn blocked_by_uipi() -> bool {
    // SAFETY: returns a pseudo-handle that needs no closing.
    let current = unsafe { GetCurrentProcess() };
    foreground_is_elevated() == Some(true) && process_is_elevated(current) == Some(false)
}

// ---------------------------------------------------------------------------
// Clipboard fallback
// ---------------------------------------------------------------------------

/// Message-only window that owns our clipboard writes: with a null owner,
/// `EmptyClipboard` makes every later `SetClipboardData` fail.
struct MessageWindow(HWND);

impl MessageWindow {
    fn create() -> Result<Self, CaptureError> {
        // SAFETY: creates a message-only STATIC window on this thread; it is
        // destroyed on the same thread in Drop.
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map(Self)
        .map_err(os_error)
    }
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: the window was created on this thread and is destroyed once.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// Holds the clipboard open; closes it on drop.
struct ClipboardLock;

impl ClipboardLock {
    fn open(owner: HWND) -> Result<Self, CaptureError> {
        let mut last_error = None;
        for _ in 0..CLIPBOARD_OPEN_ATTEMPTS {
            // SAFETY: `owner` is a live window on this thread.
            match unsafe { OpenClipboard(Some(owner)) } {
                Ok(()) => return Ok(Self),
                Err(error) => last_error = Some(error),
            }
            thread::sleep(POLL_STEP);
        }
        Err(last_error.map_or(CaptureError::Timeout, os_error))
    }
}

impl Drop for ClipboardLock {
    fn drop(&mut self) {
        // SAFETY: we opened the clipboard in `open`.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Saved clipboard formats as raw HGLOBAL bytes.
struct Snapshot(Vec<(u32, Vec<u8>)>);

/// Formats whose data is not an HGLOBAL (GDI handles, owner-display,
/// private ranges) cannot be copied byte-wise and are skipped.
fn should_snapshot(format: u32) -> bool {
    const CF_BITMAP: u32 = 2;
    const CF_METAFILEPICT: u32 = 3;
    const CF_PALETTE: u32 = 9;
    const CF_ENHMETAFILE: u32 = 14;
    const CF_OWNERDISPLAY: u32 = 0x80;
    const CF_DSPBITMAP: u32 = 0x82;
    const CF_DSPMETAFILEPICT: u32 = 0x83;
    const CF_DSPENHMETAFILE: u32 = 0x8E;
    const CF_PRIVATE_RANGE: std::ops::RangeInclusive<u32> = 0x200..=0x2FF;
    const CF_GDIOBJ_RANGE: std::ops::RangeInclusive<u32> = 0x300..=0x3FF;

    !matches!(
        format,
        CF_BITMAP
            | CF_METAFILEPICT
            | CF_PALETTE
            | CF_ENHMETAFILE
            | CF_OWNERDISPLAY
            | CF_DSPBITMAP
            | CF_DSPMETAFILEPICT
            | CF_DSPENHMETAFILE
    ) && !CF_PRIVATE_RANGE.contains(&format)
        && !CF_GDIOBJ_RANGE.contains(&format)
}

fn copy_global(handle: HGLOBAL) -> Option<Vec<u8>> {
    // SAFETY: `handle` comes from GetClipboardData while the clipboard is
    // open; we only read `size` bytes between GlobalLock and GlobalUnlock.
    unsafe {
        let size = GlobalSize(handle);
        if size == 0 {
            return None;
        }
        let data = GlobalLock(handle);
        if data.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec();
        let _ = GlobalUnlock(handle);
        Some(bytes)
    }
}

fn take_snapshot(owner: HWND) -> Result<Snapshot, CaptureError> {
    let _open = ClipboardLock::open(owner)?;
    let mut formats = Vec::new();
    let mut format = 0;
    loop {
        // SAFETY: the clipboard is open (held by `_open`).
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }
        if !should_snapshot(format) {
            continue;
        }
        // SAFETY: the clipboard is open; failures (e.g. delayed rendering
        // that fails) just skip the format.
        if let Ok(handle) = unsafe { GetClipboardData(format) }
            && let Some(bytes) = copy_global(HGLOBAL(handle.0))
        {
            formats.push((format, bytes));
        }
    }
    Ok(Snapshot(formats))
}

fn set_global(format: u32, bytes: &[u8]) {
    // SAFETY: we allocate, fill and hand the block to the clipboard, which
    // takes ownership on success; on failure we free it ourselves.
    unsafe {
        let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) else {
            return;
        };
        let data = GlobalLock(handle);
        if data.is_null() {
            let _ = GlobalFree(Some(handle));
            return;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(handle);
        if SetClipboardData(format, Some(HANDLE(handle.0))).is_err() {
            let _ = GlobalFree(Some(handle));
        }
    }
}

/// Marks the current clipboard content as transient so it stays out of
/// Win+V history and cloud clipboard.
fn mark_excluded_from_history() {
    let zero = 0u32.to_ne_bytes();
    for name in [
        w!("ExcludeClipboardContentFromMonitorProcessing"),
        w!("CanIncludeInClipboardHistory"),
        w!("CanUploadToCloudClipboard"),
    ] {
        // SAFETY: `name` is a static NUL-terminated wide string.
        let format = unsafe { RegisterClipboardFormatW(name) };
        if format != 0 {
            set_global(format, &zero);
        }
    }
}

fn restore_snapshot(owner: HWND, snapshot: &Snapshot) -> Result<(), CaptureError> {
    let _open = ClipboardLock::open(owner)?;
    // SAFETY: the clipboard is open and owned by `owner`.
    unsafe { EmptyClipboard() }.map_err(os_error)?;
    if snapshot.0.is_empty() {
        return Ok(());
    }
    for (format, bytes) in &snapshot.0 {
        set_global(*format, bytes);
    }
    mark_excluded_from_history();
    Ok(())
}

/// Puts the snapshot back when dropped, unless the clipboard never changed
/// or someone else wrote to it after we read the selection.
struct RestoreGuard {
    owner: HWND,
    snapshot: Snapshot,
    sequence_before_copy: u32,
    sequence_after_read: Option<u32>,
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        // SAFETY: no preconditions.
        let current = unsafe { GetClipboardSequenceNumber() };
        let untouched = current == self.sequence_before_copy;
        let overwritten_by_other = self
            .sequence_after_read
            .is_some_and(|after_read| after_read != current);
        if !untouched && !overwritten_by_other {
            // Best effort: if the clipboard stays locked, the copied text
            // remains on it. Nothing more can be done from a destructor.
            let _ = restore_snapshot(self.owner, &self.snapshot);
        }
    }
}

fn key_is_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: no preconditions. The high bit marks "currently down".
    let state = unsafe { GetAsyncKeyState(i32::from(key.0)) };
    state.cast_unsigned() & 0x8000 != 0
}

fn wait_for_modifier_release() {
    let started = Instant::now();
    while started.elapsed() < MODIFIER_RELEASE_WAIT
        && [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(key_is_down)
    {
        thread::sleep(POLL_STEP);
    }
}

fn key_input(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_ctrl_c() -> Result<(), CaptureError> {
    let none = KEYBD_EVENT_FLAGS(0);
    let inputs = [
        key_input(VK_CONTROL, none),
        key_input(VK_C, none),
        key_input(VK_C, KEYEVENTF_KEYUP),
        key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let size = i32::try_from(size_of::<INPUT>()).unwrap_or(i32::MAX);
    // SAFETY: `inputs` is a valid slice of fully initialized INPUT structs.
    let sent = unsafe { SendInput(&inputs, size) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(os_error(WinError::from_thread()))
    }
}

fn wait_for_sequence_change(before: u32, limit: Duration) -> bool {
    let started = Instant::now();
    loop {
        // SAFETY: no preconditions.
        if unsafe { GetClipboardSequenceNumber() } != before {
            return true;
        }
        if started.elapsed() >= limit {
            return false;
        }
        thread::sleep(POLL_STEP);
    }
}

/// Reads `CF_UNICODETEXT`; the clipboard must already be open.
fn read_unicode_text() -> Option<String> {
    let format = u32::from(CF_UNICODETEXT.0);
    // SAFETY: the caller holds the clipboard open; the data is read only
    // between GlobalLock and GlobalUnlock and never past GlobalSize.
    unsafe {
        IsClipboardFormatAvailable(format).ok()?;
        let handle = HGLOBAL(GetClipboardData(format).ok()?.0);
        let size = GlobalSize(handle);
        let data = GlobalLock(handle);
        if data.is_null() {
            return None;
        }
        let units = std::slice::from_raw_parts(data.cast::<u16>(), size / 2);
        let end = units
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(units.len());
        let text = String::from_utf16_lossy(&units[..end]);
        let _ = GlobalUnlock(handle);
        Some(text)
    }
}

/// Waits until the copied text is readable (some apps write formats in
/// several steps) and returns it.
fn read_copied_text(owner: HWND) -> Option<String> {
    let started = Instant::now();
    loop {
        if let Ok(_open) = ClipboardLock::open(owner)
            && let Some(text) = read_unicode_text()
        {
            return Some(text);
        }
        if started.elapsed() >= TEXT_READY_WAIT {
            return None;
        }
        thread::sleep(POLL_STEP);
    }
}

fn clipboard_selection() -> Result<Captured, CaptureError> {
    wait_for_modifier_release();

    let window = MessageWindow::create()?;
    let snapshot = take_snapshot(window.0)?;
    // SAFETY: no preconditions.
    let sequence_before_copy = unsafe { GetClipboardSequenceNumber() };
    // Declared after `window`, so it is dropped (and restores) first.
    let mut guard = RestoreGuard {
        owner: window.0,
        snapshot,
        sequence_before_copy,
        sequence_after_read: None,
    };

    send_ctrl_c()?;
    if !wait_for_sequence_change(sequence_before_copy, COPY_WAIT) {
        return Err(CaptureError::NothingSelected);
    }
    let text = read_copied_text(window.0);
    // SAFETY: no preconditions.
    guard.sequence_after_read = Some(unsafe { GetClipboardSequenceNumber() });
    drop(guard);

    text.and_then(|text| selection_text([text]))
        .map(|text| Captured {
            text,
            method: CaptureMethod::Clipboard,
        })
        .ok_or(CaptureError::NothingSelected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_hglobal_text_formats() {
        // CF_TEXT, CF_UNICODETEXT, CF_HDROP, CF_DIB
        for format in [1, 13, 15, 8] {
            assert!(should_snapshot(format), "format {format}");
        }
    }

    #[test]
    fn snapshots_registered_formats() {
        // Registered formats ("HTML Format", "Rich Text Format") live in 0xC000..=0xFFFF.
        assert!(should_snapshot(0xC0A5));
    }

    #[test]
    fn skips_gdi_handle_formats() {
        for format in [
            2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E, 0x200, 0x2FF, 0x300, 0x3FF,
        ] {
            assert!(!should_snapshot(format), "format {format:#x}");
        }
    }
}
