//! Rust `WindowCallbacks`.
//!
//! The C++ `window_callbacks_stub.cpp` is deleted; this module owns the full
//! input dispatch. `window_callbacks_register` sets the eglut event callbacks
//! directly (the C++ `EGLUTWindow` trampolines are gone — idle/display
//! are not set because the game drives rendering via FakeEGL). Statistically-unreachable
//! branches from the C++ original were dropped: `inputQueue.addEvent` paths
//! (game is always a game activity), `emulateTouch`, and the direct
//! mouse/keyboard feeds.
//!
//! The `callbacks` token stays an opaque `*mut c_void`; the Rust `FakeLooper`
//! owns it in per-thread state and exposes it via
//! `crate::fake_looper::current_callbacks()` (the eglut trampolines and the
//! gamepad dispatch run on the same game thread that called `prepare`).

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::time::{Duration, Instant};

// ============================================================
// Android constants (android-support-headers/android/input.h)
// ============================================================

mod android {
    pub const AINPUT_SOURCE_KEYBOARD: i32 = 0x101;
    pub const AINPUT_SOURCE_GAMEPAD: i32 = 0x401;
    pub const AINPUT_SOURCE_TOUCHSCREEN: i32 = 0x1002;
    pub const AINPUT_SOURCE_MOUSE: i32 = 0x2002;
    pub const AINPUT_SOURCE_MOUSE_RELATIVE: i32 = 0x20004;

    pub const AKEY_EVENT_ACTION_DOWN: i32 = 0;
    pub const AKEY_EVENT_ACTION_UP: i32 = 1;

    pub const AMOTION_EVENT_ACTION_DOWN: i32 = 0;
    pub const AMOTION_EVENT_ACTION_UP: i32 = 1;
    pub const AMOTION_EVENT_ACTION_MOVE: i32 = 2;
    pub const AMOTION_EVENT_ACTION_HOVER_MOVE: i32 = 7;
    pub const AMOTION_EVENT_ACTION_SCROLL: i32 = 8;
    pub const AMOTION_EVENT_ACTION_BUTTON_PRESS: i32 = 11;
    pub const AMOTION_EVENT_ACTION_BUTTON_RELEASE: i32 = 12;

    pub const AMOTION_EVENT_AXIS_X: usize = 0;
    pub const AMOTION_EVENT_AXIS_Y: usize = 1;
    pub const AMOTION_EVENT_AXIS_VSCROLL: usize = 9;
    pub const AMOTION_EVENT_AXIS_RX: usize = 12;
    pub const AMOTION_EVENT_AXIS_RY: usize = 13;
    pub const AMOTION_EVENT_AXIS_HAT_X: usize = 15;
    pub const AMOTION_EVENT_AXIS_HAT_Y: usize = 16;
    pub const AMOTION_EVENT_AXIS_GAS: usize = 22;
    pub const AMOTION_EVENT_AXIS_BRAKE: usize = 23;

    pub const AMETA_SHIFT_ON: i32 = 0x01;
    pub const AMETA_ALT_ON: i32 = 0x02;
    pub const AMETA_CTRL_ON: i32 = 0x1000;
    pub const AMETA_META_ON: i32 = 0x10000;
    pub const AMETA_CAPS_LOCK_ON: i32 = 0x100000;
    pub const AMETA_NUM_LOCK_ON: i32 = 0x200000;
}

// ============================================================
// Action constants (key_mapping.h enum values)
// ============================================================

mod action {
    pub const KEY_PRESS: i32 = 0;
    pub const KEY_REPEAT: i32 = 1;
    pub const KEY_RELEASE: i32 = 2;
    pub const MOUSE_PRESS: i32 = 0;
    pub const MOUSE_RELEASE: i32 = 1;
}

// ============================================================
// KeyCode values (key_mapping.h enum class KeyCode)
// ============================================================

mod keycode {
    pub const UNKNOWN: i32 = 0;
    pub const BACKSPACE: i32 = 8;
    pub const TAB: i32 = 9;
    pub const ENTER: i32 = 13;
    pub const LEFT_SHIFT: i32 = 16;
    pub const RIGHT_SHIFT: i32 = 16 | 256;
    pub const LEFT_CTRL: i32 = 17;
    pub const RIGHT_CTRL: i32 = 17 | 256;
    pub const PAUSE: i32 = 19;
    pub const CAPS_LOCK: i32 = 20;
    pub const ESCAPE: i32 = 27;
    pub const PAGE_UP: i32 = 33;
    pub const PAGE_DOWN: i32 = 34;
    pub const END: i32 = 35;
    pub const HOME: i32 = 36;
    pub const LEFT: i32 = 37;
    pub const UP: i32 = 38;
    pub const RIGHT: i32 = 39;
    pub const DOWN: i32 = 40;
    pub const INSERT: i32 = 45;
    pub const DELETE: i32 = 46;
    pub const NUM_0: i32 = 48;
    pub const NUM_9: i32 = 57;
    pub const NUMPAD_0: i32 = 0x60;
    pub const NUMPAD_MULTIPLY: i32 = 0x6a;
    pub const NUMPAD_DIVIDE: i32 = 0x6f;
    pub const A: i32 = 65;
    pub const C: i32 = 67;
    pub const Z: i32 = 90;
    pub const FN1: i32 = 112;
    pub const FN11: i32 = 122;
    pub const FN12: i32 = 123;
    pub const NUM_LOCK: i32 = 144;
    pub const SCROLL_LOCK: i32 = 145;
    pub const SEMICOLON: i32 = 186;
    pub const EQUAL: i32 = 187;
    pub const COMMA: i32 = 188;
    pub const MINUS: i32 = 189;
    pub const PERIOD: i32 = 190;
    pub const SLASH: i32 = 191;
    pub const GRAVE: i32 = 192;
    pub const LEFT_BRACKET: i32 = 219;
    pub const BACKSLASH: i32 = 220;
    pub const RIGHT_BRACKET: i32 = 221;
    pub const APOSTROPHE: i32 = 222;
    pub const MENU: i32 = 255;
    pub const LEFT_ALT: i32 = 0x12;
    pub const RIGHT_ALT: i32 = 0x12 | 256;
}

// ============================================================
// GamepadButtonId / GamepadAxisId (key_mapping.h enum values)
// ============================================================

mod gamepad_button {
    pub const A: usize = 0;
    pub const B: usize = 1;
    pub const X: usize = 2;
    pub const Y: usize = 3;
    pub const LB: usize = 4;
    pub const RB: usize = 5;
    pub const BACK: usize = 6;
    pub const START: usize = 7;
    pub const GUIDE: usize = 8;
    pub const LEFT_STICK: usize = 9;
    pub const RIGHT_STICK: usize = 10;
    pub const DPAD_UP: usize = 11;
    pub const DPAD_RIGHT: usize = 12;
    pub const DPAD_DOWN: usize = 13;
    pub const DPAD_LEFT: usize = 14;
    pub const BUTTON_COUNT: usize = 15;
}

mod gamepad_axis {
    pub const LEFT_X: usize = 0;
    pub const LEFT_Y: usize = 1;
    pub const RIGHT_X: usize = 2;
    pub const RIGHT_Y: usize = 3;
    pub const LEFT_TRIGGER: usize = 4;
    pub const RIGHT_TRIGGER: usize = 5;
    pub const AXIS_COUNT: usize = 6;
}

