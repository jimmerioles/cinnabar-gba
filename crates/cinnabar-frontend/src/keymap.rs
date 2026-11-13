//! Keyboard bindings and config-file key remapping.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use cinnabar_core::keypad::keys;
use clap::ValueEnum;
use serde::Deserialize;
use winit::keyboard::KeyCode;

/// Built-in keyboard layouts.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardLayout {
    /// Home-row split: ESDF D-pad, J/K face, A/L shoulders.
    #[default]
    Ergonomic,
    /// Emulator convention: arrow D-pad, Z/X face, A/S shoulders.
    Classic,
    /// Arrow D-pad with the numeric keypad for the buttons.
    Numpad,
}

type Binding = (&'static str, u16, &'static [KeyCode]);

const BUTTONS: [(&str, u16); 10] = [
    ("A", keys::A),
    ("B", keys::B),
    ("Start", keys::START),
    ("Select", keys::SELECT),
    ("Up", keys::UP),
    ("Down", keys::DOWN),
    ("Left", keys::LEFT),
    ("Right", keys::RIGHT),
    ("L", keys::L),
    ("R", keys::R),
];

// Keep these presets in sync with the browser maps in web/app.js.
const ERGONOMIC: &[Binding] = &[
    ("A", keys::A, &[KeyCode::KeyK]),
    ("B", keys::B, &[KeyCode::KeyJ]),
    ("Start", keys::START, &[KeyCode::Enter]),
    ("Select", keys::SELECT, &[KeyCode::Backspace]),
    ("Up", keys::UP, &[KeyCode::KeyE]),
    ("Down", keys::DOWN, &[KeyCode::KeyD]),
    ("Left", keys::LEFT, &[KeyCode::KeyS]),
    ("Right", keys::RIGHT, &[KeyCode::KeyF]),
    ("L", keys::L, &[KeyCode::KeyA]),
    ("R", keys::R, &[KeyCode::KeyL]),
];

const CLASSIC: &[Binding] = &[
    ("A", keys::A, &[KeyCode::KeyX]),
    ("B", keys::B, &[KeyCode::KeyZ]),
    ("Start", keys::START, &[KeyCode::Enter]),
    ("Select", keys::SELECT, &[KeyCode::Backspace]),
    ("Up", keys::UP, &[KeyCode::ArrowUp]),
    ("Down", keys::DOWN, &[KeyCode::ArrowDown]),
    ("Left", keys::LEFT, &[KeyCode::ArrowLeft]),
    ("Right", keys::RIGHT, &[KeyCode::ArrowRight]),
    ("L", keys::L, &[KeyCode::KeyA]),
    ("R", keys::R, &[KeyCode::KeyS]),
];

const NUMPAD: &[Binding] = &[
    ("A", keys::A, &[KeyCode::Numpad2]),
    ("B", keys::B, &[KeyCode::Numpad1]),
    ("Start", keys::START, &[KeyCode::NumpadEnter]),
    ("Select", keys::SELECT, &[KeyCode::NumpadDecimal]),
    ("Up", keys::UP, &[KeyCode::ArrowUp]),
    ("Down", keys::DOWN, &[KeyCode::ArrowDown]),
    ("Left", keys::LEFT, &[KeyCode::ArrowLeft]),
    ("Right", keys::RIGHT, &[KeyCode::ArrowRight]),
    ("L", keys::L, &[KeyCode::Numpad4]),
    ("R", keys::R, &[KeyCode::Numpad6]),
];

fn preset(layout: KeyboardLayout) -> &'static [Binding] {
    match layout {
        KeyboardLayout::Ergonomic => ERGONOMIC,
        KeyboardLayout::Classic => CLASSIC,
        KeyboardLayout::Numpad => NUMPAD,
    }
}

#[derive(Clone, Debug)]
pub struct KeyMap {
    bindings: Vec<(KeyCode, u16)>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::from_config(KeyboardLayout::default(), &BTreeMap::new())
            .expect("default key bindings are valid")
    }
}

impl KeyMap {
    /// Build bindings from a named preset, then apply per-button overrides.
    /// Each override is a comma-separated list of key names.
    pub fn from_config(
        layout: KeyboardLayout,
        overrides: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let mut bindings = Vec::new();
        for (_, bit, default_keys) in preset(layout) {
            for key in *default_keys {
                bindings.push((*key, *bit));
            }
        }

        for (name, spec) in overrides {
            let bit = button_bit(name).with_context(|| format!("unknown button \"{name}\""))?;
            bindings.retain(|(_, bound)| *bound != bit);
            for key_name in spec.split(',').map(str::trim).filter(|key| !key.is_empty()) {
                let key = parse_key(key_name)
                    .with_context(|| format!("invalid key for button \"{name}\""))?;
                bindings.push((key, bit));
            }
        }

        validate(&bindings)?;
        Ok(Self { bindings })
    }

