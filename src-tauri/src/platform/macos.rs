//! macOS selection capture and the Services menu provider.
//!
//! Order: Accessibility `AXSelectedText` of the focused element -> Cmd+C
//! fallback (wait for the hotkey modifiers to be released, snapshot the
//! pasteboard, post Cmd+C, wait for `changeCount`, read the text, restore the
//! snapshot marked transient so clipboard managers skip it). Both paths need
//! the Accessibility permission. The Services menu entry needs none: macOS
//! hands the selection over on a pasteboard.

use std::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSPasteboard, NSPasteboardItem, NSPasteboardType, NSPasteboardTypeString,
    NSUpdateDynamicServices,
};
use objc2_application_services::{
    AXError, AXIsProcessTrustedWithOptions, AXUIElement, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
};
use objc2_foundation::{NSArray, NSData, NSString};

use super::run_with_timeout;
use crate::core::capture::{
    CaptureError, CaptureMethod, Captured, SelectionProvider, selection_text,
};

/// Per-call limit for Accessibility requests to the target app.
const AX_TIMEOUT_SECS: f32 = 0.25;
/// How long the hotkey's modifiers may stay down before Cmd+C is sent anyway.
const MODIFIER_RELEASE_WAIT: Duration = Duration::from_millis(300);
/// How long the target app has to answer Cmd+C.
const COPY_WAIT: Duration = Duration::from_millis(500);
const POLL: Duration = Duration::from_millis(10);
/// `kVK_ANSI_C`.
const KEY_C: u16 = 8;
/// nspasteboard.org marker: clipboard managers do not record the restore.
const TRANSIENT_TYPE: &str = "org.nspasteboard.TransientType";
const ACCESSIBILITY_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// Whether a capture already showed the system permission prompt; later
/// refusals only show the viewer notice.
static PROMPTED: AtomicBool = AtomicBool::new(false);

pub struct MacSelectionProvider;

impl SelectionProvider for MacSelectionProvider {
    fn capture(&self, timeout: Duration) -> Result<Captured, CaptureError> {
        // The first refusal also shows the system prompt that adds RightPeek
        // to the Accessibility list.
        if !is_trusted(!PROMPTED.swap(true, Ordering::Relaxed)) {
            return Err(CaptureError::PermissionNeeded);
        }
        run_with_timeout(timeout, || match capture_via_accessibility() {
            Ok(captured) => Ok(captured),
            Err(_) => capture_via_copy(),
        })
    }
}

/// Whether RightPeek may use the Accessibility API. With `prompt`, macOS
/// shows its permission dialog once (per app identity) when it may not.
pub fn is_trusted(prompt: bool) -> bool {
    let key: &CFString = unsafe { kAXTrustedCheckOptionPrompt };
    let value: &CFType = CFBoolean::new(prompt);
    let options = CFDictionary::<CFType, CFType>::from_slices(&[key.as_ref()], &[value]);
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}

/// Opens System Settings > Privacy & Security > Accessibility.
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .arg(ACCESSIBILITY_SETTINGS)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| format!("open exited with {status}"))
        })
}

/// Reads `AXSelectedText` of the focused element (diagnostics: capture probe).
pub fn capture_via_accessibility() -> Result<Captured, CaptureError> {
    let system = unsafe { AXUIElement::new_system_wide() };
    unsafe { system.set_messaging_timeout(AX_TIMEOUT_SECS) };
    let focused = copy_attribute(&system, "AXFocusedUIElement")?
        .downcast::<AXUIElement>()
        .map_err(|_| CaptureError::Platform("focused element has an unexpected type".into()))?;
    unsafe { focused.set_messaging_timeout(AX_TIMEOUT_SECS) };
    let text = copy_attribute(&focused, "AXSelectedText")?
        .downcast::<CFString>()
        .map_err(|_| CaptureError::Platform("selected text has an unexpected type".into()))?
        .to_string();
    let text = selection_text([text]).ok_or(CaptureError::NothingSelected)?;
    Ok(Captured {
        text,
        method: CaptureMethod::Accessibility,
    })
}

fn copy_attribute(
    element: &AXUIElement,
    attribute: &'static str,
) -> Result<CFRetained<CFType>, CaptureError> {
    let name = CFString::from_static_str(attribute);
    let mut value: *const CFType = std::ptr::null();
    let error = unsafe { element.copy_attribute_value(&name, NonNull::from(&mut value)) };
    if error != AXError::Success {
        return Err(from_ax_error(attribute, error));
    }
    let value = NonNull::new(value.cast_mut()).ok_or(CaptureError::NothingSelected)?;
    // Copy rule: the caller owns the returned value.
    Ok(unsafe { CFRetained::from_raw(value) })
}

fn from_ax_error(attribute: &str, error: AXError) -> CaptureError {
    match error {
        AXError::APIDisabled => CaptureError::PermissionNeeded,
        AXError::NoValue => CaptureError::NothingSelected,
        AXError::CannotComplete => CaptureError::Timeout,
        other => CaptureError::Platform(format!("{attribute}: AXError {}", other.0)),
    }
}

