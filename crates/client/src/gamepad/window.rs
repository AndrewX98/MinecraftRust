//! Window glue — `LinuxGamepadJoystickManager` equivalent (ported from
//! `joystick_manager_linux_gamepad.cpp` / `joystick_manager.cpp`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use super::gamepad::Gamepad;
use super::ids::{GamepadAxis, GamepadButton};
use super::joystick::LinuxJoystickManager;
use super::manager::GamepadManager;
use super::mapping::GamepadMapping;

extern "C" {
    fn window_callbacks_on_gamepad_state(gamepad: i32, connected: bool);
    fn window_callbacks_on_gamepad_button(gamepad: i32, btn: i32, pressed: bool);
    fn window_callbacks_on_gamepad_axis(gamepad: i32, axis: i32, value: f32);
}

/// Dispatch a gamepad connect/disconnect event to the game (C++ WindowCallbacks).
pub fn dispatch_gamepad_state(gamepad: i32, connected: bool) {
    unsafe { window_callbacks_on_gamepad_state(gamepad, connected) };
}

/// Dispatch a gamepad button event to the game (C++ WindowCallbacks).
pub fn dispatch_gamepad_button(gamepad: i32, btn: i32, pressed: bool) {
    unsafe { window_callbacks_on_gamepad_button(gamepad, btn, pressed) };
}

/// Dispatch a gamepad axis event to the game (C++ WindowCallbacks).
pub fn dispatch_gamepad_axis(gamepad: i32, axis: i32, value: f32) {
    unsafe { window_callbacks_on_gamepad_axis(gamepad, axis, value) };
}

/// Program-lifetime gamepad wiring between the evdev stack, the gamepad manager
/// and the game-facing `WindowCallbacks`.
pub struct GamepadWindowManager {
    js_manager: Rc<RefCell<LinuxJoystickManager>>,
    gamepad_manager: Rc<RefCell<GamepadManager>>,
    focused: Cell<bool>,
    initialized: Cell<bool>,
    window_added: Cell<bool>,
    gamepads: RefCell<HashMap<i32, Rc<RefCell<Gamepad>>>>,
}

impl GamepadWindowManager {
    /// Create the manager stack and subscribe to gamepad-level callbacks.
    /// Returns an `Rc` so subscriptions can weakly reference this manager and
    /// re-enter it safely from within the synchronous callback chain.
    pub fn new() -> Rc<RefCell<GamepadWindowManager>> {
        let js_manager = Rc::new(RefCell::new(LinuxJoystickManager::default()));
        let gamepad_manager = GamepadManager::new(js_manager.clone());
        let this = Rc::new(RefCell::new(GamepadWindowManager {
            js_manager,
            gamepad_manager,
            focused: Cell::new(false),
            initialized: Cell::new(false),
            window_added: Cell::new(false),
            gamepads: RefCell::new(HashMap::new()),
        }));
        let weak = Rc::downgrade(&this);

        {
            let gm = this.borrow().gamepad_manager.clone();
            let mut gmc = gm.borrow_mut();
            gmc.callbacks.on_connected.push(Rc::new({
                let weak = weak.clone();
                move |gp: Rc<RefCell<Gamepad>>| {
                    if let Some(g) = weak.upgrade() {
                        g.borrow().on_gamepad_state(gp, true);
                    }
                }
            }));
            gmc.callbacks.on_disconnected.push(Rc::new({
                let weak = weak.clone();
                move |gp: Rc<RefCell<Gamepad>>| {
                    if let Some(g) = weak.upgrade() {
                        g.borrow().on_gamepad_state(gp, false);
                    }
                }
            }));
            gmc.callbacks.on_button.push(Rc::new({
                let weak = weak.clone();
                move |gp: Rc<RefCell<Gamepad>>, btn: GamepadButton, state: bool| {
                    if let Some(g) = weak.upgrade() {
                        g.borrow().on_gamepad_button(gp, btn, state);
                    }
                }
            }));
            gmc.callbacks.on_axis.push(Rc::new({
                let weak = weak.clone();
                move |gp: Rc<RefCell<Gamepad>>, axis: GamepadAxis, value: f32| {
                    if let Some(g) = weak.upgrade() {
                        g.borrow().on_gamepad_axis(gp, axis, value);
                    }
                }
            }));
        }

        this
    }

    pub fn initialize(&self) {
        if !self.initialized.get() {
            self.initialized.set(true);
            self.js_manager.borrow_mut().initialize();
        }
    }

    /// Poll joysticks (called on each event-loop iteration while focused).
    pub fn update(&self) {
        if !self.focused.get() {
            return;
        }
        self.initialize();
        self.js_manager.borrow_mut().poll();
    }

    pub fn add_window(&self) {
        self.initialize();
        if !self.window_added.get() {
            self.window_added.set(true);
            // First window: report already-connected gamepads so the game learns
            // about them (joysticks connected during initialize() fired before
            // the window existed).
            let gamepads: Vec<Rc<RefCell<Gamepad>>> =
                self.gamepads.borrow().values().cloned().collect();
            for gp in gamepads {
                self.warn_on_missing_gamepad_mapping(&gp);
                let index = gp.borrow().get_index();
                dispatch_gamepad_state(index, true);
            }
        }
    }

    pub fn on_window_focused(&self, focused: bool) {
        self.focused.set(focused);
    }

