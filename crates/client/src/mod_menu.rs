//! Rendering for mod-registered menus and windows — port of the
//! `appendMenu` + "Custom Windows" blocks in upstream
//! `mcpelauncher-client/src/imgui_ui.cpp`.
//!
//! State lives in `corelib::mod_menu` (filled by `mcpelauncher_addmenu` /
//! `mcpelauncher_show_window`, typically from a mod's `mod_init`). This module
//! only draws it: menus inside the Mods menu, windows as overlay windows.
//!
//! Callbacks are collected while rendering and invoked *after* the registry
//! lock is released — a click handler typically calls
//! `mcpelauncher_show_window`, which takes the same lock.

use corelib::mod_menu::{
    self, ModButtonClickFn, ModSliderFloatChangeFn, ModSliderIntChangeFn, ModTextInputChangeFn,
    ModWindowCloseFn, StoredControl, StoredMenuEntry,
};
use dear_imgui_rs::{FontId, Ui};
use std::ffi::c_void;

// ---- Menus (rendered inside the Mods menu) ----

type MenuClick = (corelib::mod_menu::ModMenuClickFn, usize);

pub fn render_mod_menus(ui: &Ui) {
    let Some(reg) = mod_menu::try_lock_registry() else {
        return;
    };
    let mut pending: Vec<MenuClick> = Vec::new();
    render_entries(ui, &reg.menus, &mut pending);
    drop(reg);
    for (click, user) in pending {
        unsafe { click(user as *mut c_void) };
    }
}

fn render_entries(ui: &Ui, entries: &[StoredMenuEntry], pending: &mut Vec<MenuClick>) {
    for e in entries {
        if e.subentries.is_empty() {
            let selected = match e.selected {
                Some(sel) => unsafe { sel(e.user as *mut c_void) },
                None => false,
            };
            if ui.menu_item_enabled_selected_no_shortcut(&e.name, selected, true) {
                if let Some(click) = e.click {
                    pending.push((click, e.user));
                }
            }
        } else if !e.name.is_empty() {
            ui.menu(&e.name, || render_entries(ui, &e.subentries, pending));
        } else {
            render_entries(ui, &e.subentries, pending);
        }
    }
}

// ---- Windows ----

enum PendingAction {
    Click {
        cb: ModButtonClickFn,
        user: usize,
    },
    ChangeInt {
        cb: ModSliderIntChangeFn,
        user: usize,
        value: i32,
    },
    ChangeFloat {
        cb: ModSliderFloatChangeFn,
        user: usize,
        value: f32,
    },
    ChangeText {
        cb: ModTextInputChangeFn,
        user: usize,
        value: String,
    },
    Close {
        cb: Option<ModWindowCloseFn>,
        user: usize,
    },
}

pub fn render_mod_windows(ui: &Ui, fonts: [FontId; 4]) {
    let mut actions: Vec<PendingAction> = Vec::new();
    {
        let Some(mut reg) = mod_menu::try_lock_registry() else {
            return;
        };
        for win_idx in 0..reg.windows.len() {
            let w = &mut reg.windows[win_idx];
            let title = w.title.clone();
            let mut open = w.open;
            if w.is_modal {
                if !w.modal_opened {
                    w.modal_opened = true;
                    ui.open_popup(title.clone());
                }
                {
                    let controls = &mut w.controls;
                    ui.modal_popup_with_opened(title, &mut open, || {
                        render_controls(ui, fonts, win_idx, controls, &mut actions);
                    });
                }
            } else {
                let controls = &mut w.controls;
                ui.window(title).opened(&mut open).build(|| {
                    render_controls(ui, fonts, win_idx, controls, &mut actions);
                });
            }
            w.open = open;
        }
        // Upstream drops closed windows and fires onClose (may be absent).
        let mut i = 0;
        while i < reg.windows.len() {
            if !reg.windows[i].open {
                let w = reg.windows.remove(i);
                actions.push(PendingAction::Close {
                    cb: w.on_close,
                    user: w.user,
                });
            } else {
                i += 1;
            }
        }
    }
    for a in actions {
        unsafe {
            match a {
                PendingAction::Click { cb, user } => cb(user as *mut c_void),
                PendingAction::ChangeInt { cb, user, value } => cb(user as *mut c_void, value),
                PendingAction::ChangeFloat { cb, user, value } => cb(user as *mut c_void, value),
                PendingAction::ChangeText { cb, user, value } => {
                    if let Some(s) = mod_menu::mod_cstring(&value) {
                        cb(user as *mut c_void, s.as_ptr());
                    }
                }
                PendingAction::Close { cb, user } => {
                    if let Some(cb) = cb {
                        cb(user as *mut c_void);
                    }
                }
            }
        }
    }
}