/// Sends Cmd+C to the frontmost app and reads the copied text, then puts the
/// previous pasteboard content back (diagnostics: capture probe).
pub fn capture_via_copy() -> Result<Captured, CaptureError> {
    wait_for_modifier_release();
    let pasteboard = NSPasteboard::generalPasteboard();
    let before = pasteboard.changeCount();
    let snapshot = snapshot(&pasteboard);
    post_copy()?;
    if !wait_for_change(&pasteboard, before) {
        // Nothing was copied; the pasteboard is untouched.
        return Err(CaptureError::NothingSelected);
    }
    let text = unsafe { pasteboard.stringForType(NSPasteboardTypeString) }
        .map(|text| text.to_string())
        .unwrap_or_default();
    restore(&pasteboard, &snapshot);
    let text = selection_text([text]).ok_or(CaptureError::NothingSelected)?;
    Ok(Captured {
        text,
        method: CaptureMethod::Clipboard,
    })
}

/// Current plain-text pasteboard content, if any (diagnostics only).
pub fn clipboard_text() -> Option<String> {
    unsafe { NSPasteboard::generalPasteboard().stringForType(NSPasteboardTypeString) }
        .map(|text| text.to_string())
}

/// The hotkey fires on key down; a Cmd+C sent while Shift is still held
/// would arrive as Cmd+Shift+C in some apps.
fn wait_for_modifier_release() {
    let held = CGEventFlags::MaskCommand
        | CGEventFlags::MaskShift
        | CGEventFlags::MaskControl
        | CGEventFlags::MaskAlternate;
    let deadline = Instant::now() + MODIFIER_RELEASE_WAIT;
    while Instant::now() < deadline
        && CGEventSource::flags_state(CGEventSourceStateID::CombinedSessionState).intersects(held)
    {
        thread::sleep(POLL);
    }
}

fn post_copy() -> Result<(), CaptureError> {
    // A private source keeps physically held keys out of the event's flags.
    let source = CGEventSource::new(CGEventSourceStateID::Private);
    for key_down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.as_deref(), KEY_C, key_down)
            .ok_or_else(|| CaptureError::Platform("cannot create the Cmd+C event".into()))?;
        CGEvent::set_flags(Some(&event), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    }
    Ok(())
}

fn wait_for_change(pasteboard: &NSPasteboard, before: isize) -> bool {
    let deadline = Instant::now() + COPY_WAIT;
    loop {
        if pasteboard.changeCount() != before {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
}

/// Every item with every type it offers, as data.
type Snapshot = Vec<Vec<(Retained<NSPasteboardType>, Retained<NSData>)>>;

fn snapshot(pasteboard: &NSPasteboard) -> Snapshot {
    let Some(items) = pasteboard.pasteboardItems() else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            item.types()
                .iter()
                .filter_map(|kind| item.dataForType(&kind).map(|data| (kind, data)))
                .collect()
        })
        .collect()
}

fn restore(pasteboard: &NSPasteboard, snapshot: &Snapshot) {
    pasteboard.clearContents();
    if snapshot.is_empty() {
        return;
    }
    let transient = NSString::from_str(TRANSIENT_TYPE);
    let marker = NSData::new();
    let items: Vec<Retained<ProtocolObject<dyn objc2_app_kit::NSPasteboardWriting>>> = snapshot
        .iter()
        .map(|entries| {
            let item = NSPasteboardItem::new();
            for (kind, data) in entries {
                item.setData_forType(data, kind);
            }
            item.setData_forType(&marker, &transient);
            ProtocolObject::from_retained(item)
        })
        .collect();
    pasteboard.writeObjects(&NSArray::from_retained_slice(&items));
}

/// Receives the text of a Services menu request.
type ServiceHandler = Box<dyn Fn(String) + Send + Sync>;

static SERVICE_HANDLER: OnceLock<ServiceHandler> = OnceLock::new();

define_class!(
    // SAFETY: NSObject has no subclassing requirements; no Drop impl.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RightPeekServiceProvider"]
    struct ServiceProvider;

    impl ServiceProvider {
        /// `NSMessage` "showSelection" in Info.plist `NSServices`.
        #[unsafe(method(showSelection:userData:error:))]
        fn show_selection(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            _error: *mut *mut NSString,
        ) {
            let text = unsafe { pasteboard.stringForType(NSPasteboardTypeString) }
                .map(|text| text.to_string())
                .unwrap_or_default();
            if let Some(handler) = SERVICE_HANDLER.get() {
                // Off the main thread: the handler opens a window.
                let _ = thread::Builder::new()
                    .name("rightpeek-service".into())
                    .spawn(move || handler(text));
            }
        }
    }
);

/// Registers the provider behind the "Show in RightPeek" Services entry.
/// Must run on the main thread; `handler` gets the selected text.
pub fn register_services(handler: ServiceHandler) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("Services must be registered on the main thread")?;
    SERVICE_HANDLER
        .set(handler)
        .map_err(|_| "Services are already registered")?;
    let provider: Retained<ServiceProvider> =
        unsafe { msg_send![ServiceProvider::alloc(mtm), init] };
    unsafe { NSApplication::sharedApplication(mtm).setServicesProvider(Some(&provider)) };
    // NSApplication does not retain its services provider; keep it for the
    // life of the process.
    std::mem::forget(provider);
    NSUpdateDynamicServices();
    Ok(())
}