// ============================================================
// Input mode enum (window_callbacks.h `enum class InputMode`)
// ============================================================

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(i32)]
enum InputMode {
    Touch = 0,
    Mouse = 1,
    Gamepad = 2,
    Unknown = 3,
}

fn input_mode_from_i32(v: i32) -> InputMode {
    match v {
        0 => InputMode::Touch,
        1 => InputMode::Mouse,
        2 => InputMode::Gamepad,
        _ => InputMode::Unknown,
    }
}

// ============================================================
// Event structs (game_activity_events.h) — layouts must match
// exactly; the pointers are forwarded to the game's JNI callbacks.
// ============================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct GameActivityPointerAxes {
    id: i32,
    tool_type: i32,
    axis_values: [f32; 48],
    raw_x: f32,
    raw_y: f32,
}

#[repr(C)]
struct GameActivityMotionEvent {
    device_id: i32,
    source: i32,
    action: i32,
    event_time: i64,
    down_time: i64,
    flags: i32,
    meta_state: i32,
    action_button: i32,
    button_state: i32,
    classification: i32,
    edge_flags: i32,
    pointer_count: u32,
    pointers: [GameActivityPointerAxes; 8],
    history_size: i32,
    historical_event_times_millis: *mut i64,
    historical_event_times_nanos: *mut i64,
    historical_axis_values: *mut f32,
    precision_x: f32,
    precision_y: f32,
}

#[repr(C)]
struct GameActivityKeyEvent {
    device_id: i32,
    source: i32,
    action: i32,
    event_time: i64,
    down_time: i64,
    flags: i32,
    meta_state: i32,
    modifiers: i32,
    repeat_count: i32,
    key_code: i32,
    scan_code: i32,
    unicode_char: i32,
}

// ============================================================
// Callback registry entries (window_callbacks.h)
// ============================================================

type KeyboardCallback = extern "C" fn(*mut c_void, i32, i32) -> bool;
type MouseButtonCallback = extern "C" fn(*mut c_void, f64, f64, i32, i32) -> bool;
type MousePositionCallback = extern "C" fn(*mut c_void, f64, f64, bool) -> bool;
type MouseScrollCallback = extern "C" fn(*mut c_void, f64, f64, f64, f64) -> bool;

struct CallbackEntry<T> {
    user: *mut c_void,
    cb: T,
}

#[derive(Clone)]
struct GamepadData {
    axis: [f32; 6],
    button: [bool; 15],
}

impl GamepadData {
    fn new() -> Self {
        GamepadData { axis: [0.0; 6], button: [false; 15] }
    }
}

// ============================================================
// WindowCallbacks state
// ============================================================

pub struct WindowCallbacks {
    window: *mut c_void,
    rust_jni_support: *mut c_void,

    keyboard_callbacks: Vec<CallbackEntry<KeyboardCallback>>,
    mouse_button_callbacks: Vec<CallbackEntry<MouseButtonCallback>>,
    mouse_position_callbacks: Vec<CallbackEntry<MousePositionCallback>>,
    mouse_scroll_callbacks: Vec<CallbackEntry<MouseScrollCallback>>,

    gamepads: HashMap<i32, GamepadData>,
    button_state: i32,
    delayed_paste: u8,
    last_paste_str: Vec<u8>,
    needs_queue_gamepad_input: bool,
    send_events: bool,
    cursor_locked: bool,
    menubarsize: i32,
    use_raw_input: bool,
    forced_mode: InputMode,
    input_mode: InputMode,
    input_mode_switch_delay: i32,
    last_updated: Instant,

    last_mouse_x: i32,
    last_mouse_y: i32,
    width: i32,
    height: i32,
    pointer_ids: [i32; 16],
}

impl WindowCallbacks {
    fn new(window: *mut c_void, rust_jni_support: *mut c_void) -> Self {
        let mut w = 0;
        let mut h = 0;
        unsafe { eglutGetWindowSize(&mut w, &mut h) };
        WindowCallbacks {
            window,
            rust_jni_support,
            keyboard_callbacks: Vec::new(),
            mouse_button_callbacks: Vec::new(),
            mouse_position_callbacks: Vec::new(),
            mouse_scroll_callbacks: Vec::new(),
            gamepads: HashMap::new(),
            button_state: 0,
            delayed_paste: 0,
            last_paste_str: Vec::new(),
            needs_queue_gamepad_input: true,
            send_events: false,
            cursor_locked: false,
            menubarsize: 0,
            use_raw_input: false,
            forced_mode: InputMode::Unknown,
            input_mode: InputMode::Unknown,
            input_mode_switch_delay: 100,
            last_updated: Instant::now(),
            last_mouse_x: 0,
            last_mouse_y: 0,
            width: w,
            height: h,
            pointer_ids: [-1; 16],
        }
    }

    fn has_input_mode(&mut self, want: InputMode, change_mode: bool) -> bool {
        if !self.send_events {
            return false;
        }
        if self.use_raw_input {
            return true;
        }
        if self.forced_mode != InputMode::Unknown {
            return want == self.forced_mode;
        }
        let now = Instant::now();
        if self.input_mode == want
            || (change_mode
                && (want < self.input_mode
                    || now.duration_since(self.last_updated)
                        > Duration::from_millis(self.input_mode_switch_delay as u64)))
        {
            if self.input_mode != want {
                if want == InputMode::Mouse {
                    self.set_cursor_disabled(self.cursor_locked);
                } else {
                    self.set_cursor_disabled(true);
                }
            }
            self.input_mode = want;
            self.last_updated = now;
            return true;
        }
        false
    }

    fn set_cursor_disabled(&self, disabled: bool) {
        if !disabled && std::env::var_os("GAMEWINDOW_CENTER_CURSOR").is_none() {
            unsafe { eglutWarpMousePointer(self.last_mouse_x, self.last_mouse_y) };
        }
        unsafe { eglutSetMousePointerLocked(if disabled { 1 } else { 0 }) };
    }

    fn set_window_fullscreen(&self, fullscreen: bool) {
        let cur = unsafe { eglutGet(1) }; // EGLUT_FULLSCREEN_MODE
        let want = if fullscreen { 1 } else { 0 };
        if cur != want {
            unsafe { eglutToggleFullscreen() };
        }
    }

    fn on_window_size_callback(&self, w: i32, h: i32) {
        unsafe { crate::jni_support::jni_support_on_window_resized(self.rust_jni_support, w, h - self.menubarsize) };
    }

    fn on_close() {
        unsafe { libc::_exit(0) };
    }

    fn set_cursor_locked(&mut self, locked: bool) {
        self.cursor_locked = locked;
        if self.has_input_mode(InputMode::Mouse, false) {
            self.set_cursor_disabled(locked);
        }
    }

    fn set_fullscreen(&mut self, is_fs: bool) {
        if unsafe { mc_settings_get_fullscreen() } != is_fs {
            self.set_window_fullscreen(is_fs);
            unsafe { mc_settings_set_fullscreen(is_fs) };
            unsafe { mc_settings_save() };
        }
    }

