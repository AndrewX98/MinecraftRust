//! GamepadManager — maps raw joystick events to logical gamepad buttons/axes
//! (ported from `gamepad_manager.cpp`).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use super::gamepad::Gamepad;
use super::ids::{GamepadAxis, GamepadButton};
use super::joystick::LinuxJoystick;
use super::joystick::LinuxJoystickManager;
use super::mapping::{GamepadMapping, MapFrom, MapTo};

/// Callback lists subscribed by the window glue (replaces `CallbackList`).
pub struct GamepadCallbacks {
    pub on_connected: Vec<Rc<dyn Fn(Rc<RefCell<Gamepad>>)>>,
    pub on_disconnected: Vec<Rc<dyn Fn(Rc<RefCell<Gamepad>>)>>,
    pub on_button: Vec<Rc<dyn Fn(Rc<RefCell<Gamepad>>, GamepadButton, bool)>>,
    pub on_axis: Vec<Rc<dyn Fn(Rc<RefCell<Gamepad>>, GamepadAxis, f32)>>,
}

impl Default for GamepadCallbacks {
    fn default() -> Self {
        GamepadCallbacks {
            on_connected: Vec::new(),
            on_disconnected: Vec::new(),
            on_button: Vec::new(),
            on_axis: Vec::new(),
        }
    }
}

/// A gamepad-level event produced from a raw joystick event.
pub enum GamepadOutput {
    Button { gp: Rc<RefCell<Gamepad>>, btn: GamepadButton, state: bool },
    Axis { gp: Rc<RefCell<Gamepad>>, axis: GamepadAxis, value: f32 },
}

pub struct GamepadManager {
    pub callbacks: GamepadCallbacks,
    mappings: HashMap<String, Rc<RefCell<GamepadMapping>>>,
    default_mapping: Rc<RefCell<GamepadMapping>>,
    gamepads: HashMap<usize, Rc<RefCell<Gamepad>>>,
    taken_gamepad_ids: BTreeSet<i32>,
    taken_gamepad_low_id: i32,
}

impl GamepadManager {
    /// Create a manager, subscribing it to the joystick manager's callbacks.
    /// Returns an `Rc` so the manager can subscribe closures that weakly
    /// reference it and re-enter it safely from within callbacks.
    pub fn new(js_manager: Rc<RefCell<LinuxJoystickManager>>) -> Rc<RefCell<GamepadManager>> {
        let manager = Rc::new(RefCell::new(GamepadManager {
            callbacks: GamepadCallbacks::default(),
            mappings: HashMap::new(),
            default_mapping: Rc::new(RefCell::new(GamepadMapping::default())),
            gamepads: HashMap::new(),
            taken_gamepad_ids: BTreeSet::new(),
            taken_gamepad_low_id: 0,
        }));
        let weak = Rc::downgrade(&manager);

        {
            let mut jsm = js_manager.borrow_mut();
            jsm.callbacks.on_connected.push(Box::new({
                let weak = weak.clone();
                move |js: Rc<RefCell<LinuxJoystick>>| {
                    if let Some(gm) = weak.upgrade() {
                        let gp = gm.borrow_mut().on_joystick_connected(js);
                        if let Some(gp) = gp {
                            let cbs = gm.borrow().callbacks.on_connected.clone();
                            for cb in cbs {
                                cb(gp.clone());
                            }
                        }
                    }
                }
            }));
            jsm.callbacks.on_disconnected.push(Box::new({
                let weak = weak.clone();
                move |js: Rc<RefCell<LinuxJoystick>>| {
                    if let Some(gm) = weak.upgrade() {
                        let gp = gm.borrow_mut().on_joystick_disconnected(js);
                        if let Some(gp) = gp {
                            let cbs = gm.borrow().callbacks.on_disconnected.clone();
                            for cb in cbs {
                                cb(gp.clone());
                            }
                        }
                    }
                }
            }));
            jsm.callbacks.on_button.push(Box::new({
                let weak = weak.clone();
                move |js: Rc<RefCell<LinuxJoystick>>, button: i32, state: bool| {
                    if let Some(gm) = weak.upgrade() {
                        let outputs = gm.borrow().on_joystick_button(&js, button, state);
                        dispatch_outputs(&gm, outputs);
                    }
                }
            }));
            jsm.callbacks.on_axis.push(Box::new({
                let weak = weak.clone();
                move |js: Rc<RefCell<LinuxJoystick>>, axis: i32, value: f32| {
                    if let Some(gm) = weak.upgrade() {
                        let outputs = gm.borrow().on_joystick_axis(&js, axis, value);
                        dispatch_outputs(&gm, outputs);
                    }
                }
            }));
            jsm.callbacks.on_hat.push(Box::new({
                let weak = weak.clone();
                move |js: Rc<RefCell<LinuxJoystick>>, hat: i32, value: i32| {
                    if let Some(gm) = weak.upgrade() {
                        let outputs = gm.borrow().on_joystick_hat(&js, hat, value);
                        dispatch_outputs(&gm, outputs);
                    }
                }
            }));
        }

        manager
    }

    pub fn add_mapping(&mut self, mapping: GamepadMapping) {
        let guid = mapping.guid.clone();
        self.mappings.insert(guid, Rc::new(RefCell::new(mapping)));
    }

    pub fn add_mapping_str(&mut self, mapping: &str) -> Result<(), String> {
        let mut m = GamepadMapping::default();
        m.parse(mapping)?;
        self.add_mapping(m);
        Ok(())
    }

    pub fn has_mapping(&self, guid: &str) -> bool {
        self.mappings.contains_key(guid)
    }