/// imgui uses the label as the widget ID; fall back to a unique hidden ID
/// when the mod passed an empty label.
fn widget_id<'a>(
    label: &'a str,
    fallback: &'a str,
) -> &'a str {
    if label.is_empty() {
        fallback
    } else {
        label
    }
}

fn render_controls(
    ui: &Ui,
    fonts: [FontId; 4],
    win_idx: usize,
    controls: &mut [StoredControl],
    actions: &mut Vec<PendingAction>,
) {
    for (i, c) in controls.iter_mut().enumerate() {
        match c {
            StoredControl::Button {
                label,
                user,
                on_click,
            } => {
                let fallback = format!("##modbtn_{win_idx}_{i}");
                if ui.button(widget_id(label, &fallback)) {
                    if let Some(cb) = *on_click {
                        actions.push(PendingAction::Click { cb, user: *user });
                    }
                }
            }
            StoredControl::SliderInt {
                label,
                min,
                value,
                max,
                user,
                on_change,
            } => {
                if !label.is_empty() {
                    ui.text(label.as_str());
                }
                let fallback = format!("##modsliderint_{win_idx}_{i}");
                if ui.slider_i32(widget_id(label, &fallback), value, *min, *max) {
                    if let Some(cb) = *on_change {
                        actions.push(PendingAction::ChangeInt {
                            cb,
                            user: *user,
                            value: *value,
                        });
                    }
                }
            }
            StoredControl::SliderFloat {
                label,
                min,
                value,
                max,
                user,
                on_change,
            } => {
                if !label.is_empty() {
                    ui.text(label.as_str());
                }
                let fallback = format!("##modsliderfloat_{win_idx}_{i}");
                if ui.slider_f32(widget_id(label, &fallback), value, *min, *max) {
                    if let Some(cb) = *on_change {
                        actions.push(PendingAction::ChangeFloat {
                            cb,
                            user: *user,
                            value: *value,
                        });
                    }
                }
            }
            StoredControl::Text { label, size } => {
                if label.is_empty() {
                    continue;
                }
                match *size {
                    1 => {
                        let _f = ui.push_font(fonts[1]);
                        ui.text(label.as_str());
                    }
                    2 => {
                        let _f = ui.push_font(fonts[2]);
                        ui.text(label.as_str());
                    }
                    3 => {
                        let _f = ui.push_font(fonts[3]);
                        ui.text(label.as_str());
                    }
                    _ => ui.text(label.as_str()),
                }
            }
            StoredControl::TextInput {
                label,
                value,
                placeholder,
                user,
                on_change,
            } => {
                // Upstream passes the label as the hint; prefer the mod's
                // placeholder when it provided one.
                let hint = if placeholder.is_empty() {
                    label.clone()
                } else {
                    placeholder.clone()
                };
                let fallback = format!("##modtext_{win_idx}_{i}");
                if ui
                    .input_text(widget_id(label, &fallback), value)
                    .hint(hint)
                    .build()
                {
                    if let Some(cb) = *on_change {
                        actions.push(PendingAction::ChangeText {
                            cb,
                            user: *user,
                            value: value.clone(),
                        });
                    }
                }
            }
        }
    }
}