    fn on_mouse_button(&mut self, x: f64, y: f64, btn: i32, action: i32) {
        if !self.has_input_mode(InputMode::Mouse, true) {
            return;
        }
        let mut it = self.mouse_button_callbacks.iter();
        while let Some(e) = it.next() {
            if (e.cb)(e.user, x, y, btn, action) {
                return;
            }
        }
        if btn < 1 {
            return;
        }
        if btn > 3 {
            return self.on_keyboard(
                btn,
                if action == action::MOUSE_PRESS { action::KEY_PRESS } else { action::KEY_RELEASE },
                0,
            );
        }
        if action == action::MOUSE_PRESS {
            self.button_state |= map_mouse_button_to_android(btn);
        } else {
            self.button_state &= !map_mouse_button_to_android(btn);
        }
        let android_action = if action == action::MOUSE_PRESS {
            android::AMOTION_EVENT_ACTION_BUTTON_PRESS
        } else {
            android::AMOTION_EVENT_ACTION_BUTTON_RELEASE
        };
        self.send_mouse_event(
            android::AINPUT_SOURCE_MOUSE,
            0,
            android_action,
            self.button_state,
            x as f32,
            (y - self.menubarsize as f64) as f32,
            0.0,
        );
    }

    fn on_mouse_position(&mut self, x: f64, y: f64) {
        if !self.has_input_mode(InputMode::Mouse, true) {
            return;
        }
        let mut it = self.mouse_position_callbacks.iter();
        while let Some(e) = it.next() {
            if (e.cb)(e.user, x, y, false) {
                return;
            }
        }
        self.send_mouse_event(
            android::AINPUT_SOURCE_MOUSE,
            0,
            android::AMOTION_EVENT_ACTION_HOVER_MOVE,
            self.button_state,
            x as f32,
            (y - self.menubarsize as f64) as f32,
            0.0,
        );
    }

    fn on_mouse_relative_position(&mut self, x: f64, y: f64) {
        if !self.has_input_mode(InputMode::Mouse, x.abs() > 10.0 || y.abs() > 10.0) {
            return;
        }
        let mut it = self.mouse_position_callbacks.iter();
        while let Some(e) = it.next() {
            if (e.cb)(e.user, x, y, true) {
                return;
            }
        }
        self.send_mouse_event(
            android::AINPUT_SOURCE_MOUSE_RELATIVE,
            0,
            android::AMOTION_EVENT_ACTION_HOVER_MOVE,
            self.button_state,
            x as f32,
            y as f32,
            0.0,
        );
    }

    fn on_mouse_scroll(&mut self, x: f64, y: f64, dx: f64, dy: f64) {
        if !self.has_input_mode(InputMode::Mouse, true) {
            return;
        }
        let mut it = self.mouse_scroll_callbacks.iter();
        while let Some(e) = it.next() {
            if (e.cb)(e.user, x, y, dx, dy) {
                return;
            }
        }
        let cdy = (dy * 127.0).clamp(-127.0, 127.0) as i8;
        self.send_mouse_event(
            android::AINPUT_SOURCE_MOUSE,
            0,
            android::AMOTION_EVENT_ACTION_SCROLL,
            self.button_state,
            x as f32,
            (y - self.menubarsize as f64) as f32,
            cdy as f32,
        );
    }