    pub fn bit_for(&self, code: KeyCode) -> Option<u16> {
        self.bindings
            .iter()
            .find_map(|(bound, bit)| (*bound == code).then_some(*bit))
    }
}

fn button_bit(name: &str) -> Option<u16> {
    BUTTONS
        .iter()
        .find_map(|(button, bit)| button.eq_ignore_ascii_case(name).then_some(*bit))
}

fn button_name(bit: u16) -> &'static str {
    BUTTONS
        .iter()
        .find_map(|(button, candidate)| (*candidate == bit).then_some(*button))
        .unwrap_or("unknown")
}

fn validate(bindings: &[(KeyCode, u16)]) -> Result<()> {
    for (button, bit) in BUTTONS {
        if !bindings.iter().any(|(_, bound)| *bound == bit) {
            bail!("button \"{button}\" has no keys; add a key or remove the override");
        }
    }

    for (index, (code, bit)) in bindings.iter().enumerate() {
        if let Some((_, other)) = bindings[..index]
            .iter()
            .find(|(other_code, other_bit)| other_code == code && other_bit != bit)
        {
            bail!(
                "key {code:?} is bound to both {} and {}; each key can only trigger one button",
                button_name(*other),
                button_name(*bit)
            );
        }
    }

    Ok(())
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
        "numlock" => KeyCode::NumLock,
        "numpad0" | "kp0" => KeyCode::Numpad0,
        "numpad1" | "kp1" => KeyCode::Numpad1,
        "numpad2" | "kp2" => KeyCode::Numpad2,
        "numpad3" | "kp3" => KeyCode::Numpad3,
        "numpad4" | "kp4" => KeyCode::Numpad4,
        "numpad5" | "kp5" => KeyCode::Numpad5,
        "numpad6" | "kp6" => KeyCode::Numpad6,
        "numpad7" | "kp7" => KeyCode::Numpad7,
        "numpad8" | "kp8" => KeyCode::Numpad8,
        "numpad9" | "kp9" => KeyCode::Numpad9,
        "numpadenter" | "kpenter" => KeyCode::NumpadEnter,
        "numpadadd" | "numpadplus" | "kpplus" => KeyCode::NumpadAdd,
        "numpadsubtract" | "numpadminus" | "kpminus" => KeyCode::NumpadSubtract,
        "numpadmultiply" | "numpadstar" | "kpstar" => KeyCode::NumpadMultiply,
        "numpaddivide" | "numpadslash" | "kpslash" => KeyCode::NumpadDivide,
        "numpaddecimal" | "numpaddot" | "kpdot" => KeyCode::NumpadDecimal,
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

    fn map_with(layout: KeyboardLayout, overrides: &[(&str, &str)]) -> KeyMap {
        let overrides = overrides
            .iter()
            .map(|(button, keys)| ((*button).to_string(), (*keys).to_string()))
            .collect();
        KeyMap::from_config(layout, &overrides).expect("valid key map")
    }

    #[test]
    fn ergonomic_layout_matches_the_documented_controls() {
        let map = KeyMap::default();
        assert_eq!(map.bit_for(KeyCode::KeyE), Some(keys::UP));
        assert_eq!(map.bit_for(KeyCode::KeyS), Some(keys::LEFT));
        assert_eq!(map.bit_for(KeyCode::KeyD), Some(keys::DOWN));
        assert_eq!(map.bit_for(KeyCode::KeyF), Some(keys::RIGHT));
        assert_eq!(map.bit_for(KeyCode::KeyJ), Some(keys::B));
        assert_eq!(map.bit_for(KeyCode::KeyK), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyA), Some(keys::L));
        assert_eq!(map.bit_for(KeyCode::KeyL), Some(keys::R));
        assert_eq!(map.bit_for(KeyCode::Enter), Some(keys::START));
        assert_eq!(map.bit_for(KeyCode::Backspace), Some(keys::SELECT));
        assert_eq!(map.bit_for(KeyCode::ArrowUp), None);
    }

    #[test]
    fn classic_layout_matches_the_emulator_convention() {
        let map = map_with(KeyboardLayout::Classic, &[]);
        assert_eq!(map.bit_for(KeyCode::ArrowUp), Some(keys::UP));
        assert_eq!(map.bit_for(KeyCode::KeyZ), Some(keys::B));
        assert_eq!(map.bit_for(KeyCode::KeyX), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyA), Some(keys::L));
        assert_eq!(map.bit_for(KeyCode::KeyS), Some(keys::R));
        assert_eq!(map.bit_for(KeyCode::KeyE), None);
    }

    #[test]
    fn numpad_layout_uses_a_spatially_correct_cluster() {
        let map = map_with(KeyboardLayout::Numpad, &[]);
        assert_eq!(map.bit_for(KeyCode::Numpad1), Some(keys::B));
        assert_eq!(map.bit_for(KeyCode::Numpad2), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::Numpad4), Some(keys::L));
        assert_eq!(map.bit_for(KeyCode::Numpad6), Some(keys::R));
        assert_eq!(map.bit_for(KeyCode::NumpadEnter), Some(keys::START));
        assert_eq!(map.bit_for(KeyCode::NumpadDecimal), Some(keys::SELECT));
        assert_eq!(map.bit_for(KeyCode::ArrowUp), Some(keys::UP));
    }

    #[test]
    fn config_overrides_replace_defaults() {
        let map = map_with(KeyboardLayout::Ergonomic, &[("A", "space, g")]);
        assert_eq!(map.bit_for(KeyCode::Space), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyG), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::KeyK), None);
        assert_eq!(map.bit_for(KeyCode::KeyA), Some(keys::L));
    }

    #[test]
    fn wasd_overrides_replace_the_ergonomic_defaults() {
        let map = map_with(
            KeyboardLayout::Ergonomic,
            &[
                ("Up", "w"),
                ("Down", "s"),
                ("Left", "a"),
                ("Right", "d"),
                ("L", "lshift"),
                ("R", "o"),
            ],
        );
        assert_eq!(map.bit_for(KeyCode::KeyW), Some(keys::UP));
        assert_eq!(map.bit_for(KeyCode::KeyS), Some(keys::DOWN));
        assert_eq!(map.bit_for(KeyCode::KeyA), Some(keys::LEFT));
        assert_eq!(map.bit_for(KeyCode::KeyD), Some(keys::RIGHT));
        assert_eq!(map.bit_for(KeyCode::ShiftLeft), Some(keys::L));
        assert_eq!(map.bit_for(KeyCode::KeyO), Some(keys::R));
        assert_eq!(map.bit_for(KeyCode::KeyL), None);
    }

    #[test]
    fn parses_documented_key_names() {
        assert_eq!(parse_key("space").unwrap(), KeyCode::Space);
        assert_eq!(parse_key("semicolon").unwrap(), KeyCode::Semicolon);
        assert_eq!(parse_key("lshift").unwrap(), KeyCode::ShiftLeft);
    }

    #[test]
    fn parses_numpad_keys() {
        assert_eq!(parse_key("numpad0").unwrap(), KeyCode::Numpad0);
        assert_eq!(parse_key("Numpad9").unwrap(), KeyCode::Numpad9);
        assert_eq!(parse_key("kp4").unwrap(), KeyCode::Numpad4);
        assert_eq!(parse_key("numpadenter").unwrap(), KeyCode::NumpadEnter);
        assert_eq!(parse_key("numpaddecimal").unwrap(), KeyCode::NumpadDecimal);
        assert_eq!(parse_key("numlock").unwrap(), KeyCode::NumLock);
    }

    #[test]
    fn config_overrides_accept_numpad_bindings() {
        let mut overrides = BTreeMap::new();
        overrides.insert("A".to_string(), "numpad0".to_string());
        overrides.insert("Start".to_string(), "numpadenter".to_string());
        let map = KeyMap::from_config(KeyboardLayout::Ergonomic, &overrides).unwrap();
        assert_eq!(map.bit_for(KeyCode::Numpad0), Some(keys::A));
        assert_eq!(map.bit_for(KeyCode::NumpadEnter), Some(keys::START));
        assert_eq!(map.bit_for(KeyCode::KeyE), Some(keys::UP));
    }

    #[test]
    fn rejects_conflicting_keys() {
        let mut overrides = BTreeMap::new();
        overrides.insert("A".to_string(), "j".to_string());
        assert!(KeyMap::from_config(KeyboardLayout::Ergonomic, &overrides).is_err());
    }

    #[test]
    fn rejects_empty_button_bindings() {
        let mut overrides = BTreeMap::new();
        overrides.insert("A".to_string(), " , ".to_string());
        assert!(KeyMap::from_config(KeyboardLayout::Ergonomic, &overrides).is_err());
    }

    #[test]
    fn rejects_unknown_keys_and_buttons() {
        assert!(parse_key("not-a-key").is_err());
        let mut overrides = BTreeMap::new();
        overrides.insert("Turbo".to_string(), "a".to_string());
        assert!(KeyMap::from_config(KeyboardLayout::Ergonomic, &overrides).is_err());
    }
}