    pub fn load_mappings_from_file(&self, path: &str) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        for line in content.lines() {
            if !line.is_empty() && !line.starts_with('#') {
                if let Err(e) = self.gamepad_manager.borrow_mut().add_mapping_str(line) {
                    println!("Invalid mapping in {}: {}", path, e);
                }
            }
        }
    }

    pub fn load_mappings(&self, content: &str) {
        for line in content.lines() {
            if let Err(e) = self.gamepad_manager.borrow_mut().add_mapping_str(line) {
                println!("Invalid mapping: {}", e);
            }
        }
    }

    fn on_gamepad_state(&self, gp: Rc<RefCell<Gamepad>>, connected: bool) {
        let index = gp.borrow().get_index();
        if connected {
            self.warn_on_missing_gamepad_mapping(&gp);
            self.gamepads.borrow_mut().insert(index, gp);
            if self.window_added.get() {
                dispatch_gamepad_state(index, true);
            }
        } else {
            self.gamepads.borrow_mut().remove(&index);
            if self.window_added.get() {
                dispatch_gamepad_state(index, false);
            }
        }
    }

    fn on_gamepad_button(&self, gp: Rc<RefCell<Gamepad>>, btn: GamepadButton, state: bool) {
        if !self.focused.get() {
            return;
        }
        let index = gp.borrow().get_index();
        dispatch_gamepad_button(index, btn as i32, state);
    }

    fn on_gamepad_axis(&self, gp: Rc<RefCell<Gamepad>>, axis: GamepadAxis, value: f32) {
        if !self.focused.get() {
            return;
        }
        let index = gp.borrow().get_index();
        dispatch_gamepad_axis(index, axis as i32, value);
    }

    fn warn_on_missing_gamepad_mapping(&self, gp: &Rc<RefCell<Gamepad>>) {
        if !gp.borrow().get_mapping().borrow().mappings.is_empty() {
            return;
        }
        if !self.window_added.get() {
            // No warning before the first window is created
            return;
        }
        let guid = gp.borrow().get_joystick().borrow().get_guid();
        let gm = self.gamepad_manager.clone();
        let gp_rc = gp.clone();
        handle_missing_gamepad_mapping(
            "Unknown",
            &guid,
            4,
            12,
            1,
            move |mapping: String| -> bool {
                for line in mapping.lines() {
                    if let Err(e) = gm.borrow_mut().add_mapping_str(line) {
                        println!("Invalid dummy mapping: {}", e);
                    }
                }
                if gp_rc.borrow().get_mapping().borrow().mappings.is_empty() {
                    // Update Gamepad, needed to add refreshed mapping
                    let mut m = GamepadMapping::default();
                    if m.parse(&mapping).is_ok() {
                        gp_rc.borrow_mut().set_mapping(Rc::new(RefCell::new(m)));
                    }
                }
                !gp_rc.borrow().get_mapping().borrow().mappings.is_empty()
            },
        );
    }
}

/// Dummy mapping generator + warning (ported from `JoystickManager::handleMissingGamePadMapping`).
#[cfg(debug_assertions)]
fn handle_missing_gamepad_mapping(
    name: &str,
    guid: &str,
    axes_count: usize,
    buttons_count: usize,
    hats_count: usize,
    mut update_mapping: impl FnMut(String) -> bool,
) -> bool {
    let mut errormsg =
        format!("Missing Gamepad Mapping for controller '{}'({}). Please create a Gamepad Mapping for your gamepad.", name, guid);
    let mut mapping = format!("{},{}", guid, name);

    let axes = ["leftx", "lefty", "rightx", "righty", "lefttrigger", "righttrigger"];
    if axes_count > 0 {
        for i in 0..axes_count.min(axes.len()) {
            mapping.push_str(&format!(",{}:a{}", axes[i], i));
        }
    }
    let hats = ["dpup", "dpright", "dpdown", "dpleft"];
    if hats_count > 0 {
        for i in 0..hats_count.min(hats.len() / 4) {
            for j in 0..4 {
                mapping.push_str(&format!(",{}:h{}.{}", hats[i * 4 + j], i, 1 << j));
            }
        }
    }
    let btns = [
        "a", "b", "x", "y", "leftshoulder", "rightshoulder", "righttrigger", "lefttrigger",
        "back", "start", "leftstick", "rightstick", "guide", "dpleft", "dpdown", "dpright", "dpup",
    ];
    if buttons_count > 0 {
        for i in 0..buttons_count.min(btns.len()) {
            mapping.push_str(&format!(",{}:b{}", btns[i], i));
        }
    }

    let mapstr = format!("{},platform:Linux,\n{},platform:Mac OS X,", mapping, mapping);
    let has_mapping = update_mapping(mapstr.clone());

    if !has_mapping {
        errormsg.push_str(" Failed to create a valid dummy mapping for this controller, you won't be able to use this controller.");
    } else {
        errormsg.push_str(&format!(
            " This Launcher has created a dummy Gamepad Mapping for you, you will have to create your own for best experience: '{}'",
            mapstr
        ));
    }
    log::warn!("[JoystickManager]: {}", errormsg);
    has_mapping
}

#[cfg(not(debug_assertions))]
fn handle_missing_gamepad_mapping(
    _name: &str,
    _guid: &str,
    _axes_count: usize,
    _buttons_count: usize,
    _hats_count: usize,
    _update_mapping: impl FnMut(String) -> bool,
) -> bool {
    false
}