    fn send_mouse_event(
        &self,
        source: i32,
        device_id: i32,
        action: i32,
        button_state: i32,
        x: f32,
        y: f32,
        scroll_y: f32,
    ) {
        let mut event: GameActivityMotionEvent = unsafe { std::mem::zeroed() };
        event.source = source;
        event.device_id = device_id;
        event.action = action;
        event.button_state = button_state;
        event.precision_x = x;
        event.precision_y = y;
        event.pointer_count = 2;
        event.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_X] = x;
        event.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_Y] = y;
        event.pointers[0].raw_x = x;
        event.pointers[0].raw_y = x;
        event.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_VSCROLL] = scroll_y;
        unsafe { crate::jni_support::jni_support_send_motion_event(self.rust_jni_support, &event as *const _ as *const c_void) };
    }

    fn on_touch_start(&mut self, id: i32, x: f64, y: f64) {
        if self.has_input_mode(InputMode::Touch, true) {
            self.send_touch_event(id, android::AMOTION_EVENT_ACTION_DOWN, x as f32, (y - self.menubarsize as f64) as f32);
        }
    }

    fn on_touch_update(&mut self, id: i32, x: f64, y: f64) {
        if self.has_input_mode(InputMode::Touch, true) {
            self.send_touch_event(id, android::AMOTION_EVENT_ACTION_MOVE, x as f32, (y - self.menubarsize as f64) as f32);
        }
    }

    fn on_touch_end(&mut self, id: i32, x: f64, y: f64) {
        if self.has_input_mode(InputMode::Touch, true) {
            self.send_touch_event(id, android::AMOTION_EVENT_ACTION_UP, x as f32, (y - self.menubarsize as f64) as f32);
        }
    }

    fn send_touch_event(&self, pointer_id: i32, action: i32, x: f32, y: f32) {
        let mut ev: GameActivityMotionEvent = unsafe { std::mem::zeroed() };
        ev.source = android::AINPUT_SOURCE_TOUCHSCREEN;
        ev.action = action;
        ev.pointer_count = 1;
        ev.device_id = 0;
        ev.pointers[0].id = pointer_id;
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_X] = x;
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_Y] = y;
        ev.pointers[0].raw_x = x;
        ev.pointers[0].raw_y = y;
        unsafe { crate::jni_support::jni_support_send_motion_event(self.rust_jni_support, &ev as *const _ as *const c_void) };
    }

    fn on_keyboard(&mut self, key: i32, action: i32, mods: i32) {
        if !self.has_input_mode(InputMode::Mouse, true) {
            return;
        }
        let mut it = self.keyboard_callbacks.iter();
        while let Some(e) = it.next() {
            if (e.cb)(e.user, key, action) {
                return;
            }
        }
        let mod_ctrl = mods & 2; // KEY_MOD_CTRL
        let text_handler = unsafe { crate::jnivm_globals::jnivm_get_text_input_handler() };
        let mut copy_len: usize = 0;
        let copy_text =
            unsafe { crate::text_input_handler::text_handler_get_copy_text(text_handler, &mut copy_len) };
        if mod_ctrl != 0 && action == action::KEY_PRESS && key == keycode::C && copy_len > 0 {
            unsafe { eglutSetClipboardText(copy_text) };
        } else {
            unsafe { crate::text_input_handler::text_handler_on_key_pressed(text_handler, key, action, mods) };
        }

        if key == keycode::FN11 && action == action::KEY_PRESS {
            let fs = unsafe { mc_settings_get_fullscreen() };
            self.set_fullscreen(!fs);
        }

        let mut state = 0i32;
        if mods & 1 != 0 {
            state |= android::AMETA_SHIFT_ON; // KEY_MOD_SHIFT
        }
        if mods & 8 != 0 {
            state |= android::AMETA_ALT_ON; // KEY_MOD_ALT
        }
        if mods & 2 != 0 {
            state |= android::AMETA_CTRL_ON; // KEY_MOD_CTRL
        }
        if mods & 4 != 0 {
            state |= android::AMETA_META_ON; // KEY_MOD_SUPER
        }
        if mods & 16 != 0 {
            state |= android::AMETA_CAPS_LOCK_ON; // KEY_MOD_CAPSLOCK
        }
        if mods & 32 != 0 {
            state |= android::AMETA_NUM_LOCK_ON; // KEY_MOD_NUMLOCK
        }

        let mut event: GameActivityKeyEvent = unsafe { std::mem::zeroed() };
        event.device_id = 0;
        event.source = android::AINPUT_SOURCE_KEYBOARD;
        event.action = if action == action::KEY_PRESS {
            android::AKEY_EVENT_ACTION_DOWN
        } else {
            android::AKEY_EVENT_ACTION_UP
        };
        event.meta_state = state;
        event.key_code = map_minecraft_to_android_key(key);
        if action == action::KEY_PRESS {
            unsafe { crate::jni_support::jni_support_send_key_down(self.rust_jni_support, &event as *const _ as *const c_void) };
        } else if action == action::KEY_RELEASE {
            unsafe { crate::jni_support::jni_support_send_key_up(self.rust_jni_support, &event as *const _ as *const c_void) };
        }
    }

    fn on_keyboard_text(&mut self, c: &[u8]) {
        let text_handler = unsafe { crate::jnivm_globals::jnivm_get_text_input_handler() };
        if c == b"\n" && !unsafe { crate::text_input_handler::text_input_handler_is_multiline(text_handler) } {
            unsafe { crate::jni_support::jni_support_on_return_key_pressed(self.rust_jni_support) };
        } else {
            let mut data = c.to_vec();
            data.push(0);
            unsafe {
                crate::text_input_handler::text_handler_on_text_input(text_handler, data.as_ptr() as *const c_char)
            };
        }
    }

    fn on_drop(&mut self, path: *const c_char) {
        unsafe { crate::jni_support::jni_support_import_file(self.rust_jni_support, path) };
    }

    fn on_paste(&mut self, str_ptr: *const c_char, len: i32) {
        if str_ptr.is_null() {
            return;
        }
        let data = unsafe { std::slice::from_raw_parts(str_ptr as *const u8, len as usize) };
        if unsafe { mc_settings_get_enable_keyboard_autofocus_paste_patches_1_20_60() } {
            self.last_paste_str = data.to_vec();
        }
        let mut cdata = data.to_vec();
        cdata.push(0);
        let text_handler = unsafe { crate::jnivm_globals::jnivm_get_text_input_handler() };
        unsafe { crate::text_input_handler::text_handler_on_text_input(text_handler, cdata.as_ptr() as *const c_char) };
    }

    fn on_gamepad_state(&mut self, gamepad: i32, connected: bool) {
        log::trace!("Gamepad {} #{}", if connected { "connected" } else { "disconnected" }, gamepad);
        if connected {
            self.gamepads.insert(gamepad, GamepadData::new());
        } else {
            self.gamepads.remove(&gamepad);
        }
        if self.send_events {
            unsafe { crate::jni_support::jni_support_set_game_controller_connected(self.rust_jni_support, gamepad, connected) };
        }
    }

    fn queue_gamepad_axis_input_if_needed(&mut self, gamepad: i32) {
        // is_game_activity is always true in this build; the C++ gating
        // (`!needsQueueGamepadInput && !isGameActivity`) never returns early.
        let gp = match self.gamepads.get(&gamepad) {
            Some(g) => g.clone(),
            None => return,
        };
        let mut ev: GameActivityMotionEvent = unsafe { std::mem::zeroed() };
        ev.source = android::AINPUT_SOURCE_GAMEPAD;
        ev.device_id = gamepad;
        ev.action = android::AMOTION_EVENT_ACTION_MOVE;
        ev.pointer_count = 1;
        ev.pointers[0].id = 0;
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_X] = gp.axis[gamepad_axis::LEFT_X];
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_Y] = gp.axis[gamepad_axis::LEFT_Y];
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_RX] = gp.axis[gamepad_axis::RIGHT_X];
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_RY] = gp.axis[gamepad_axis::RIGHT_Y];
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_BRAKE] = gp.axis[gamepad_axis::LEFT_TRIGGER];
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_GAS] = gp.axis[gamepad_axis::RIGHT_TRIGGER];
        let mut hat_x = 0.0f32;
        if gp.button[gamepad_button::DPAD_LEFT] {
            hat_x = -1.0;
        }
        if gp.button[gamepad_button::DPAD_RIGHT] {
            hat_x = 1.0;
        }
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_HAT_X] = hat_x;
        let mut hat_y = 0.0f32;
        if gp.button[gamepad_button::DPAD_UP] {
            hat_y = -1.0;
        }
        if gp.button[gamepad_button::DPAD_DOWN] {
            hat_y = 1.0;
        }
        ev.pointers[0].axis_values[android::AMOTION_EVENT_AXIS_HAT_Y] = hat_y;
        unsafe { crate::jni_support::jni_support_send_motion_event(self.rust_jni_support, &ev as *const _ as *const c_void) };
        self.needs_queue_gamepad_input = false;
    }

    fn on_gamepad_button(&mut self, gamepad: i32, btn: i32, pressed: bool) {
        if !self.has_input_mode(InputMode::Gamepad, true) {
            return;
        }
        if !self.gamepads.contains_key(&gamepad) {
            return;
        }
        if btn < 0 || btn >= gamepad_button::BUTTON_COUNT as i32 {
            panic!("bad button id");
        }
        let gp = self.gamepads.get_mut(&gamepad).unwrap();
        let idx = btn as usize;
        if gp.button[idx] == pressed {
            return;
        }
        gp.button[idx] = pressed;

        if idx == gamepad_button::DPAD_UP
            || idx == gamepad_button::DPAD_DOWN
            || idx == gamepad_button::DPAD_LEFT
            || idx == gamepad_button::DPAD_RIGHT
        {
            self.queue_gamepad_axis_input_if_needed(gamepad);
            return;
        }

        let mut event: GameActivityKeyEvent = unsafe { std::mem::zeroed() };
        event.device_id = gamepad;
        event.source = android::AINPUT_SOURCE_GAMEPAD;
        event.action = if pressed { android::AKEY_EVENT_ACTION_DOWN } else { android::AKEY_EVENT_ACTION_UP };
        event.key_code = map_gamepad_to_android_key(btn);
        if pressed {
            unsafe { crate::jni_support::jni_support_send_key_down(self.rust_jni_support, &event as *const _ as *const c_void) };
        } else {
            unsafe { crate::jni_support::jni_support_send_key_up(self.rust_jni_support, &event as *const _ as *const c_void) };
        }
    }

    fn on_gamepad_axis(&mut self, gamepad: i32, ax: i32, value: f32) {
        if !self.has_input_mode(InputMode::Gamepad, value.abs() > 0.4) {
            return;
        }
        if !self.gamepads.contains_key(&gamepad) {
            return;
        }
        if ax < 0 || ax >= gamepad_axis::AXIS_COUNT as i32 {
            panic!("bad axis id");
        }
        self.gamepads.get_mut(&gamepad).unwrap().axis[ax as usize] = value;
        self.queue_gamepad_axis_input_if_needed(gamepad);
    }

    fn add_keyboard_callback(&mut self, user: *mut c_void, callback: KeyboardCallback) {
        self.keyboard_callbacks.push(CallbackEntry { user, cb: callback });
    }

    fn add_mouse_button_callback(&mut self, user: *mut c_void, callback: MouseButtonCallback) {
        self.mouse_button_callbacks.push(CallbackEntry { user, cb: callback });
    }

    fn add_mouse_position_callback(&mut self, user: *mut c_void, callback: MousePositionCallback) {
        self.mouse_position_callbacks.push(CallbackEntry { user, cb: callback });
    }

    fn add_mouse_scroll_callback(&mut self, user: *mut c_void, callback: MouseScrollCallback) {
        self.mouse_scroll_callbacks.push(CallbackEntry { user, cb: callback });
    }

    fn set_delayed_paste(&mut self) {
        self.delayed_paste = 2;
    }

    fn start_send_events(&mut self) {
        if !self.send_events {
            self.send_events = true;
            let gamepads: Vec<i32> = self.gamepads.keys().copied().collect();
            for gp in gamepads {
                unsafe { crate::jni_support::jni_support_set_game_controller_connected(self.rust_jni_support, gp, true) };
            }
        }
        let next_size = unsafe { mc_settings_get_menubarsize() };
        if next_size != self.menubarsize {
            self.menubarsize = next_size;
            let mut w = 0;
            let mut h = 0;
            unsafe { eglutGetWindowSize(&mut w, &mut h) };
            self.on_window_size_callback(w, h);
        }
        if self.delayed_paste > 0 {
            self.delayed_paste -= 1;
            if self.delayed_paste == 0 {
                let text_handler = unsafe { crate::jnivm_globals::jnivm_get_text_input_handler() };
                unsafe { crate::text_input_handler::text_handler_on_text_input(text_handler, c"\x08".as_ptr()) };
                let mut data = self.last_paste_str.clone();
                data.push(0);
                unsafe {
                    crate::text_input_handler::text_handler_on_text_input(text_handler, data.as_ptr() as *const c_char)
                };
            }
        }
    }

    fn mark_requeue_gamepad_input(&mut self) {
        self.needs_queue_gamepad_input = true;
    }

    fn get_input_mode(&self) -> i32 {
        self.input_mode as i32
    }

    fn obtain_touch_pointer(&mut self, eglut_id: i32) -> i32 {
        for i in 0..self.pointer_ids.len() {
            if self.pointer_ids[i] == eglut_id {
                return i as i32;
            }
        }
        for i in 0..self.pointer_ids.len() {
            if self.pointer_ids[i] == -1 {
                self.pointer_ids[i] = eglut_id;
                return i as i32;
            }
        }
        self.pointer_ids.len() as i32 + eglut_id
    }

    fn release_touch_pointer(&mut self, our_id: i32) {
        if our_id >= 0 && (our_id as usize) < self.pointer_ids.len() {
            self.pointer_ids[our_id as usize] = -1;
        }
    }
}

