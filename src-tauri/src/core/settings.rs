//! User settings model and validation. Persistence lives in the app layer.

use serde::{Deserialize, Serialize};

/// Cmd+Shift+Space on macOS (canonical form, see [`normalize_hotkey`]).
#[cfg(target_os = "macos")]
pub const DEFAULT_HOTKEY: &str = "Shift+Super+Space";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
pub const DEFAULT_FONT_SIZE: u8 = 18;
pub const MIN_FONT_SIZE: u8 = 12;
pub const MAX_FONT_SIZE: u8 = 40;
/// Passed by the "Start with Windows" entry so sign-in launches stay quiet.
pub const AUTOSTART_ARG: &str = "--autostart";
/// Captures the selection and opens the viewer, like the hotkey. For desktops
/// without global hotkeys (Wayland): bind a desktop shortcut to `rightpeek --show`.
pub const SHOW_ARG: &str = "--show";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub autostart: bool,
    pub font_size: u8,
    /// Write a rotating log file (diagnostics; never the captured text).
    pub logging: bool,
    /// Justify viewer paragraphs (both edges aligned) instead of ragged.
    pub justify: bool,
    /// Open Settings when launched by hand, so a first run is noticed.
    pub show_settings_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_owned(),
            autostart: false,
            font_size: DEFAULT_FONT_SIZE,
            logging: cfg!(debug_assertions),
            justify: false,
            show_settings_on_start: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyError {
    Empty,
    NoModifier,
    NoKey,
    MultipleKeys,
    UnknownToken(String),
}

impl std::fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("The shortcut is empty."),
            Self::NoModifier => {
                f.write_str("The shortcut needs at least one of Ctrl, Alt, Shift or Win.")
            }
            Self::NoKey => f.write_str("The shortcut needs a key besides the modifiers."),
            Self::MultipleKeys => {
                f.write_str("The shortcut can have only one key besides the modifiers.")
            }
            Self::UnknownToken(token) => write!(f, "Unknown key: {token}"),
        }
    }
}

impl std::error::Error for HotkeyError {}

/// Modifiers in canonical order.
const MODIFIERS: [(&str, &[&str]); 4] = [
    ("Ctrl", &["ctrl", "control"]),
    ("Alt", &["alt", "option"]),
    ("Shift", &["shift"]),
    (
        "Super",
        &["super", "win", "windows", "meta", "cmd", "command"],
    ),
];

/// Named keys the global-shortcut plugin understands, with input aliases.
const NAMED_KEYS: [(&str, &[&str]); 26] = [
    ("Space", &["space"]),
    ("Enter", &["enter", "return"]),
    ("Tab", &["tab"]),
    ("Escape", &["escape", "esc"]),
    ("Backspace", &["backspace"]),
    ("Delete", &["delete", "del"]),
    ("Insert", &["insert", "ins"]),
    ("Home", &["home"]),
    ("End", &["end"]),
    ("PageUp", &["pageup", "pgup"]),
    ("PageDown", &["pagedown", "pgdn"]),
    ("ArrowUp", &["arrowup", "up"]),
    ("ArrowDown", &["arrowdown", "down"]),
    ("ArrowLeft", &["arrowleft", "left"]),
    ("ArrowRight", &["arrowright", "right"]),
    ("Comma", &["comma"]),
    ("Period", &["period"]),
    ("Slash", &["slash"]),
    ("Backslash", &["backslash"]),
    ("Backquote", &["backquote"]),
    ("Minus", &["minus"]),
    ("Equal", &["equal"]),
    ("Semicolon", &["semicolon"]),
    ("Quote", &["quote"]),
    ("BracketLeft", &["bracketleft"]),
    ("BracketRight", &["bracketright"]),
];

