//! Pure-Rust port of the linux-gamepad library + gamewindow gamepad glue
//! (replaces `mcpelauncher-linux-gamepad` and the gamepad bits of
//! `mcpelauncher-gamewindow`).

pub mod ffi;
pub mod gamepad;
pub mod ids;
pub mod joystick;
pub mod manager;
pub mod mapping;
pub mod window;

use std::cell::RefCell;
use std::rc::Rc;

use window::GamepadWindowManager;

/// Raw pointer to the leaked program-lifetime manager. `Rc`/`RefCell` are not
/// `Send`/`Sync`, but the manager is only ever touched from the main and game
/// threads (never concurrently — same single-threaded guarantee as the C++
/// `LinuxGamepadJoystickManager::instance` singleton).
struct GamepadSingleton(*const RefCell<GamepadWindowManager>);
unsafe impl Send for GamepadSingleton {}
unsafe impl Sync for GamepadSingleton {}

static GAMEPAD: std::sync::OnceLock<GamepadSingleton> = std::sync::OnceLock::new();

fn get() -> Rc<RefCell<GamepadWindowManager>> {
    let ptr = GAMEPAD
        .get_or_init(|| GamepadSingleton(Rc::into_raw(GamepadWindowManager::new())))
        .0;
    unsafe {
        Rc::increment_strong_count(ptr);
        Rc::from_raw(ptr)
    }
}

/// Initialize udev/joystick stack (idempotent; first window creation also does this).
pub fn initialize() {
    get().borrow().initialize();
}

/// Poll joysticks + hotplug events. Called once per event-loop iteration while
/// the window is focused (replaces `WindowWithLinuxJoystick::updateGamepad`).
pub fn update() {
    get().borrow().update();
}

/// Register the (first) window so already-connected gamepads get reported.
pub fn add_window() {
    get().borrow().add_window();
}

/// Focus change event (replaces `LinuxGamepadJoystickManager::onWindowFocused`).
pub fn on_window_focused(focused: bool) {
    get().borrow().on_window_focused(focused);
}

/// Load SDL gamecontrollerdb mappings from a file (skips blank/`#` lines).
pub fn load_mappings_from_file(path: &str) {
    get().borrow().load_mappings_from_file(path);
}

/// Add SDL gamecontrollerdb mappings from inline content.
pub fn load_mappings(content: &str) {
    get().borrow().load_mappings(content);
}

/// extern "C" entry points used by the C++ EGLUTWindowManager glue
/// (window_manager_eglut.cpp).
#[no_mangle]
pub extern "C" fn gamepad_load_mappings_from_file(path: *const std::ffi::c_char) {
    if path.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
    load_mappings_from_file(&s);
}

#[no_mangle]
pub extern "C" fn gamepad_load_mappings(content: *const std::ffi::c_char) {
    if content.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(content) }.to_string_lossy();
    load_mappings(&s);
}
