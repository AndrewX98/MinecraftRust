//! Logical gamepad button/axis identifiers (matches `gamepad_ids.h`).

/// Logical gamepad buttons (matches `GamepadButton` enum order in C++).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum GamepadButton {
    A,
    B,
    X,
    Y,
    LB,
    RB,
    Back,
    Start,
    Guide,
    LeftStick,
    RightStick,
    DpadUp,
    DpadRight,
    DpadDown,
    DpadLeft,
}

/// Logical gamepad axes (matches `GamepadAxis` enum order in C++).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

pub const GAMEPAD_BUTTON_COUNT: usize = 15;
pub const GAMEPAD_AXIS_COUNT: usize = 6;