/// Maps a key token (`a`, `KeyA`, `1`, `Digit1`, `f12`, `space`, ...) to its
/// canonical name.
fn canonical_key(token: &str) -> Option<String> {
    let lower = token.to_ascii_lowercase();
    let bare = lower
        .strip_prefix("key")
        .or_else(|| lower.strip_prefix("digit"))
        .filter(|rest| rest.len() == 1)
        .unwrap_or(&lower);
    let mut chars = bare.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_alphanumeric()
    {
        return Some(c.to_ascii_uppercase().to_string());
    }
    if let Some(number) = bare.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&number)
    {
        return Some(format!("F{number}"));
    }
    NAMED_KEYS
        .iter()
        .find(|(_, aliases)| aliases.contains(&bare))
        .map(|(name, _)| (*name).to_owned())
}

/// Validates a hotkey like `ctrl + alt + space` and returns its canonical
/// form (`Ctrl+Alt+Space`): modifiers in Ctrl, Alt, Shift, Super order,
/// then exactly one key.
pub fn normalize_hotkey(input: &str) -> Result<String, HotkeyError> {
    if input.trim().is_empty() {
        return Err(HotkeyError::Empty);
    }
    let mut modifiers = [false; MODIFIERS.len()];
    let mut key = None;
    for token in input.split('+').map(str::trim) {
        let lower = token.to_ascii_lowercase();
        if let Some(index) = MODIFIERS
            .iter()
            .position(|(_, aliases)| aliases.contains(&lower.as_str()))
        {
            modifiers[index] = true;
            continue;
        }
        let name =
            canonical_key(token).ok_or_else(|| HotkeyError::UnknownToken(token.to_owned()))?;
        if key.replace(name).is_some() {
            return Err(HotkeyError::MultipleKeys);
        }
    }
    let key = key.ok_or(HotkeyError::NoKey)?;
    if !modifiers.contains(&true) {
        return Err(HotkeyError::NoModifier);
    }
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .zip(modifiers)
        .filter_map(|((name, _), on)| on.then_some(*name))
        .collect();
    parts.push(&key);
    Ok(parts.join("+"))
}