// ============================================================
// eglut externs (all Rust exports in crate::eglut)
// ============================================================

extern "C" {
    fn eglutReshapeFunc(func: Option<unsafe extern "C" fn(i32, i32)>);
    fn eglutMouseFunc(func: Option<unsafe extern "C" fn(i32, i32)>);
    fn eglutMouseRawFunc(func: Option<unsafe extern "C" fn(f64, f64)>);
    fn eglutMouseButtonFunc(func: Option<unsafe extern "C" fn(i32, i32, i32, i32)>);
    fn eglutTouchStartFunc(func: Option<unsafe extern "C" fn(i32, f64, f64)>);
    fn eglutTouchUpdateFunc(func: Option<unsafe extern "C" fn(i32, f64, f64)>);
    fn eglutTouchEndFunc(func: Option<unsafe extern "C" fn(i32, f64, f64)>);
    fn eglutKeyboardFunc(func: Option<unsafe extern "C" fn(*mut c_char, i32)>);
    fn eglutDropFunc(func: Option<unsafe extern "C" fn(*const c_char)>);
    fn eglutSpecialFunc(func: Option<unsafe extern "C" fn(i32, i32, u32)>);
    fn eglutPasteFunc(func: Option<unsafe extern "C" fn(*const c_char, i32)>);
    fn eglutFocusFunc(func: Option<unsafe extern "C" fn(i32)>);
    fn eglutCloseWindowFunc(func: Option<unsafe extern "C" fn()>);
    fn eglutSetMousePointerLocked(locked: i32);
    fn eglutWarpMousePointer(x: i32, y: i32);
    fn eglutSetClipboardText(text: *const c_char);
    fn eglutRequestPaste();
    fn eglutToggleFullscreen();
    fn eglutGet(param: i32) -> i32;
    fn eglutGetWindowSize(w: *mut i32, h: *mut i32);
}

// ============================================================
// Settings + text-handler FFI (settings_stub.cpp / text_input_handler.rs)
// ============================================================

extern "C" {
    fn mc_settings_get_menubarsize() -> i32;
    fn mc_settings_get_enable_keyboard_autofocus_paste_patches_1_20_60() -> bool;
    fn mc_settings_get_fullscreen() -> bool;
    fn mc_settings_set_fullscreen(fs: bool);
    fn mc_settings_save();
}

// ============================================================
// Key mapping (reuses rust_bridge.rs exports)
// ============================================================

fn map_mouse_button_to_android(btn: i32) -> i32 {
    crate::rust_bridge::window_callbacks_map_mouse_button(btn)
}

fn map_minecraft_to_android_key(code: i32) -> i32 {
    crate::rust_bridge::window_callbacks_map_minecraft_key(code)
}

fn map_gamepad_to_android_key(btn: i32) -> i32 {
    crate::rust_bridge::window_callbacks_map_gamepad_key(btn)
}

// ============================================================
// getKeyMinecraft — X11 keysym -> Minecraft KeyCode
// (window_eglut.cpp `EGLUTWindow::getKeyMinecraft`)
// ============================================================

mod xk {
    // ASCII
    pub const A: i32 = 0x41;
    pub const Z: i32 = 0x5a;
    pub const a: i32 = 0x61;
    pub const z: i32 = 0x7a;
    pub const exclam: i32 = 0x21;
    pub const at: i32 = 0x40;
    pub const numbersign: i32 = 0x23;
    pub const dollar: i32 = 0x24;
    pub const percent: i32 = 0x25;
    pub const asciicircum: i32 = 0x5e;
    pub const ampersand: i32 = 0x26;
    pub const asterisk: i32 = 0x2a;
    pub const parenleft: i32 = 0x28;
    pub const parenright: i32 = 0x29;
    pub const underscore: i32 = 0x5f;
    pub const plus: i32 = 0x2b;
    pub const semicolon: i32 = 0x3b;
    pub const equal: i32 = 0x3d;
    pub const comma: i32 = 0x2c;
    pub const minus: i32 = 0x2d;
    pub const period: i32 = 0x2e;
    pub const slash: i32 = 0x2f;
    pub const grave: i32 = 0x60;
    pub const bracketleft: i32 = 0x5b;
    pub const backslash: i32 = 0x5c;
    pub const bracketright: i32 = 0x5d;
    pub const apostrophe: i32 = 0x27;
    // Misc
    pub const BackSpace: i32 = 0xff08;
    pub const Tab: i32 = 0xff09;
    pub const ISO_Left_Tab: i32 = 0xfe20;
    pub const Return: i32 = 0xff0d;
    pub const Pause: i32 = 0xff13;
    pub const Scroll_Lock: i32 = 0xff14;
    pub const Escape: i32 = 0xff1b;
    pub const Home: i32 = 0xff50;
    pub const Left: i32 = 0xff51;
    pub const Up: i32 = 0xff52;
    pub const Right: i32 = 0xff53;
    pub const Down: i32 = 0xff54;
    pub const Page_Up: i32 = 0xff55;
    pub const Page_Down: i32 = 0xff56;
    pub const End: i32 = 0xff57;
    pub const Insert: i32 = 0xff63;
    pub const Num_Lock: i32 = 0xff7f;
    pub const Delete: i32 = 0xffff;
    pub const Shift_L: i32 = 0xffe1;
    pub const Shift_R: i32 = 0xffe2;
    pub const Control_L: i32 = 0xffe3;
    pub const Control_R: i32 = 0xffe4;
    pub const Caps_Lock: i32 = 0xffe5;
    pub const Alt_L: i32 = 0xffe9;
    pub const Alt_R: i32 = 0xffea;
    // Function
    pub const F1: i32 = 0xffbe;
    pub const F12: i32 = 0xffc9;
    // Keypad
    pub const KP_0: i32 = 0xffb0;
    pub const KP_9: i32 = 0xffb9;
    pub const KP_Multiply: i32 = 0xffaa;
    pub const KP_Divide: i32 = 0xffaf;
    pub const KP_Enter: i32 = 0xff8d;
    pub const KP_Home: i32 = 0xff95;
    pub const KP_Up: i32 = 0xff97;
    pub const KP_Prior: i32 = 0xff9a;
    pub const KP_Left: i32 = 0xff96;
    pub const KP_Right: i32 = 0xff98;
    pub const KP_End: i32 = 0xff9c;
    pub const KP_Down: i32 = 0xff99;
    pub const KP_Next: i32 = 0xff9b;
}

