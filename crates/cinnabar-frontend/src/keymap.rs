//! Keyboard bindings and config-file key remapping.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use cinnabar_core::keypad::keys;
use winit::keyboard::KeyCode;

const DEFAULTS: [(&str, u16, &[KeyCode]); 10] = [
    ("A", keys::A, &[KeyCode::KeyA, KeyCode::KeyZ]),
    ("B", keys::B, &[KeyCode::KeyS, KeyCode::KeyX]),
    ("Start", keys::START, &[KeyCode::Enter]),
    ("Select", keys::SELECT, &[KeyCode::Backspace]),
    ("Up", keys::UP, &[KeyCode::ArrowUp]),
    ("Down", keys::DOWN, &[KeyCode::ArrowDown]),
    ("Left", keys::LEFT, &[KeyCode::ArrowLeft]),
    ("Right", keys::RIGHT, &[KeyCode::ArrowRight]),
    ("L", keys::L, &[KeyCode::KeyQ]),
    ("R", keys::R, &[KeyCode::KeyE]),
];

#[derive(Clone, Debug)]
pub struct KeyMap {
    bindings: Vec<(KeyCode, u16)>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::from_config(&BTreeMap::new()).expect("default key bindings are valid")
    }
}

impl KeyMap {
    /// Build bindings from defaults, replacing any buttons named in `overrides`.
    /// Each override is a comma-separated list of key names.
    pub fn from_config(overrides: &BTreeMap<String, String>) -> Result<Self> {
        let mut bindings = Vec::new();
        for (_, bit, default_keys) in DEFAULTS {
            for key in default_keys {
                bindings.push((*key, bit));
            }
        }

        for (name, spec) in overrides {
            let bit = button_bit(name).with_context(|| format!("unknown button \"{name}\""))?;
            bindings.retain(|(_, bound)| *bound != bit);
            for key_name in spec.split(',').map(str::trim).filter(|key| !key.is_empty()) {
                bindings.push((parse_key(key_name)?, bit));
            }
        }

        Ok(Self { bindings })
    }

    pub fn bit_for(&self, code: KeyCode) -> Option<u16> {
        self.bindings
            .iter()
            .find_map(|(bound, bit)| (*bound == code).then_some(*bit))
    }
}

fn button_bit(name: &str) -> Option<u16> {
    DEFAULTS
        .iter()
        .find_map(|(default_name, bit, _)| default_name.eq_ignore_ascii_case(name).then_some(*bit))
}

pub fn parse_key(name: &str) -> Result<KeyCode> {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized.len() == 1 {
        let character = normalized.chars().next().unwrap_or('a');
        if character.is_ascii_alphabetic() {
            return Ok(letter_key(character));
        }
        if character.is_ascii_digit() {
            return Ok(digit_key(character));
        }
    }

    Ok(match normalized.as_str() {
        "space" => KeyCode::Space,
        "enter" | "return" => KeyCode::Enter,
        "backspace" => KeyCode::Backspace,
        "tab" => KeyCode::Tab,
        "escape" | "esc" => KeyCode::Escape,
        "up" => KeyCode::ArrowUp,
        "down" => KeyCode::ArrowDown,
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "shift" | "lshift" => KeyCode::ShiftLeft,
        "rshift" => KeyCode::ShiftRight,
        "ctrl" | "control" => KeyCode::ControlLeft,
        "alt" => KeyCode::AltLeft,
        "comma" => KeyCode::Comma,
        "period" => KeyCode::Period,
        "slash" => KeyCode::Slash,
        "semicolon" => KeyCode::Semicolon,
        "quote" => KeyCode::Quote,
        "bracketleft" => KeyCode::BracketLeft,
        "bracketright" => KeyCode::BracketRight,
        "backslash" => KeyCode::Backslash,
        "minus" => KeyCode::Minus,
        "equal" => KeyCode::Equal,
        "backquote" => KeyCode::Backquote,
        "f1" => KeyCode::F1,
        "f2" => KeyCode::F2,
        "f3" => KeyCode::F3,
        "f4" => KeyCode::F4,
        "f5" => KeyCode::F5,
        "f6" => KeyCode::F6,
        "f7" => KeyCode::F7,
        "f8" => KeyCode::F8,
        "f9" => KeyCode::F9,
        "f10" => KeyCode::F10,
        "f11" => KeyCode::F11,
        "f12" => KeyCode::F12,
        _ => bail!("unknown key \"{name}\""),
    })
}

fn letter_key(character: char) -> KeyCode {
    match character.to_ascii_uppercase() {
        'A' => KeyCode::KeyA,
        'B' => KeyCode::KeyB,
        'C' => KeyCode::KeyC,
        'D' => KeyCode::KeyD,
        'E' => KeyCode::KeyE,
        'F' => KeyCode::KeyF,
        'G' => KeyCode::KeyG,
        'H' => KeyCode::KeyH,
        'I' => KeyCode::KeyI,
        'J' => KeyCode::KeyJ,
        'K' => KeyCode::KeyK,
        'L' => KeyCode::KeyL,
        'M' => KeyCode::KeyM,
        'N' => KeyCode::KeyN,
        'O' => KeyCode::KeyO,
        'P' => KeyCode::KeyP,
        'Q' => KeyCode::KeyQ,
        'R' => KeyCode::KeyR,
        'S' => KeyCode::KeyS,
        'T' => KeyCode::KeyT,
        'U' => KeyCode::KeyU,
        'V' => KeyCode::KeyV,
        'W' => KeyCode::KeyW,
        'X' => KeyCode::KeyX,
        'Y' => KeyCode::KeyY,
        _ => KeyCode::KeyZ,
    }
}

fn digit_key(character: char) -> KeyCode {
    match character {
        '0' => KeyCode::Digit0,
        '1' => KeyCode::Digit1,
        '2' => KeyCode::Digit2,
        '3' => KeyCode::Digit3,
        '4' => KeyCode::Digit4,
        '5' => KeyCode::Digit5,
        '6' => KeyCode::Digit6,
        '7' => KeyCode::Digit7,
        '8' => KeyCode::Digit8,
        _ => KeyCode::Digit9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_controls() {
        let map = KeyMap::default();
        assert_eq!(map.bit_for(KeyCode::KeyA), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyZ), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyS), Some(keys::B));
        assert_eq!(map.bit_for(KeyCode::KeyX), Some(keys::B));
        assert_eq!(map.bit_for(KeyCode::KeyQ), Some(keys::L));
        assert_eq!(map.bit_for(KeyCode::KeyE), Some(keys::R));
        assert_eq!(map.bit_for(KeyCode::Escape), None);
    }

    #[test]
    fn config_overrides_replace_defaults() {
        let mut overrides = BTreeMap::new();
        overrides.insert("A".to_string(), "space, f".to_string());
        let map = KeyMap::from_config(&overrides).unwrap();
        assert_eq!(map.bit_for(KeyCode::Space), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyF), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyA), None);
        assert_eq!(map.bit_for(KeyCode::KeyZ), None);
    }

    #[test]
    fn rejects_unknown_keys_and_buttons() {
        assert!(parse_key("not-a-key").is_err());
        let mut overrides = BTreeMap::new();
        overrides.insert("Turbo".to_string(), "a".to_string());
        assert!(KeyMap::from_config(&overrides).is_err());
    }
}
