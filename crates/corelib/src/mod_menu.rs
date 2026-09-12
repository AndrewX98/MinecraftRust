//! Mod menu/window registry — Rust port of the `mcpelauncher_addmenu`,
//! `mcpelauncher_show_window` and `mcpelauncher_close_window` API in upstream
//! `mcpelauncher-client/src/imgui_ui.h` / `imgui_ui.cpp`.
//!
//! Mods call these during `mod_init` (or later, e.g. from a menu-click
//! callback). Everything is deep-copied at registration time: mods commonly
//! pass stack-allocated `control` arrays, so retaining the pointers would
//! dangle. The client crate renders the stored state from its imgui overlay
//! (`client/src/mod_menu.rs`); this crate only owns the data and the C
//! entry points registered in `minecraft_utils::get_api()`.
//!
//! Locking: a single `Mutex<Registry>` guards menus and windows (upstream
//! used two mutexes). Renderer code must use `try_lock_registry()` and skip
//! the frame when it is busy, and must invoke mod callbacks *after*
//! releasing the guard — a click handler typically calls
//! `mcpelauncher_show_window`, which would self-deadlock otherwise.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Mutex, MutexGuard, OnceLock};

// ---- C ABI (matches upstream `imgui_ui.h`) ----

pub type ModMenuSelectedFn = unsafe extern "C" fn(*mut c_void) -> bool;
pub type ModMenuClickFn = unsafe extern "C" fn(*mut c_void);
pub type ModButtonClickFn = unsafe extern "C" fn(*mut c_void);
pub type ModSliderIntChangeFn = unsafe extern "C" fn(*mut c_void, c_int);
pub type ModSliderFloatChangeFn = unsafe extern "C" fn(*mut c_void, f32);
pub type ModTextInputChangeFn = unsafe extern "C" fn(*mut c_void, *const c_char);
pub type ModWindowCloseFn = unsafe extern "C" fn(*mut c_void);

/// Upstream `MenuEntryABI`. `selected` returns `bool` upstream; mods written
/// against an `int` return type stay compatible (only the low byte is read).
#[repr(C)]
pub struct MenuEntryABI {
    pub name: *const c_char,
    pub user: *mut c_void,
    pub selected: Option<ModMenuSelectedFn>,
    pub click: Option<ModMenuClickFn>,
    pub length: usize,
    pub subentries: *const MenuEntryABI,
}

/// Upstream `control` union, split into `Copy` parts (Rust unions cannot hold
/// non-`Copy` fields; all of these are raw pointers/ints).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModButtonABI {
    pub label: *const c_char,
    pub user: *mut c_void,
    pub on_click: Option<ModButtonClickFn>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModSliderIntABI {
    pub label: *const c_char,
    pub min: c_int,
    pub def: c_int,
    pub max: c_int,
    pub user: *mut c_void,
    pub on_change: Option<ModSliderIntChangeFn>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModSliderFloatABI {
    pub label: *const c_char,
    pub min: f32,
    pub def: f32,
    pub max: f32,
    pub user: *mut c_void,
    pub on_change: Option<ModSliderFloatChangeFn>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModTextABI {
    pub label: *const c_char,
    /// 0 normal / 1 medium / 2 large / 3 very large (upstream comment).
    pub size: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModTextInputABI {
    pub label: *const c_char,
    pub def: *const c_char,
    pub placeholder: *const c_char,
    pub user: *mut c_void,
    pub on_change: Option<ModTextInputChangeFn>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ModControlDataABI {
    pub button: ModButtonABI,
    pub sliderint: ModSliderIntABI,
    pub sliderfloat: ModSliderFloatABI,
    pub text: ModTextABI,
    pub textinput: ModTextInputABI,
}

/// Upstream `control`: `type` 0 = button, 1 = sliderint, 2 = sliderfloat,
/// 3 = text, 4 = textinput.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModControlABI {
    pub type_: c_int,
    pub data: ModControlDataABI,
}

// ---- Owned state (rendered by the client crate) ----

pub struct StoredMenuEntry {
    pub name: String,
    /// Mod `user` pointer as an address (`*mut c_void` is not `Send`, so the
    /// owned state keeps `usize` and casts back at the call site).
    pub user: usize,
    pub selected: Option<ModMenuSelectedFn>,
    pub click: Option<ModMenuClickFn>,
    pub subentries: Vec<StoredMenuEntry>,
}

pub enum StoredControl {
    Button {
        label: String,
        user: usize,
        on_click: Option<ModButtonClickFn>,
    },
    SliderInt {
        label: String,
        min: i32,
        value: i32,
        max: i32,
        user: usize,
        on_change: Option<ModSliderIntChangeFn>,
    },
    SliderFloat {
        label: String,
        min: f32,
        value: f32,
        max: f32,
        user: usize,
        on_change: Option<ModSliderFloatChangeFn>,
    },
    Text {
        label: String,
        size: i32,
    },
    TextInput {
        label: String,
        value: String,
        placeholder: String,
        user: usize,
        on_change: Option<ModTextInputChangeFn>,
    },
}

pub struct StoredWindow {
    pub title: String,
    pub is_modal: bool,
    /// Set false when the user closes the window; the renderer removes it and
    /// fires `on_close` (upstream `ActiveWindow::open`).
    pub open: bool,
    /// Tracks the one-shot `OpenPopup` for modals (upstream `modalOpened`).
    pub modal_opened: bool,
    pub user: usize,
    pub on_close: Option<ModWindowCloseFn>,
    pub controls: Vec<StoredControl>,
}

pub struct Registry {
    pub menus: Vec<StoredMenuEntry>,
    pub windows: Vec<StoredWindow>,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            menus: Vec::new(),
            windows: Vec::new(),
        })
    })
}