fn get_key_minecraft(key_code: i32) -> i32 {
    if key_code >= xk::A && key_code <= xk::Z {
        return key_code - xk::A + keycode::A;
    }
    if key_code >= xk::a && key_code <= xk::z {
        return key_code - xk::a + keycode::A;
    }
    if key_code >= xk::F1 && key_code <= xk::F12 {
        return key_code - xk::F1 + keycode::FN1;
    }
    if key_code >= xk::KP_0 && key_code <= xk::KP_9 {
        return key_code - xk::KP_0 + keycode::NUMPAD_0;
    }
    if key_code >= xk::KP_Multiply && key_code <= xk::KP_Divide {
        return key_code - xk::KP_Multiply + keycode::NUMPAD_MULTIPLY;
    }
    if key_code >= xk::KP_Home && key_code <= xk::KP_Down {
        return key_code - xk::KP_Home + keycode::HOME;
    }
    if key_code >= xk::KP_Prior && key_code <= xk::KP_End {
        return key_code - xk::KP_Prior + keycode::PAGE_UP;
    }
    match key_code {
        xk::exclam => keycode::NUM_0 + 1,
        xk::at => keycode::NUM_0 + 2,
        xk::numbersign => keycode::NUM_0 + 3,
        xk::dollar => keycode::NUM_0 + 4,
        xk::percent => keycode::NUM_0 + 5,
        xk::asciicircum => keycode::NUM_0 + 6,
        xk::ampersand => keycode::NUM_0 + 7,
        xk::asterisk => keycode::NUM_0 + 8,
        xk::parenleft => keycode::NUM_0 + 9,
        xk::parenright => keycode::NUM_0,
        xk::underscore => keycode::MINUS,
        xk::plus => keycode::EQUAL,
        xk::BackSpace => keycode::BACKSPACE,
        xk::ISO_Left_Tab | xk::Tab => keycode::TAB,
        xk::Return => keycode::ENTER,
        xk::Shift_L => keycode::LEFT_SHIFT,
        xk::Shift_R => keycode::RIGHT_SHIFT,
        xk::Control_L => keycode::LEFT_CTRL,
        xk::Control_R => keycode::RIGHT_CTRL,
        xk::Pause => keycode::PAUSE,
        xk::Caps_Lock => keycode::CAPS_LOCK,
        xk::Escape => keycode::ESCAPE,
        xk::Page_Up => keycode::PAGE_UP,
        xk::Page_Down => keycode::PAGE_DOWN,
        xk::End => keycode::END,
        xk::Home => keycode::HOME,
        xk::Left => keycode::LEFT,
        xk::Up => keycode::UP,
        xk::Right => keycode::RIGHT,
        xk::Down => keycode::DOWN,
        xk::Insert => keycode::INSERT,
        xk::Delete => keycode::DELETE,
        xk::Num_Lock => keycode::NUM_LOCK,
        xk::Scroll_Lock => keycode::SCROLL_LOCK,
        xk::semicolon => keycode::SEMICOLON,
        xk::equal => keycode::EQUAL,
        xk::comma => keycode::COMMA,
        xk::minus => keycode::MINUS,
        xk::period => keycode::PERIOD,
        xk::slash => keycode::SLASH,
        xk::grave => keycode::GRAVE,
        xk::bracketleft => keycode::LEFT_BRACKET,
        xk::backslash => keycode::BACKSLASH,
        xk::bracketright => keycode::RIGHT_BRACKET,
        xk::apostrophe => keycode::APOSTROPHE,
        xk::Alt_L => keycode::LEFT_ALT,
        xk::Alt_R => keycode::RIGHT_ALT,
        xk::KP_Enter => keycode::ENTER,
        _ => {
            if key_code < 256 {
                key_code
            } else {
                keycode::UNKNOWN
            }
        }
    }
}

// ============================================================
// translateMeta (window_eglut.cpp)
// ============================================================

fn translate_meta(meta: u32) -> i32 {
    let mut mods = 0;
    if meta & 1 != 0 {
        mods |= 1; // ShiftMask -> KEY_MOD_SHIFT
    }
    if meta & 4 != 0 {
        mods |= 2; // ControlMask -> KEY_MOD_CTRL
    }
    if meta & 8 != 0 {
        mods |= 8; // Mod1Mask -> KEY_MOD_ALT
    }
    if meta & 64 != 0 {
        mods |= 4; // Mod4Mask -> KEY_MOD_SUPER
    }
    if meta & 2 != 0 {
        mods |= 16; // LockMask -> KEY_MOD_CAPSLOCK
    }
    if meta & 16 != 0 {
        mods |= 32; // Mod2Mask -> KEY_MOD_NUMLOCK
    }
    mods
}

// ============================================================
// Env helpers (util.h ReadEnvFlag / ReadEnvInt)
// ============================================================

fn read_env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => v == "true" || v == "1" || v == "on",
        Err(_) => false,
    }
}

fn read_env_int(name: &str, def: i32) -> i32 {
    match std::env::var(name) {
        Ok(v) => v.parse::<i32>().unwrap_or(def),
        Err(_) => def,
    }
}

// ============================================================
// Active-token resolution
// ============================================================

unsafe fn current_callbacks() -> *mut WindowCallbacks {
    crate::fake_looper::current_callbacks() as *mut WindowCallbacks
}

// ============================================================
// eglut trampolines (replace the C++ EGLUTWindow event handlers)
// ============================================================

unsafe extern "C" fn eglut_cb_reshape(w: i32, h: i32) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    if c.width == w && c.height == h {
        return;
    }
    c.width = w;
    c.height = h;
    c.on_window_size_callback(w, h);
}

unsafe extern "C" fn eglut_cb_mouse(x: i32, y: i32) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    c.last_mouse_x = x;
    c.last_mouse_y = y;
    c.on_mouse_position(x as f64, y as f64);
}

unsafe extern "C" fn eglut_cb_mouse_raw(x: f64, y: f64) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    (&mut *cb).on_mouse_relative_position(x, y);
}

