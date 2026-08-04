//! Logical gamepad mapping over a raw joystick (ported from `gamepad.cpp`).

use std::cell::RefCell;
use std::rc::Rc;

use super::ids::{GamepadAxis, GamepadButton};
use super::joystick::LinuxJoystick;
use super::mapping::{GamepadMapping, MapFrom, MapTo};

/// A logical gamepad bound to a raw joystick and a controller mapping.
pub struct Gamepad {
    index: i32,
    joystick: Rc<RefCell<LinuxJoystick>>,
    mapping: Rc<RefCell<GamepadMapping>>,
}

impl Gamepad {
    pub fn new(
        index: i32,
        joystick: Rc<RefCell<LinuxJoystick>>,
        mapping: Rc<RefCell<GamepadMapping>>,
    ) -> Gamepad {
        Gamepad { index, joystick, mapping }
    }

    pub fn get_index(&self) -> i32 {
        self.index
    }

    pub fn get_joystick(&self) -> &Rc<RefCell<LinuxJoystick>> {
        &self.joystick
    }

    pub fn get_mapping(&self) -> &Rc<RefCell<GamepadMapping>> {
        &self.mapping
    }

    /// Replace the mapping (used when a dummy mapping is generated at runtime).
    pub fn set_mapping(&mut self, mapping: Rc<RefCell<GamepadMapping>>) {
        self.mapping = mapping;
    }

    pub fn get_button(&self, index: GamepadButton) -> bool {
        let maps: Vec<_> = self.mapping.borrow().mappings.clone();
        for m in &maps {
            let matches = match m.to {
                MapTo::Button { id } => id == index,
                _ => false,
            };
            if !matches {
                continue;
            }
            match &m.from {
                MapFrom::Button { id } => {
                    if self.joystick.borrow().get_button(*id) {
                        return true;
                    }
                }
                MapFrom::Axis { id, .. } => {
                    let v = self.joystick.borrow().get_axis(*id);
                    if GamepadMapping::is_axis_active(&m.from, v) {
                        return true;
                    }
                }
                MapFrom::Hat { id, mask } => {
                    let v = self.joystick.borrow().get_hat(*id);
                    if v & mask != 0 {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn get_axis(&self, index: GamepadAxis) -> f32 {
        let maps: Vec<_> = self.mapping.borrow().mappings.clone();
        for m in &maps {
            let matches = match m.to {
                MapTo::Axis { id, .. } => id == index,
                _ => false,
            };
            if !matches {
                continue;
            }
            match &m.from {
                MapFrom::Button { id } => {
                    if self.joystick.borrow().get_button(*id) {
                        if let MapTo::Axis { max, .. } = m.to {
                            return max;
                        }
                    }
                }
                MapFrom::Axis { id, .. } => {
                    let v = self.joystick.borrow().get_axis(*id);
                    let v = GamepadMapping::get_axis_transformed_value(m, v);
                    if !v.is_nan() {
                        return v;
                    }
                }
                MapFrom::Hat { id, mask } => {
                    let v = self.joystick.borrow().get_hat(*id);
                    if v & mask != 0 {
                        if let MapTo::Axis { max, .. } = m.to {
                            return max;
                        }
                    }
                }
            }
        }
        0.0
    }
}