/// How a canonical hotkey is shown to the user. On macOS the modifiers get
/// their Mac names (`Super` is Cmd, `Alt` is Option); the label still parses
/// back through [`normalize_hotkey`].
pub fn hotkey_label(hotkey: &str, macos: bool) -> String {
    if !macos {
        return hotkey.to_owned();
    }
    hotkey
        .split('+')
        .map(|part| match part {
            "Super" => "Cmd",
            "Alt" => "Option",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Keeps a font size inside the supported range.
pub fn clamp_font_size(size: u8) -> u8 {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

/// Whether Settings opens at start-up. Never for a `--show` launch (it opens
/// the viewer instead). Otherwise always when the hotkey failed (the app is
/// unusable without it), or when the option is on and the launch was not the
/// sign-in autostart.
pub fn open_settings_on_start<I>(settings: &Settings, hotkey_failed: bool, args: I) -> bool
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    if args.iter().any(|arg| arg == SHOW_ARG) {
        return false;
    }
    hotkey_failed
        || (settings.show_settings_on_start && !args.iter().any(|arg| arg == AUTOSTART_ARG))
}

/// Whether a launch (first or forwarded second one) asks for a capture.
pub fn wants_capture<I>(args: I) -> bool
where
    I: IntoIterator<Item = String>,
{
    args.into_iter().any(|arg| arg == SHOW_ARG)
}

/// What the running instance does when a second launch is forwarded to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondLaunch {
    /// `--show`: capture the selection and open the viewer.
    Capture,
    /// A manual launch: bring Settings up to show the app is running.
    OpenSettings,
    /// A sign-in launch (`--autostart`): stay quiet.
    Nothing,
}

pub fn second_launch<I>(args: I) -> SecondLaunch
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    if args.iter().any(|arg| arg == SHOW_ARG) {
        SecondLaunch::Capture
    } else if args.iter().any(|arg| arg == AUTOSTART_ARG) {
        SecondLaunch::Nothing
    } else {
        SecondLaunch::OpenSettings
    }
}

/// Drops a leading UTF-8 byte order mark. Some Windows editors add one when
/// saving, and JSON parsers reject it, which would silently reset settings.
pub fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_plan() {
        let settings = Settings::default();
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert!(!settings.autostart);
        assert_eq!(settings.font_size, DEFAULT_FONT_SIZE);
        assert!(!settings.justify, "ragged edges are the default");
        assert!(
            settings.show_settings_on_start,
            "a first launch must show that the app is running"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn default_hotkey_is_cmd_shift_space_on_macos() {
        assert_eq!(DEFAULT_HOTKEY, "Shift+Super+Space");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn default_hotkey_is_ctrl_shift_space_elsewhere() {
        assert_eq!(DEFAULT_HOTKEY, "Ctrl+Shift+Space");
    }

    #[test]
    fn default_hotkey_is_canonical() {
        assert_eq!(normalize_hotkey(DEFAULT_HOTKEY).unwrap(), DEFAULT_HOTKEY);
    }

    #[test]
    fn mac_label_uses_cmd_and_option() {
        assert_eq!(hotkey_label("Shift+Super+Space", true), "Shift+Cmd+Space");
        assert_eq!(hotkey_label("Ctrl+Alt+Super+Q", true), "Ctrl+Option+Cmd+Q");
    }

    #[test]
    fn label_elsewhere_is_the_canonical_form() {
        assert_eq!(hotkey_label("Ctrl+Alt+Super+Q", false), "Ctrl+Alt+Super+Q");
    }

    #[test]
    fn mac_label_normalizes_back_to_the_canonical_form() {
        let label = hotkey_label("Ctrl+Alt+Shift+Super+F5", true);
        assert_eq!(normalize_hotkey(&label).unwrap(), "Ctrl+Alt+Shift+Super+F5");
    }

    #[test]
    fn logging_is_on_in_debug_builds_only() {
        assert_eq!(Settings::default().logging, cfg!(debug_assertions));
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"autostart":true}"#).unwrap();
        assert!(settings.autostart);
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert_eq!(settings.font_size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn settings_round_trip_through_json() {
        let settings = Settings {
            hotkey: "Ctrl+Shift+R".into(),
            autostart: true,
            font_size: 22,
            logging: true,
            justify: true,
            show_settings_on_start: false,
        };
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(json["fontSize"], 22);
        assert_eq!(json["justify"], true);
        assert_eq!(json["showSettingsOnStart"], false);
        assert_eq!(serde_json::from_value::<Settings>(json).unwrap(), settings);
    }

    #[test]
    fn hotkey_is_canonicalized() {
        assert_eq!(
            normalize_hotkey(" alt + CTRL+space ").unwrap(),
            "Ctrl+Alt+Space"
        );
    }

    #[test]
    fn hotkey_accepts_letters_digits_and_function_keys() {
        assert_eq!(normalize_hotkey("ctrl+shift+r").unwrap(), "Ctrl+Shift+R");
        assert_eq!(normalize_hotkey("alt+1").unwrap(), "Alt+1");
        assert_eq!(normalize_hotkey("ctrl+f12").unwrap(), "Ctrl+F12");
    }

    #[test]
    fn hotkey_accepts_modifier_aliases() {
        assert_eq!(
            normalize_hotkey("control+option+win+q").unwrap(),
            "Ctrl+Alt+Super+Q"
        );
    }

    #[test]
    fn hotkey_requires_a_modifier() {
        assert_eq!(normalize_hotkey("Space"), Err(HotkeyError::NoModifier));
    }

    #[test]
    fn hotkey_requires_a_key() {
        assert_eq!(normalize_hotkey("Ctrl+Alt"), Err(HotkeyError::NoKey));
    }

    #[test]
    fn hotkey_rejects_two_keys() {
        assert_eq!(normalize_hotkey("Ctrl+A+B"), Err(HotkeyError::MultipleKeys));
    }

    #[test]
    fn hotkey_rejects_empty_input() {
        assert_eq!(normalize_hotkey("  "), Err(HotkeyError::Empty));
    }

    #[test]
    fn hotkey_rejects_unknown_tokens() {
        assert_eq!(
            normalize_hotkey("Ctrl+Banana"),
            Err(HotkeyError::UnknownToken("Banana".into()))
        );
    }

    #[test]
    fn hotkey_error_messages_are_readable() {
        assert_eq!(
            HotkeyError::NoModifier.to_string(),
            "The shortcut needs at least one of Ctrl, Alt, Shift or Win."
        );
        assert_eq!(
            HotkeyError::UnknownToken("Banana".into()).to_string(),
            "Unknown key: Banana"
        );
    }

    #[test]
    fn font_size_is_clamped() {
        assert_eq!(clamp_font_size(4), MIN_FONT_SIZE);
        assert_eq!(clamp_font_size(200), MAX_FONT_SIZE);
        assert_eq!(clamp_font_size(20), 20);
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn manual_launch_opens_settings_by_default() {
        let settings = Settings::default();
        assert!(open_settings_on_start(
            &settings,
            false,
            args(&["rightpeek.exe"])
        ));
    }

    #[test]
    fn launch_at_sign_in_stays_in_the_tray() {
        let settings = Settings::default();
        let launch = args(&["rightpeek.exe", AUTOSTART_ARG]);
        assert!(!open_settings_on_start(&settings, false, launch));
    }

    #[test]
    fn turning_the_option_off_starts_quietly() {
        let settings = Settings {
            show_settings_on_start: false,
            ..Settings::default()
        };
        assert!(!open_settings_on_start(
            &settings,
            false,
            args(&["rightpeek.exe"])
        ));
    }

    #[test]
    fn a_failed_hotkey_always_opens_settings() {
        let settings = Settings {
            show_settings_on_start: false,
            ..Settings::default()
        };
        let launch = args(&["rightpeek.exe", AUTOSTART_ARG]);
        assert!(open_settings_on_start(&settings, true, launch));
    }

    #[test]
    fn show_launch_opens_the_viewer_not_settings() {
        // A desktop shortcut runs `rightpeek --show` (Wayland has no global
        // hotkey), so a hotkey failure must not pop Settings every time.
        let launch = args(&["rightpeek", SHOW_ARG]);
        assert!(!open_settings_on_start(&Settings::default(), true, launch));
    }

    #[test]
    fn show_argument_requests_a_capture() {
        assert!(wants_capture(args(&["rightpeek", SHOW_ARG])));
    }

    #[test]
    fn plain_and_autostart_launches_do_not_capture() {
        assert!(!wants_capture(args(&["rightpeek"])));
        assert!(!wants_capture(args(&["rightpeek", AUTOSTART_ARG])));
    }

    #[test]
    fn second_show_launch_captures() {
        let launch = args(&["rightpeek", SHOW_ARG]);
        assert_eq!(second_launch(launch), SecondLaunch::Capture);
    }

    #[test]
    fn second_plain_launch_opens_settings() {
        // Starting the app again from the Start menu shows that it runs.
        let launch = args(&["rightpeek.exe"]);
        assert_eq!(second_launch(launch), SecondLaunch::OpenSettings);
    }

    #[test]
    fn second_sign_in_launch_stays_quiet() {
        // A duplicate startup entry must not pop Settings at sign-in.
        let launch = args(&["rightpeek.exe", AUTOSTART_ARG]);
        assert_eq!(second_launch(launch), SecondLaunch::Nothing);
    }

    #[test]
    fn utf8_bom_is_stripped() {
        assert_eq!(strip_utf8_bom(b"\xEF\xBB\xBF{}"), b"{}");
    }

    #[test]
    fn text_without_bom_is_unchanged() {
        assert_eq!(strip_utf8_bom(b"{}"), b"{}");
        assert_eq!(strip_utf8_bom(b""), b"");
    }

    #[test]
    fn settings_file_with_bom_parses() {
        let bytes = b"\xEF\xBB\xBF{\"settings\":{\"hotkey\":\"Alt+X\"}}";
        let value: serde_json::Value = serde_json::from_slice(strip_utf8_bom(bytes)).unwrap();
        assert_eq!(value["settings"]["hotkey"], "Alt+X");
    }
}
