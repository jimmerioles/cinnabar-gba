//! Gamepad support via gilrs.

use anyhow::Result;
use cinnabar_core::keypad::keys;
use gilrs::{Axis, Button, EventType, Gilrs};

const DEADZONE: f32 = 0.5;

pub struct Gamepad {
    gilrs: Gilrs,
    pressed: u16,
}

impl Gamepad {
    pub fn new() -> Result<Self> {
        let gilrs = Gilrs::new()
            .map_err(|error| anyhow::anyhow!("no gamepad backend available: {error}"))?;
        Ok(Self { gilrs, pressed: 0 })
    }

    /// Pump gamepad events and return the current GBA key bits.
    pub fn poll(&mut self) -> u16 {
        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                EventType::ButtonPressed(button, _) => {
                    if let Some(bit) = button_bit(button) {
                        self.pressed |= bit;
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(bit) = button_bit(button) {
                        self.pressed &= !bit;
                    }
                }
                EventType::AxisChanged(axis, value, _) => match axis {
                    Axis::LeftStickX | Axis::DPadX => {
                        set_axis(&mut self.pressed, keys::LEFT, keys::RIGHT, value);
                    }
                    Axis::LeftStickY | Axis::DPadY => {
                        set_axis(&mut self.pressed, keys::DOWN, keys::UP, value);
                    }
                    _ => {}
                },
                EventType::Disconnected => self.pressed = 0,
                _ => {}
            }
        }
        self.pressed
    }
}

fn set_axis(pressed: &mut u16, negative: u16, positive: u16, value: f32) {
    *pressed &= !(negative | positive);
    if value <= -DEADZONE {
        *pressed |= negative;
    } else if value >= DEADZONE {
        *pressed |= positive;
    }
}

pub fn button_bit(button: Button) -> Option<u16> {
    Some(match button {
        Button::South => keys::A,
        Button::East => keys::B,
        Button::Start => keys::START,
        Button::Select | Button::Mode => keys::SELECT,
        Button::DPadUp => keys::UP,
        Button::DPadDown => keys::DOWN,
        Button::DPadLeft => keys::LEFT,
        Button::DPadRight => keys::RIGHT,
        Button::LeftTrigger | Button::LeftTrigger2 => keys::L,
        Button::RightTrigger | Button::RightTrigger2 => keys::R,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_common_gamepad_layout() {
        assert_eq!(button_bit(Button::South), Some(keys::A));
        assert_eq!(button_bit(Button::East), Some(keys::B));
        assert_eq!(button_bit(Button::Start), Some(keys::START));
        assert_eq!(button_bit(Button::DPadLeft), Some(keys::LEFT));
        assert_eq!(button_bit(Button::LeftTrigger), Some(keys::L));
        assert_eq!(button_bit(Button::RightTrigger2), Some(keys::R));
        assert_eq!(button_bit(Button::North), None);
    }

    #[test]
    fn analog_sticks_use_a_deadzone() {
        let mut pressed = 0;
        set_axis(&mut pressed, keys::LEFT, keys::RIGHT, -0.2);
        assert_eq!(pressed, 0);
        set_axis(&mut pressed, keys::LEFT, keys::RIGHT, -0.8);
        assert_eq!(pressed, keys::LEFT);
        set_axis(&mut pressed, keys::LEFT, keys::RIGHT, 0.9);
        assert_eq!(pressed, keys::RIGHT);
    }
}
