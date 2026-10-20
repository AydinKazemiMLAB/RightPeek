//! A canonical hotkey (`core::settings::normalize_hotkey`) in the forms the
//! Linux backends need: an X11 keysym and modifier mask (key grab), a GNOME
//! custom-shortcut binding and an XDG GlobalShortcuts trigger. Pure data, so
//! it is tested on every OS (docs/design.md, Linux).

use super::settings::{HotkeyError, normalize_hotkey};

/// X11 modifier bits (core protocol, standard modifier mapping).
pub const SHIFT_MASK: u16 = 1;
pub const LOCK_MASK: u16 = 1 << 1;
pub const CONTROL_MASK: u16 = 1 << 2;
/// Alt.
pub const MOD1_MASK: u16 = 1 << 3;
/// Num Lock.
pub const MOD2_MASK: u16 = 1 << 4;
/// Super (Win).
pub const MOD4_MASK: u16 = 1 << 6;

/// The modifiers a hotkey can use; Caps Lock, Num Lock and the rest of a key
/// event's state are ignored when matching.
const HOTKEY_MODIFIERS: u16 = SHIFT_MASK | CONTROL_MASK | MOD1_MASK | MOD4_MASK;

/// Lock-key states a grab must also cover, or the hotkey stops working
/// while Caps Lock or Num Lock is on.
pub const LOCK_VARIANTS: [u16; 4] = [0, LOCK_MASK, MOD2_MASK, LOCK_MASK | MOD2_MASK];

/// A hotkey as X11 sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X11Hotkey {
    pub keysym: u32,
    pub modifiers: u16,
}

impl X11Hotkey {
    /// Parses any form `normalize_hotkey` accepts.
    pub fn parse(hotkey: &str) -> Result<Self, HotkeyError> {
        let canonical = normalize_hotkey(hotkey)?;
        let mut modifiers = 0;
        let mut keysym = None;
        for token in canonical.split('+') {
            match token {
                "Ctrl" => modifiers |= CONTROL_MASK,
                "Alt" => modifiers |= MOD1_MASK,
                "Shift" => modifiers |= SHIFT_MASK,
                "Super" => modifiers |= MOD4_MASK,
                key => {
                    keysym = Some(
                        x11_keysym(key).ok_or_else(|| HotkeyError::UnknownToken(key.to_owned()))?,
                    )
                }
            }
        }
        let keysym = keysym.ok_or(HotkeyError::NoKey)?;
        Ok(Self { keysym, modifiers })
    }

    /// Whether a key event's modifier state is this hotkey's.
    pub fn matches_state(&self, state: u16) -> bool {
        state & HOTKEY_MODIFIERS == self.modifiers
    }
}

/// The X11 keysym of a canonical key name (letters as their lowercase
/// keysym, which is what the keyboard map lists first).
pub fn x11_keysym(key: &str) -> Option<u32> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_uppercase() {
            return Some(u32::from(c.to_ascii_lowercase()));
        }
        if c.is_ascii_digit() {
            return Some(u32::from(c));
        }
    }
    if let Some(number) = key.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
        && (1..=24).contains(&number)
    {
        return Some(0xffbe + number - 1);
    }
    let keysym = match key {
        "Space" => 0x0020,
        "Enter" => 0xff0d,
        "Tab" => 0xff09,
        "Escape" => 0xff1b,
        "Backspace" => 0xff08,
        "Delete" => 0xffff,
        "Insert" => 0xff63,
        "Home" => 0xff50,
        "End" => 0xff57,
        "PageUp" => 0xff55,
        "PageDown" => 0xff56,
        "ArrowUp" => 0xff52,
        "ArrowDown" => 0xff54,
        "ArrowLeft" => 0xff51,
        "ArrowRight" => 0xff53,
        "Comma" => 0x002c,
        "Period" => 0x002e,
        "Slash" => 0x002f,
        "Backslash" => 0x005c,
        "Backquote" => 0x0060,
        "Minus" => 0x002d,
        "Equal" => 0x003d,
        "Semicolon" => 0x003b,
        "Quote" => 0x0027,
        "BracketLeft" => 0x005b,
        "BracketRight" => 0x005d,
        _ => return None,
    };
    Some(keysym)
}

/// Path of RightPeek's entry among GNOME's custom keyboard shortcuts.
pub const GNOME_SHORTCUT_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/rightpeek/";

/// The xkb keysym name of a canonical key name, as GNOME bindings and XDG
/// shortcut triggers spell keys.
pub fn xkb_key_name(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_alphanumeric()
    {
        return Some(c.to_ascii_lowercase().to_string());
    }
    if let Some(number) = key.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&number)
    {
        return Some(key.to_owned());
    }
    let name = match key {
        "Space" => "space",
        "Enter" => "Return",
        "Tab" => "Tab",
        "Escape" => "Escape",
        "Backspace" => "BackSpace",
        "Delete" => "Delete",
        "Insert" => "Insert",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "Page_Up",
        "PageDown" => "Page_Down",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "Comma" => "comma",
        "Period" => "period",
        "Slash" => "slash",
        "Backslash" => "backslash",
        "Backquote" => "grave",
        "Minus" => "minus",
        "Equal" => "equal",
        "Semicolon" => "semicolon",
        "Quote" => "apostrophe",
        "BracketLeft" => "bracketleft",
        "BracketRight" => "bracketright",
        _ => return None,
    };
    Some(name.to_owned())
}