    fn take_gamepad_id(&mut self) -> i32 {
        let mut p = self.taken_gamepad_low_id - 1;
        for id in self.taken_gamepad_ids.range(self.taken_gamepad_low_id..) {
            if *id == p + 1 {
                p = *id;
            } else {
                break;
            }
        }
        if p + 1 == self.taken_gamepad_low_id {
            self.taken_gamepad_low_id += 1;
        }
        p + 1
    }

    fn put_gamepad_id_back(&mut self, i: i32) {
        if i < self.taken_gamepad_low_id {
            self.taken_gamepad_low_id = i;
        }
        self.taken_gamepad_ids.remove(&i);
    }

    fn get_mapping_for(&self, js: &Rc<RefCell<LinuxJoystick>>) -> Rc<RefCell<GamepadMapping>> {
        let guid = js.borrow().get_guid();
        self.mappings
            .get(&guid)
            .cloned()
            .unwrap_or_else(|| self.default_mapping.clone())
    }

    fn on_joystick_connected(&mut self, js: Rc<RefCell<LinuxJoystick>>) -> Option<Rc<RefCell<Gamepad>>> {
        let gpi = self.take_gamepad_id();
        let mapping = self.get_mapping_for(&js);
        let gp = Rc::new(RefCell::new(Gamepad::new(gpi, js.clone(), mapping)));
        self.gamepads.insert(Rc::as_ptr(&js) as usize, gp.clone());
        Some(gp)
    }

    fn on_joystick_disconnected(&mut self, js: Rc<RefCell<LinuxJoystick>>) -> Option<Rc<RefCell<Gamepad>>> {
        let gp = self.gamepads.remove(&(Rc::as_ptr(&js) as usize))?;
        self.put_gamepad_id_back(gp.borrow().get_index());
        Some(gp)
    }

    fn on_joystick_button(
        &self,
        js: &Rc<RefCell<LinuxJoystick>>,
        button: i32,
        state: bool,
    ) -> Vec<GamepadOutput> {
        let mut out = Vec::new();
        let Some(gp) = self.gamepads.get(&(Rc::as_ptr(js) as usize)).cloned() else {
            return out;
        };
        let maps = gp.borrow().get_mapping().borrow().mappings.clone();
        for m in &maps {
            if let MapFrom::Button { id } = m.from {
                if id != button {
                    continue;
                }
                match m.to {
                    MapTo::Button { id: btn } => {
                        out.push(GamepadOutput::Button { gp: gp.clone(), btn, state });
                    }
                    MapTo::Axis { id, min, max } => {
                        let v = if state { max } else { min };
                        out.push(GamepadOutput::Axis { gp: gp.clone(), axis: id, value: v });
                    }
                }
            }
        }
        out
    }

    fn on_joystick_axis(
        &self,
        js: &Rc<RefCell<LinuxJoystick>>,
        axis: i32,
        value: f32,
    ) -> Vec<GamepadOutput> {
        let mut out = Vec::new();
        let Some(gp) = self.gamepads.get(&(Rc::as_ptr(js) as usize)).cloned() else {
            return out;
        };
        let maps = gp.borrow().get_mapping().borrow().mappings.clone();
        for m in &maps {
            if let MapFrom::Axis { id, .. } = m.from {
                if id != axis {
                    continue;
                }
                match m.to {
                    MapTo::Button { id: btn } => {
                        let state = GamepadMapping::is_axis_active(&m.from, value);
                        out.push(GamepadOutput::Button { gp: gp.clone(), btn, state });
                    }
                    MapTo::Axis { id: ax, .. } => {
                        let v = GamepadMapping::get_axis_transformed_value(m, value);
                        if !v.is_nan() {
                            out.push(GamepadOutput::Axis { gp: gp.clone(), axis: ax, value: v });
                        }
                    }
                }
            }
        }
        out
    }

    fn on_joystick_hat(
        &self,
        js: &Rc<RefCell<LinuxJoystick>>,
        hat: i32,
        value: i32,
    ) -> Vec<GamepadOutput> {
        let mut out = Vec::new();
        let Some(gp) = self.gamepads.get(&(Rc::as_ptr(js) as usize)).cloned() else {
            return out;
        };
        let maps = gp.borrow().get_mapping().borrow().mappings.clone();
        for m in &maps {
            if let MapFrom::Hat { id, mask } = m.from {
                if id != hat {
                    continue;
                }
                let state = (value & mask) != 0;
                match m.to {
                    MapTo::Button { id: btn } => {
                        out.push(GamepadOutput::Button { gp: gp.clone(), btn, state });
                    }
                    MapTo::Axis { id: ax, min, max } => {
                        let v = if state { max } else { min };
                        out.push(GamepadOutput::Axis { gp: gp.clone(), axis: ax, value: v });
                    }
                }
            }
        }
        out
    }
}

/// Fire gamepad-level callbacks for a batch of outputs. The callback lists are
/// cloned under a short borrow so subscriber callbacks may re-enter the manager.
fn dispatch_outputs(gm: &Rc<RefCell<GamepadManager>>, outputs: Vec<GamepadOutput>) {
    let cbs_button = gm.borrow().callbacks.on_button.clone();
    let cbs_axis = gm.borrow().callbacks.on_axis.clone();
    for out in outputs {
        match out {
            GamepadOutput::Button { gp, btn, state } => {
                for cb in &cbs_button {
                    cb(gp.clone(), btn, state);
                }
            }
            GamepadOutput::Axis { gp, axis, value } => {
                for cb in &cbs_axis {
                    cb(gp.clone(), axis, value);
                }
            }
        }
    }
}
