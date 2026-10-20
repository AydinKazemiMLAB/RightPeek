//! What the Linux X11 hotkey listener grabs: a canonical hotkey
//! (`core::settings::normalize_hotkey`) as an X11 keysym and modifier mask.
//! Pure data, so it is tested on every OS (docs/design.md, Hotkey).

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

#[cfg(test)]
mod tests {
    use super::*;

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