/// Splits a hotkey into modifier flags (Ctrl, Alt, Shift, Super) and the
/// xkb name of its key.
fn xkb_parts(hotkey: &str) -> Result<([bool; 4], String), HotkeyError> {
    let canonical = normalize_hotkey(hotkey)?;
    let mut modifiers = [false; 4];
    let mut key = None;
    for token in canonical.split('+') {
        match token {
            "Ctrl" => modifiers[0] = true,
            "Alt" => modifiers[1] = true,
            "Shift" => modifiers[2] = true,
            "Super" => modifiers[3] = true,
            name => {
                key = Some(
                    xkb_key_name(name).ok_or_else(|| HotkeyError::UnknownToken(name.to_owned()))?,
                )
            }
        }
    }
    Ok((modifiers, key.ok_or(HotkeyError::NoKey)?))
}

/// Joins the names of the set modifiers, then the key.
fn join_parts(modifiers: [bool; 4], names: [&str; 4], key: &str, separator: &str) -> String {
    let mut parts: Vec<&str> = names
        .iter()
        .zip(modifiers)
        .filter_map(|(name, on)| on.then_some(*name))
        .collect();
    parts.push(key);
    parts.join(separator)
}

/// A GNOME keybinding such as `<Control><Shift>space`.
pub fn gnome_binding(hotkey: &str) -> Result<String, HotkeyError> {
    let (modifiers, key) = xkb_parts(hotkey)?;
    let names = ["<Control>", "<Alt>", "<Shift>", "<Super>"];
    Ok(join_parts(modifiers, names, &key, ""))
}

/// An XDG shortcut trigger such as `CTRL+SHIFT+space` (GlobalShortcuts
/// portal, `preferred_trigger`).
pub fn portal_trigger(hotkey: &str) -> Result<String, HotkeyError> {
    let (modifiers, key) = xkb_parts(hotkey)?;
    let names = ["CTRL", "ALT", "SHIFT", "LOGO"];
    Ok(join_parts(modifiers, names, &key, "+"))
}

/// Whether `XDG_CURRENT_DESKTOP` (a colon-separated list) names GNOME.
pub fn is_gnome(current_desktop: &str) -> bool {
    current_desktop
        .split(':')
        .any(|desktop| desktop.eq_ignore_ascii_case("gnome"))
}

/// A string in GVariant text form, as `gsettings set` expects it.
pub fn gvariant_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