unsafe extern "C" fn eglut_cb_mouse_button(x: i32, y: i32, btn: i32, action: i32) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    if (btn == 4 || btn == 5) && action == 0 {
        c.on_mouse_scroll(x as f64, y as f64, 0.0, if btn == 5 { -1.0 } else { 1.0 });
        return;
    }
    if (btn == 6 || btn == 7) && action == 0 {
        c.on_mouse_scroll(x as f64, y as f64, if btn == 7 { -1.0 } else { 1.0 }, 0.0);
        return;
    }
    let b = if btn == 2 {
        3
    } else if btn == 3 {
        2
    } else {
        btn
    };
    let a = if action == 0 { action::MOUSE_PRESS } else { action::MOUSE_RELEASE };
    c.on_mouse_button(x as f64, y as f64, b, a);
}

unsafe extern "C" fn eglut_cb_touch_start(id: i32, x: f64, y: f64) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    let our_id = c.obtain_touch_pointer(id);
    c.on_touch_start(our_id, x, y);
}

unsafe extern "C" fn eglut_cb_touch_update(id: i32, x: f64, y: f64) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    let our_id = c.obtain_touch_pointer(id);
    c.on_touch_update(our_id, x, y);
}

unsafe extern "C" fn eglut_cb_touch_end(id: i32, x: f64, y: f64) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    let our_id = c.obtain_touch_pointer(id);
    c.on_touch_end(our_id, x, y);
    c.release_touch_pointer(our_id);
}

unsafe extern "C" fn eglut_cb_keyboard(buf: *mut c_char, action: i32) {
    let cb = current_callbacks();
    if cb.is_null() || buf.is_null() {
        return;
    }
    let bytes = CStr::from_ptr(buf).to_bytes();
    if matches!(bytes, b"\t" | b"\x03" | b"\x16" | b"\x1b") {
        return;
    }
    if action == 0 || action == 2 {
        let mut data = bytes.to_vec();
        if data == b"\r" {
            data = b"\n".to_vec();
        }
        (&mut *cb).on_keyboard_text(&data);
    }
}

unsafe extern "C" fn eglut_cb_special(key: i32, action: i32, meta: u32) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    let c = &mut *cb;
    let mods = translate_meta(meta);
    let m_key = get_key_minecraft(key);
    let enum_action = if action == 0 {
        action::KEY_PRESS
    } else if action == 2 {
        action::KEY_REPEAT
    } else {
        action::KEY_RELEASE
    };
    if m_key != keycode::UNKNOWN {
        c.on_keyboard(m_key, enum_action, mods);
    }
    if (key == 86 || key == 118) && mods & 2 != 0 && action == 0 {
        eglutRequestPaste();
    }
}

unsafe extern "C" fn eglut_cb_drop(path: *const c_char) {
    let cb = current_callbacks();
    if cb.is_null() || path.is_null() {
        return;
    }
    (&mut *cb).on_drop(path);
}

unsafe extern "C" fn eglut_cb_paste(str_ptr: *const c_char, len: i32) {
    let cb = current_callbacks();
    if cb.is_null() {
        return;
    }
    (&mut *cb).on_paste(str_ptr, len);
}

unsafe extern "C" fn eglut_cb_focus(_action: i32) {
    // Gamepad focus tracking is handled by the Rust eglut event loop.
}

unsafe extern "C" fn eglut_cb_close() {
    WindowCallbacks::on_close();
}

// ============================================================
// Exported FFI surface
// ============================================================

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_create(
    window: *mut c_void,
    rust_jni_support: *mut c_void,
    _input_queue: *mut c_void,
) -> *mut c_void {
    let mut cb = Box::new(WindowCallbacks::new(window, rust_jni_support));
    cb.use_raw_input = read_env_flag("MCPELAUNCHER_CLIENT_RAW_INPUT");
    cb.forced_mode = input_mode_from_i32(read_env_int(
        "MCPELAUNCHER_CLIENT_FORCED_INPUT_MODE",
        InputMode::Unknown as i32,
    ));
    cb.input_mode_switch_delay = read_env_int("MCPELAUNCHER_CLIENT_INPUT_SWITCH_DELAY", 100);
    if mc_settings_get_fullscreen() {
        cb.set_window_fullscreen(true);
    }
    Box::into_raw(cb) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_register(w: *mut c_void) {
    if w.is_null() {
        return;
    }
    // Idle/display are intentionally NOT set: the game drives rendering itself
    // through FakeEGL (`fake_egl_swap_buffers` → `game_window_swap_buffers` →
    // `eglutSwapBuffers`), so no redraw callback is needed (the C++ EGLUTWindow
    // that used to register `eglutIdleFunc`/`eglutDisplayFunc` is gone).
    eglutReshapeFunc(Some(eglut_cb_reshape));
    eglutMouseFunc(Some(eglut_cb_mouse));
    eglutMouseRawFunc(Some(eglut_cb_mouse_raw));
    eglutMouseButtonFunc(Some(eglut_cb_mouse_button));
    eglutTouchStartFunc(Some(eglut_cb_touch_start));
    eglutTouchUpdateFunc(Some(eglut_cb_touch_update));
    eglutTouchEndFunc(Some(eglut_cb_touch_end));
    eglutKeyboardFunc(Some(eglut_cb_keyboard));
    eglutDropFunc(Some(eglut_cb_drop));
    eglutSpecialFunc(Some(eglut_cb_special));
    eglutPasteFunc(Some(eglut_cb_paste));
    eglutFocusFunc(Some(eglut_cb_focus));
    eglutCloseWindowFunc(Some(eglut_cb_close));
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_destroy(w: *mut c_void) {
    if w.is_null() {
        return;
    }
    drop(Box::from_raw(w as *mut WindowCallbacks));
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_load_gamepad_mappings() {
    let mut paths = crate::path_helper::find_all_data_files("gamecontrollerdb.txt");
    paths.reverse();
    for p in paths {
        log::trace!("Loading gamepad mappings: {}", p);
        crate::gamepad::load_mappings_from_file(&p);
    }
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_start_send_events(w: *mut c_void) {
    if w.is_null() {
        return;
    }
    (&mut *(w as *mut WindowCallbacks)).start_send_events();
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_mark_requeue_gamepad(w: *mut c_void) {
    if w.is_null() {
        return;
    }
    (&mut *(w as *mut WindowCallbacks)).mark_requeue_gamepad_input();
}

// --- CorePatches-backed helpers (fully Rust) ---

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_get_input_mode(callbacks: *mut c_void) -> i32 {
    if callbacks.is_null() {
        return 0;
    }
    (*(callbacks as *const WindowCallbacks)).get_input_mode()
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_set_cursor_locked(callbacks: *mut c_void, locked: bool) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).set_cursor_locked(locked);
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_set_fullscreen(callbacks: *mut c_void, fs: bool) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).set_fullscreen(fs);
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_set_delayed_paste(callbacks: *mut c_void) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).set_delayed_paste();
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_add_keyboard_callback(
    callbacks: *mut c_void,
    user: *mut c_void,
    cb: KeyboardCallback,
) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).add_keyboard_callback(user, cb);
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_add_mouse_button_callback(
    callbacks: *mut c_void,
    user: *mut c_void,
    cb: MouseButtonCallback,
) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).add_mouse_button_callback(user, cb);
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_add_mouse_position_callback(
    callbacks: *mut c_void,
    user: *mut c_void,
    cb: MousePositionCallback,
) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).add_mouse_position_callback(user, cb);
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_add_mouse_scroll_callback(
    callbacks: *mut c_void,
    user: *mut c_void,
    cb: MouseScrollCallback,
) {
    if callbacks.is_null() {
        return;
    }
    (&mut *(callbacks as *mut WindowCallbacks)).add_mouse_scroll_callback(user, cb);
}

