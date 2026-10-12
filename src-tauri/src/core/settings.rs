//! User settings model and validation. Persistence lives in the app layer.

use serde::{Deserialize, Serialize};

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
pub const DEFAULT_FONT_SIZE: u8 = 18;
pub const MIN_FONT_SIZE: u8 = 12;
pub const MAX_FONT_SIZE: u8 = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub autostart: bool,
    pub font_size: u8,
    /// Keep a hidden viewer alive for a faster first open (costs RAM).
    pub prewarm: bool,
    /// Write a rotating log file (diagnostics; never the captured text).
    pub logging: bool,
    /// Justify viewer paragraphs (both edges aligned) instead of ragged.
    pub justify: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_owned(),
            autostart: false,
            font_size: DEFAULT_FONT_SIZE,
            prewarm: false,
            logging: cfg!(debug_assertions),
            justify: false,
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

/// Keeps a font size inside the supported range.
pub fn clamp_font_size(size: u8) -> u8 {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
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
        assert_eq!(settings.hotkey, "Ctrl+Shift+Space");
        assert!(!settings.autostart);
        assert_eq!(settings.font_size, DEFAULT_FONT_SIZE);
        assert!(!settings.prewarm, "pre-warm costs RAM, so it is opt-in");
        assert!(!settings.justify, "ragged edges are the default");
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
            prewarm: true,
            logging: true,
            justify: true,
        };
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(json["fontSize"], 22);
        assert_eq!(json["justify"], true);
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