/// Renderer entry point: `None` when a mod is registering on another thread
/// (caller must skip the frame, like upstream's `try_lock`).
pub fn try_lock_registry() -> Option<MutexGuard<'static, Registry>> {
    registry().try_lock().ok()
}

fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

/// Depth cap for nested submenus (upstream recurses unchecked).
const MAX_MENU_DEPTH: usize = 8;
/// Sanity cap on controls per window.
const MAX_CONTROLS: usize = 256;

unsafe fn convert_entry(e: &MenuEntryABI, depth: usize) -> StoredMenuEntry {
    let mut subs = Vec::new();
    if !e.subentries.is_null() && e.length > 0 && depth < MAX_MENU_DEPTH {
        for i in 0..e.length {
            subs.push(convert_entry(&*e.subentries.add(i), depth + 1));
        }
    }
    StoredMenuEntry {
        name: cstr_to_string(e.name),
        user: e.user as usize,
        selected: e.selected,
        click: e.click,
        subentries: subs,
    }
}

unsafe fn convert_control(c: &ModControlABI) -> Option<StoredControl> {
    match c.type_ {
        0 => {
            let b = c.data.button;
            Some(StoredControl::Button {
                label: cstr_to_string(b.label),
                user: b.user as usize,
                on_click: b.on_click,
            })
        }
        1 => {
            let s = c.data.sliderint;
            Some(StoredControl::SliderInt {
                label: cstr_to_string(s.label),
                min: s.min as i32,
                value: s.def as i32,
                max: s.max as i32,
                user: s.user as usize,
                on_change: s.on_change,
            })
        }
        2 => {
            let s = c.data.sliderfloat;
            Some(StoredControl::SliderFloat {
                label: cstr_to_string(s.label),
                min: s.min,
                value: s.def,
                max: s.max,
                user: s.user as usize,
                on_change: s.on_change,
            })
        }
        3 => {
            let t = c.data.text;
            Some(StoredControl::Text {
                label: cstr_to_string(t.label),
                size: t.size as i32,
            })
        }
        4 => {
            let t = c.data.textinput;
            Some(StoredControl::TextInput {
                label: cstr_to_string(t.label),
                value: cstr_to_string(t.def),
                placeholder: cstr_to_string(t.placeholder),
                user: t.user as usize,
                on_change: t.on_change,
            })
        }
        _ => None,
    }
}

/// Upstream `mcpelauncher_addmenu(size_t length, MenuEntryABI* entries)`.
/// Appends deep copies to the global menu list (rendered in the Mods menu).
#[no_mangle]
pub unsafe extern "C" fn mcpelauncher_addmenu(length: usize, entries: *const MenuEntryABI) {
    if entries.is_null() || length == 0 {
        return;
    }
    let mut stored = Vec::new();
    for i in 0..length {
        stored.push(convert_entry(&*entries.add(i), 0));
    }
    if let Ok(mut reg) = registry().lock() {
        reg.menus.extend(stored);
    }
}

/// Upstream `mcpelauncher_show_window(...)`. Upserts by title and deep-copies
/// every control (mods typically pass stack arrays).
#[no_mangle]
pub unsafe extern "C" fn mcpelauncher_show_window(
    title: *const c_char,
    is_modal: c_int,
    user: *mut c_void,
    on_close: Option<ModWindowCloseFn>,
    count: c_int,
    controls: *const ModControlABI,
) {
    let title = cstr_to_string(title);
    let title = if title.is_empty() {
        "Untitled".to_string()
    } else {
        title
    };
    let mut stored = Vec::new();
    if !controls.is_null() && count > 0 {
        for i in 0..(count as usize).min(MAX_CONTROLS) {
            if let Some(c) = convert_control(&*controls.add(i)) {
                stored.push(c);
            }
        }
    }
    if let Ok(mut reg) = registry().lock() {
        if let Some(w) = reg.windows.iter_mut().find(|w| w.title == title) {
            w.is_modal = is_modal != 0;
            w.user = user as usize;
            w.on_close = on_close;
            w.controls = stored;
        } else {
            reg.windows.push(StoredWindow {
                title,
                is_modal: is_modal != 0,
                open: true,
                modal_opened: false,
                user: user as usize,
                on_close,
                controls: stored,
            });
        }
    }
}