/// `list` (a GVariant string array as `gsettings get` prints it) with `path`
/// appended; `None` if it is already there or `list` cannot be read.
pub fn with_path_added(list: &str, path: &str) -> Option<String> {
    let inner = list
        .trim()
        .trim_start_matches("@as")
        .trim()
        .strip_prefix('[')?
        .strip_suffix(']')?;
    let mut items: Vec<String> = inner
        .split(',')
        .map(|item| item.trim().trim_matches('\'').to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    if items.iter().any(|item| item == path) {
        return None;
    }
    items.push(path.to_owned());
    let quoted: Vec<String> = items.iter().map(|item| gvariant_string(item)).collect();
    Some(format!("[{}]", quoted.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_binding_uses_angle_bracket_modifiers_and_keysym_names() {
        assert_eq!(
            gnome_binding("Ctrl+Shift+Space").unwrap(),
            "<Control><Shift>space"
        );
        assert_eq!(gnome_binding("Ctrl+Alt+R").unwrap(), "<Control><Alt>r");
        assert_eq!(gnome_binding("Super+PageDown").unwrap(), "<Super>Page_Down");
        assert_eq!(gnome_binding("Alt+Shift+F5").unwrap(), "<Alt><Shift>F5");
    }

    #[test]
    fn portal_trigger_uses_xdg_modifier_names() {
        assert_eq!(
            portal_trigger("Ctrl+Shift+Space").unwrap(),
            "CTRL+SHIFT+space"
        );
        assert_eq!(portal_trigger("alt+win+1").unwrap(), "ALT+LOGO+1");
        assert_eq!(portal_trigger("Ctrl+Quote").unwrap(), "CTRL+apostrophe");
    }

    #[test]
    fn every_named_key_has_an_xkb_name() {
        for key in [
            "Space",
            "Enter",
            "Tab",
            "Escape",
            "Backspace",
            "Delete",
            "Insert",
            "Home",
            "End",
            "PageUp",
            "PageDown",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Comma",
            "Period",
            "Slash",
            "Backslash",
            "Backquote",
            "Minus",
            "Equal",
            "Semicolon",
            "Quote",
            "BracketLeft",
            "BracketRight",
        ] {
            assert!(gnome_binding(&format!("Ctrl+{key}")).is_ok(), "{key}");
        }
    }

    #[test]
    fn xkb_bindings_reject_invalid_hotkeys() {
        assert_eq!(gnome_binding("Space"), Err(HotkeyError::NoModifier));
        assert_eq!(portal_trigger("Ctrl"), Err(HotkeyError::NoKey));
    }

    #[test]
    fn gnome_is_found_in_the_desktop_list() {
        assert!(is_gnome("ubuntu:GNOME"));
        assert!(is_gnome("GNOME"));
        assert!(!is_gnome("KDE"));
        assert!(!is_gnome("X-Cinnamon"));
        assert!(!is_gnome(""));
    }

    #[test]
    fn gvariant_strings_are_quoted_and_escaped() {
        assert_eq!(gvariant_string("RightPeek"), "'RightPeek'");
        assert_eq!(
            gvariant_string(r#""/opt/it's" --show"#),
            r#"'"/opt/it\'s" --show'"#
        );
        assert_eq!(gvariant_string(r"a\b"), r"'a\\b'");
    }

    #[test]
    fn our_path_is_appended_to_the_custom_shortcut_list() {
        let path = GNOME_SHORTCUT_PATH;
        assert_eq!(with_path_added("@as []", path), Some(format!("['{path}']")));
        assert_eq!(
            with_path_added("['/a/', '/b/']", path),
            Some(format!("['/a/', '/b/', '{path}']"))
        );
        assert_eq!(with_path_added(&format!("['/a/', '{path}']"), path), None);
        assert_eq!(with_path_added("garbage", path), None);
    }

    #[test]
    fn default_hotkey_is_ctrl_shift_space() {
        assert_eq!(
            X11Hotkey::parse("Ctrl+Shift+Space").unwrap(),
            X11Hotkey {
                keysym: 0x20,
                modifiers: CONTROL_MASK | SHIFT_MASK
            }
        );
    }

    #[test]
    fn alt_and_super_map_to_mod1_and_mod4() {
        let hotkey = X11Hotkey::parse("alt+win+q").unwrap();
        assert_eq!(hotkey.modifiers, MOD1_MASK | MOD4_MASK);
        assert_eq!(hotkey.keysym, u32::from(b'q'));
    }

    #[test]
    fn letters_digits_and_function_keys_have_keysyms() {
        assert_eq!(x11_keysym("A"), Some(0x61));
        assert_eq!(x11_keysym("Z"), Some(0x7a));
        assert_eq!(x11_keysym("0"), Some(0x30));
        assert_eq!(x11_keysym("F1"), Some(0xffbe));
        assert_eq!(x11_keysym("F12"), Some(0xffc9));
        assert_eq!(x11_keysym("F24"), Some(0xffd5));
        assert_eq!(x11_keysym("F25"), None);
    }

    #[test]
    fn every_named_key_the_settings_accept_has_a_keysym() {
        for key in [
            "Space",
            "Enter",
            "Tab",
            "Escape",
            "Backspace",
            "Delete",
            "Insert",
            "Home",
            "End",
            "PageUp",
            "PageDown",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Comma",
            "Period",
            "Slash",
            "Backslash",
            "Backquote",
            "Minus",
            "Equal",
            "Semicolon",
            "Quote",
            "BracketLeft",
            "BracketRight",
        ] {
            let hotkey = format!("Ctrl+{key}");
            assert!(X11Hotkey::parse(&hotkey).is_ok(), "{hotkey}");
        }
    }

    #[test]
    fn invalid_hotkeys_are_rejected_like_in_settings() {
        assert_eq!(X11Hotkey::parse("Space"), Err(HotkeyError::NoModifier));
        assert_eq!(X11Hotkey::parse("Ctrl+Alt"), Err(HotkeyError::NoKey));
    }

    #[test]
    fn lock_keys_do_not_change_the_match() {
        let hotkey = X11Hotkey::parse("Ctrl+Shift+Space").unwrap();
        let pressed = CONTROL_MASK | SHIFT_MASK;
        for lock in LOCK_VARIANTS {
            assert!(hotkey.matches_state(pressed | lock));
        }
    }

    #[test]
    fn extra_or_missing_modifiers_do_not_match() {
        let hotkey = X11Hotkey::parse("Ctrl+Shift+Space").unwrap();
        assert!(!hotkey.matches_state(CONTROL_MASK));
        assert!(!hotkey.matches_state(CONTROL_MASK | SHIFT_MASK | MOD1_MASK));
    }

    #[test]
    fn lock_variants_cover_caps_and_num_lock() {
        assert!(LOCK_VARIANTS.contains(&(LOCK_MASK | MOD2_MASK)));
        assert_eq!(LOCK_VARIANTS.len(), 4);
    }
}
