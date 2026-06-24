use gilrs::{Gilrs, Event, EventType};

pub enum InputEvent {
    KeyDown(u32),
    KeyUp(u32),
    MouseMove(f64, f64),
    MouseDown(u32),
    MouseUp(u32),
    MouseWheel(f32),
    GamepadButton(u32, bool),
    GamepadAxis(u32, f32),
    Touch(f64, f64, TouchAction),
}

pub enum TouchAction {
    Down,
    Up,
    Move,
}

pub struct GamepadState {
    gilrs: Option<Gilrs>,
}

impl GamepadState {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().ok();
        Self { gilrs }
    }

    pub fn poll(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        if let Some(ref mut gilrs) = self.gilrs {
            while let Some(Event { event, .. }) = gilrs.next_event() {
                match event {
                    EventType::ButtonChanged(btn, val, _) => {
                        events.push(InputEvent::GamepadButton(
                            btn as u32,
                            val > 0.5,
                        ));
                    }
                    EventType::AxisChanged(axis, val, _) => {
                        events.push(InputEvent::GamepadAxis(
                            axis as u32,
                            val,
                        ));
                    }
                    _ => {}
                }
            }
        }
        events
    }
}