/// Upstream `mcpelauncher_close_window(const char* title)`.
#[no_mangle]
pub unsafe extern "C" fn mcpelauncher_close_window(title: *const c_char) {
    let title = cstr_to_string(title);
    if let Ok(mut reg) = registry().lock() {
        reg.windows.retain(|w| w.title != title);
    }
}

/// Build a NUL-terminated string for handing owned text back to a mod
/// callback (`textinput` onChange). Returns `None` on interior NUL.
pub fn mod_cstring(s: &str) -> Option<CString> {
    CString::new(s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        if let Ok(mut reg) = registry().lock() {
            reg.menus.clear();
            reg.windows.clear();
        }
    }

    unsafe extern "C" fn yes(_user: *mut c_void) -> bool {
        true
    }
    unsafe extern "C" fn nop(_user: *mut c_void) {}

    #[test]
    fn addmenu_deep_copies_entries() {
        reset();
        let name = CString::new("Vexa").unwrap();
        let sub_name = CString::new("Sub").unwrap();
        let sub = MenuEntryABI {
            name: sub_name.as_ptr(),
            user: 0x7 as *mut c_void,
            selected: Some(yes),
            click: Some(nop),
            length: 0,
            subentries: std::ptr::null(),
        };
        let entry = MenuEntryABI {
            name: name.as_ptr(),
            user: std::ptr::null_mut(),
            selected: None,
            click: Some(nop),
            length: 1,
            subentries: &sub,
        };
        unsafe { mcpelauncher_addmenu(1, &entry) };
        // Mutating the source afterwards must not affect stored state
        // (proves the copy; source lives on this stack frame anyway).
        let reg = registry().lock().unwrap();
        assert_eq!(reg.menus.len(), 1);
        assert_eq!(reg.menus[0].name, "Vexa");
        assert_eq!(reg.menus[0].subentries.len(), 1);
        assert_eq!(reg.menus[0].subentries[0].name, "Sub");
        assert!(reg.menus[0].selected.is_none());
        assert!(reg.menus[0].click.is_some());
        drop(reg);
        reset();
    }

    #[test]
    fn show_window_upserts_by_title_and_close_removes() {
        reset();
        let title = CString::new("Vexa").unwrap();
        let label = CString::new("Enable Trigger").unwrap();
        let ctl = ModControlABI {
            type_: 0,
            data: ModControlDataABI {
                button: ModButtonABI {
                    label: label.as_ptr(),
                    user: 0x3 as *mut c_void,
                    on_click: Some(nop),
                },
            },
        };
        unsafe {
            mcpelauncher_show_window(title.as_ptr(), 0, std::ptr::null_mut(), Some(nop), 1, &ctl);
            // Second call with the same title replaces controls, no new window.
            mcpelauncher_show_window(title.as_ptr(), 1, std::ptr::null_mut(), None, 0, std::ptr::null());
        }
        {
            let reg = registry().lock().unwrap();
            assert_eq!(reg.windows.len(), 1);
            assert!(reg.windows[0].is_modal);
            assert!(reg.windows[0].controls.is_empty());
        }
        unsafe { mcpelauncher_close_window(title.as_ptr()) };
        assert!(registry().lock().unwrap().windows.is_empty());
        reset();
    }

    #[test]
    fn rejects_null_and_unknown_types() {
        reset();
        unsafe {
            mcpelauncher_addmenu(0, std::ptr::null());
            mcpelauncher_addmenu(3, std::ptr::null());
            let title = CString::new("T").unwrap();
            let bad = ModControlABI {
                type_: 99,
                data: ModControlDataABI {
                    text: ModTextABI {
                        label: std::ptr::null(),
                        size: 0,
                    },
                },
            };
            mcpelauncher_show_window(title.as_ptr(), 0, std::ptr::null_mut(), None, 1, &bad);
        }
        let reg = registry().lock().unwrap();
        assert!(reg.menus.is_empty());
        assert_eq!(reg.windows.len(), 1);
        assert!(reg.windows[0].controls.is_empty());
        drop(reg);
        reset();
    }
}
