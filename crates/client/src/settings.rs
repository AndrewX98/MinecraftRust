//! Rust port of the C++ `settings_stub.cpp` (and previously `settings.cpp`).
//! The C++ `Settings` class is gone — `main.cpp` (the only `Settings::load`
//! caller) was excluded from the Rust build. These `mc_settings_*` accessors
//! are the FFI surface consumed by `crate::window_callbacks`.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static MENUBARSIZE: AtomicI32 = AtomicI32::new(0);
static ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES: AtomicBool = AtomicBool::new(false);
static FULLSCREEN: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn mc_settings_get_menubarsize() -> i32 {
    MENUBARSIZE.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_get_enable_keyboard_autofocus_paste_patches_1_20_60() -> bool {
    ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_get_fullscreen() -> bool {
    FULLSCREEN.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_fullscreen(fs: bool) {
    FULLSCREEN.store(fs, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_save() {}