// --- Gamepad dispatch thunks (called from the Rust gamepad stack) ---

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_on_gamepad_state(gamepad: i32, connected: bool) {
    let cb = current_callbacks();
    if !cb.is_null() {
        (&mut *cb).on_gamepad_state(gamepad, connected);
    }
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_on_gamepad_button(gamepad: i32, btn: i32, pressed: bool) {
    let cb = current_callbacks();
    if !cb.is_null() {
        (&mut *cb).on_gamepad_button(gamepad, btn, pressed);
    }
}

#[no_mangle]
pub unsafe extern "C" fn window_callbacks_on_gamepad_axis(gamepad: i32, axis: i32, value: f32) {
    let cb = current_callbacks();
    if !cb.is_null() {
        (&mut *cb).on_gamepad_axis(gamepad, axis, value);
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cb() -> WindowCallbacks {
        let mut cb = WindowCallbacks::new(std::ptr::null_mut(), std::ptr::null_mut());
        cb.send_events = true;
        cb.input_mode_switch_delay = 100;
        cb
    }

    #[test]
    fn key_minecraft_alphanumeric() {
        assert_eq!(get_key_minecraft(xk::A), keycode::A);
        assert_eq!(get_key_minecraft(xk::Z), keycode::Z);
        assert_eq!(get_key_minecraft(xk::a), keycode::A);
        assert_eq!(get_key_minecraft(xk::z), keycode::Z);
    }

    #[test]
    fn key_minecraft_special() {
        assert_eq!(get_key_minecraft(xk::Return), keycode::ENTER);
        assert_eq!(get_key_minecraft(xk::Left), keycode::LEFT);
        assert_eq!(get_key_minecraft(xk::F1), keycode::FN1);
        assert_eq!(get_key_minecraft(xk::F12), keycode::FN12);
        assert_eq!(get_key_minecraft(xk::KP_0), keycode::NUMPAD_0);
        assert_eq!(get_key_minecraft(xk::KP_9), keycode::NUMPAD_0 + 9);
        assert_eq!(get_key_minecraft(xk::KP_Multiply), keycode::NUMPAD_MULTIPLY);
        assert_eq!(get_key_minecraft(xk::KP_Divide), keycode::NUMPAD_DIVIDE);
        assert_eq!(get_key_minecraft(xk::KP_Home), keycode::HOME);
        assert_eq!(get_key_minecraft(xk::KP_End), keycode::END);
        assert_eq!(get_key_minecraft(xk::KP_Enter), keycode::ENTER);
        assert_eq!(get_key_minecraft(xk::exclam), keycode::NUM_0 + 1);
        assert_eq!(get_key_minecraft(xk::parenright), keycode::NUM_0);
        assert_eq!(get_key_minecraft(xk::underscore), keycode::MINUS);
        assert_eq!(get_key_minecraft(xk::plus), keycode::EQUAL);
        assert_eq!(get_key_minecraft(0x20), 0x20);
        assert_eq!(get_key_minecraft(0x1234), keycode::UNKNOWN);
    }

    #[test]
    fn map_minecraft_key_to_android() {
        assert_eq!(map_minecraft_to_android_key(keycode::A), 29);
        assert_eq!(map_minecraft_to_android_key(keycode::ENTER), 66);
        assert_eq!(map_minecraft_to_android_key(keycode::ESCAPE), 111);
        assert_eq!(map_minecraft_to_android_key(keycode::LEFT_CTRL), 113);
        assert_eq!(map_minecraft_to_android_key(0), 0);
    }

    #[test]
    fn map_mouse_and_gamepad() {
        assert_eq!(map_mouse_button_to_android(1), 1);
        assert_eq!(map_mouse_button_to_android(2), 2);
        assert_eq!(map_mouse_button_to_android(3), 4);
        assert_eq!(map_mouse_button_to_android(8), 8);
        assert_eq!(map_mouse_button_to_android(9), 16);
        assert_eq!(map_gamepad_to_android_key(0), 96);
        assert_eq!(map_gamepad_to_android_key(14), 21);
        assert_eq!(map_gamepad_to_android_key(5), 103);
    }

    #[test]
    fn translate_meta_masks() {
        assert_eq!(translate_meta(1), 1); // shift
        assert_eq!(translate_meta(4), 2); // ctrl
        assert_eq!(translate_meta(8), 8); // alt
        assert_eq!(translate_meta(64), 4); // super
        assert_eq!(translate_meta(2), 16); // caps
        assert_eq!(translate_meta(16), 32); // numlock
        assert_eq!(translate_meta(1 | 4), 3);
    }

    #[test]
    fn input_mode_switching() {
        let mut cb = make_cb();
        assert_eq!(cb.get_input_mode(), InputMode::Unknown as i32);
        assert!(!cb.has_input_mode(InputMode::Touch, false));
        cb.send_events = false;
        assert!(!cb.has_input_mode(InputMode::Mouse, true));
        cb.send_events = true;
        assert!(cb.has_input_mode(InputMode::Mouse, true));
        assert_eq!(cb.get_input_mode(), InputMode::Mouse as i32);
        // Upward switch within the delay window is rejected.
        assert!(!cb.has_input_mode(InputMode::Gamepad, true));
        // ... but accepted once the delay elapses.
        cb.last_updated = Instant::now() - Duration::from_millis(200);
        assert!(cb.has_input_mode(InputMode::Gamepad, true));
    }

    #[test]
    fn forced_input_mode() {
        let mut cb = make_cb();
        cb.forced_mode = InputMode::Touch;
        assert!(cb.has_input_mode(InputMode::Touch, true));
        assert!(!cb.has_input_mode(InputMode::Mouse, true));
    }

    #[test]
    fn raw_input_overrides_mode() {
        let mut cb = make_cb();
        cb.use_raw_input = true;
        assert!(cb.has_input_mode(InputMode::Gamepad, false));
    }

    #[test]
    fn touch_pointer_ids_are_reused() {
        let mut cb = WindowCallbacks::new(std::ptr::null_mut(), std::ptr::null_mut());
        let p0 = cb.obtain_touch_pointer(100);
        let p1 = cb.obtain_touch_pointer(200);
        assert_ne!(p0, p1);
        assert_eq!(cb.obtain_touch_pointer(100), p0);
        cb.release_touch_pointer(p0);
        assert_eq!(cb.obtain_touch_pointer(300), p0);
    }

    #[test]
    fn gamepad_data_state() {
        let mut cb = make_cb();
        cb.on_gamepad_state(0, true);
        assert!(cb.gamepads.contains_key(&0));
        cb.on_gamepad_button(0, gamepad_button::A as i32, true);
        assert!(cb.gamepads.get(&0).unwrap().button[0]);
        cb.on_gamepad_axis(0, gamepad_axis::LEFT_X as i32, 0.5);
        assert_eq!(cb.gamepads.get(&0).unwrap().axis[0], 0.5);
        cb.on_gamepad_state(0, false);
        assert!(!cb.gamepads.contains_key(&0));
    }
}
