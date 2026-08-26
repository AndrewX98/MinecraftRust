//! Rust port of the C++ `settings.cpp`/`settings.h` + the properties parser
//! (`properties-parser/include/properties/{property_list.h,property.h}`).
//! Settings persist to `<primary data dir>/mcpelauncher-client-settings.txt`
//! (same file, same key=value format as mcpelauncher-manifest). `load()` is
//! called once at startup from `main.rs`; the ImGui menus and
//! `window_callbacks` fullscreen toggle call `mc_settings_save()`.
//!
//! These `mc_settings_*` accessors are also the FFI surface consumed by
//! `crate::window_callbacks`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

static MENUBARSIZE: AtomicI32 = AtomicI32::new(0);
static ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES: AtomicBool = AtomicBool::new(false);
static ENABLE_KEYBOARD_AUTOFOCUS_PATCHES: AtomicBool = AtomicBool::new(false);
static ENABLE_INTEL_SPRINT_STRAFE: AtomicBool = AtomicBool::new(false);
static FULLSCREEN: AtomicBool = AtomicBool::new(false);
static ENABLE_IMGUI: AtomicBool = AtomicBool::new(true);
static ENABLE_MENUBAR: AtomicBool = AtomicBool::new(true);
static VSYNC: AtomicBool = AtomicBool::new(true);
static ALT_FOCUS_MENUBAR: AtomicBool = AtomicBool::new(true);
// None = "auto" (C++ `std::optional<bool> Settings::enable_imgui`).
static ENABLE_IMGUI_AUTO: AtomicBool = AtomicBool::new(true);
// UI scale in quarter steps *4 (4 = 100% .. 20 = 500%), C++ `Settings::scale`.
static SCALE_NUM: AtomicI32 = AtomicI32::new(4);
// HUD visibility modes (manifest Settings::enable_fps_hud semantics):
// 0 = never, 1 = always, 2 = ingame only.
static FPS_HUD_MODE: AtomicI32 = AtomicI32::new(1);
static KEYSTROKE_HUD_MODE: AtomicI32 = AtomicI32::new(1);
// HUD anchor positions as fractions of the viewport * 10000.
static FPS_HUD_X10000: AtomicU32 = AtomicU32::new(150);
static FPS_HUD_Y10000: AtomicU32 = AtomicU32::new(600);
static KEYSTROKE_HUD_X10000: AtomicU32 = AtomicU32::new(500);
static KEYSTROKE_HUD_Y10000: AtomicU32 = AtomicU32::new(3000);
// Keys we don't know — preserved verbatim across load/save so files written by
// other launcher versions keep their extra entries (property_list::unknown_props).
// Vec keeps insertion order and has a const constructor.
static UNKNOWN_PROPS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn mc_settings_get_menubarsize() -> i32 {
    MENUBARSIZE.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_get_enable_keyboard_autofocus_paste_patches_1_20_60() -> bool {
    ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_enable_keyboard_autofocus_paste_patches_1_20_60(v: bool) {
    ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_enable_keyboard_autofocus_patches_1_20_60() -> bool {
    ENABLE_KEYBOARD_AUTOFOCUS_PATCHES.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_enable_keyboard_autofocus_patches_1_20_60(v: bool) {
    ENABLE_KEYBOARD_AUTOFOCUS_PATCHES.store(v, Ordering::SeqCst);
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn mc_settings_get_enable_intel_sprint_strafe() -> bool {
    ENABLE_INTEL_SPRINT_STRAFE.load(Ordering::SeqCst)
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn mc_settings_set_enable_intel_sprint_strafe(v: bool) {
    ENABLE_INTEL_SPRINT_STRAFE.store(v, Ordering::SeqCst);
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
pub extern "C" fn mc_settings_get_enable_imgui() -> bool {
    ENABLE_IMGUI.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_enable_imgui(v: bool) {
    ENABLE_IMGUI.store(v, Ordering::SeqCst);
    ENABLE_IMGUI_AUTO.store(false, Ordering::SeqCst);
}

// UI scale (C++ `Settings::scale`, quarter steps).
#[no_mangle]
pub extern "C" fn mc_settings_get_scale() -> f32 {
    SCALE_NUM.load(Ordering::SeqCst) as f32 / 4.0
}

#[no_mangle]
pub extern "C" fn mc_settings_set_scale(v: f32) {
    SCALE_NUM.store((v * 4.0).round() as i32, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_set_menubarsize(v: i32) {
    MENUBARSIZE.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_enable_menubar() -> bool {
    ENABLE_MENUBAR.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_enable_menubar(v: bool) {
    ENABLE_MENUBAR.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_vsync() -> bool {
    VSYNC.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_vsync(v: bool) {
    VSYNC.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_alt_focus_menubar() -> bool {
    ALT_FOCUS_MENUBAR.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_alt_focus_menubar(v: bool) {
    ALT_FOCUS_MENUBAR.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_fps_hud_mode() -> i32 {
    FPS_HUD_MODE.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_fps_hud_mode(v: i32) {
    FPS_HUD_MODE.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_keystroke_hud_mode() -> i32 {
    KEYSTROKE_HUD_MODE.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_keystroke_hud_mode(v: i32) {
    KEYSTROKE_HUD_MODE.store(v, Ordering::SeqCst);
}

// HUD anchor fractions * 10000 (u32 to stay lock-free).
#[no_mangle]
pub extern "C" fn mc_settings_get_fps_hud_x() -> u32 {
    FPS_HUD_X10000.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_fps_hud_x(v: u32) {
    FPS_HUD_X10000.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_fps_hud_y() -> u32 {
    FPS_HUD_Y10000.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_fps_hud_y(v: u32) {
    FPS_HUD_Y10000.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_keystroke_hud_x() -> u32 {
    KEYSTROKE_HUD_X10000.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_keystroke_hud_x(v: u32) {
    KEYSTROKE_HUD_X10000.store(v, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn mc_settings_get_keystroke_hud_y() -> u32 {
    KEYSTROKE_HUD_Y10000.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn mc_settings_set_keystroke_hud_y(v: u32) {
    KEYSTROKE_HUD_Y10000.store(v, Ordering::SeqCst);
}

fn settings_path() -> String {
    format!(
        "{}mcpelauncher-client-settings.txt",
        crate::path_helper::get_primary_data_directory()
    )
}

/// `property<bool>::parse_value` — accepts 1/true/on/yes and 0/false/off/no.
fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Some(true),
        "0" | "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

fn parse_f32(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok()
}

/// Port of `Settings::load()` (settings.cpp:66): parses
/// `<data dir>/mcpelauncher-client-settings.txt` into the statics.
pub fn load() {
    let path = settings_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        log::info!("settings: no settings file at {} (using defaults)", path);
        return;
    };
    let mut unknown = UNKNOWN_PROPS.lock().unwrap();
    unknown.clear();
    for line in content.lines() {        // Files coming from windows use CRLF; lines() keeps the '\r'.
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            // C++ stores enable_imgui as string: "auto" or a bool.
            "enable_imgui" => {
                if value.trim() == "auto" {
                    ENABLE_IMGUI_AUTO.store(true, Ordering::SeqCst);
                } else if let Some(b) = parse_bool(value) {
                    ENABLE_IMGUI_AUTO.store(false, Ordering::SeqCst);
                    ENABLE_IMGUI.store(b, Ordering::SeqCst);
                }
            }
            "enable_keyboard_autofocus_patches_1_20_60" => {
                if let Some(b) = parse_bool(value) {
                    ENABLE_KEYBOARD_AUTOFOCUS_PATCHES.store(b, Ordering::SeqCst);
                }
            }
            "enable_keyboard_autofocus_paste_patches_1_20_60" => {
                if let Some(b) = parse_bool(value) {
                    ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES.store(b, Ordering::SeqCst);
                }
            }
            "enable_intel_sprint_strafe_patch" => {
                if let Some(b) = parse_bool(value) {
                    ENABLE_INTEL_SPRINT_STRAFE.store(b, Ordering::SeqCst);
                }
            }
            "enable_menubar" => {
                if let Some(b) = parse_bool(value) {
                    ENABLE_MENUBAR.store(b, Ordering::SeqCst);
                }
            }
            "fullscreen" => {
                if let Some(b) = parse_bool(value) {
                    FULLSCREEN.store(b, Ordering::SeqCst);
                }
            }
            "vsync" => {
                if let Some(b) = parse_bool(value) {
                    VSYNC.store(b, Ordering::SeqCst);
                }
            }
            // C++ stores the focus key as a string ("alt"/""); we only have alt.
            "menubarFocusKey" => ALT_FOCUS_MENUBAR.store(value.trim() == "alt", Ordering::SeqCst),
            "enable_fps_hud" => {
                if let Ok(v) = value.trim().parse::<i32>() {
                    FPS_HUD_MODE.store(v, Ordering::SeqCst);
                }
            }
            "enable_keystroke_hud" => {
                if let Ok(v) = value.trim().parse::<i32>() {
                    KEYSTROKE_HUD_MODE.store(v, Ordering::SeqCst);
                }
            }
            // C++ persists absolute pixels here; this port persists viewport
            // fractions *10000. Values are only meaningful to whichever
            // launcher wrote them (round-trips consistently).
            "fps_hud_x" => {
                if let Some(v) = parse_f32(value) {
                    FPS_HUD_X10000.store((v * 10_000.0).max(0.0) as u32, Ordering::SeqCst);
                }
            }
            "fps_hud_y" => {
                if let Some(v) = parse_f32(value) {
                    FPS_HUD_Y10000.store((v * 10_000.0).max(0.0) as u32, Ordering::SeqCst);
                }
            }
            "keystroke_hud_x" => {
                if let Some(v) = parse_f32(value) {
                    KEYSTROKE_HUD_X10000.store((v * 10_000.0).max(0.0) as u32, Ordering::SeqCst);
                }
            }
            "keystroke_hud_y" => {
                if let Some(v) = parse_f32(value) {
                    KEYSTROKE_HUD_Y10000.store((v * 10_000.0).max(0.0) as u32, Ordering::SeqCst);
                }
            }
            "scale" => {
                if let Some(v) = parse_f32(value) {
                    SCALE_NUM.store((v * 4.0).round() as i32, Ordering::SeqCst);
                }
            }
            _ => {
                unknown.retain(|(k, _)| k != key);
                unknown.push((key.to_string(), value.to_string()));
            }
        }
    }
    drop(unknown);
    log::info!("settings: loaded {}", path);
}

fn push_prop(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    out.push_str(value);
    out.push('\n');
}

fn bool_str(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

/// Port of `Settings::save()` (settings.cpp:97): writes all settings plus any
/// unknown keys back to `<data dir>/mcpelauncher-client-settings.txt`.
#[no_mangle]
pub extern "C" fn mc_settings_save() {
    let mut out = String::new();
    push_prop(
        &mut out,
        "enable_imgui",
        &if ENABLE_IMGUI_AUTO.load(Ordering::SeqCst) {
            "auto".to_string()
        } else {
            bool_str(ENABLE_IMGUI.load(Ordering::SeqCst)).to_string()
        },
    );
    push_prop(
        &mut out,
        "enable_keyboard_autofocus_patches_1_20_60",
        bool_str(ENABLE_KEYBOARD_AUTOFOCUS_PATCHES.load(Ordering::SeqCst)),
    );
    push_prop(
        &mut out,
        "enable_keyboard_autofocus_paste_patches_1_20_60",
        bool_str(ENABLE_KEYBOARD_AUTOFOCUS_PASTE_PATCHES.load(Ordering::SeqCst)),
    );
    #[cfg(target_arch = "x86_64")]
    push_prop(
        &mut out,
        "enable_intel_sprint_strafe_patch",
        bool_str(ENABLE_INTEL_SPRINT_STRAFE.load(Ordering::SeqCst)),
    );
    #[cfg(not(target_arch = "x86_64"))]
    push_prop(&mut out, "enable_intel_sprint_strafe_patch", "false");
    push_prop(
        &mut out,
        "enable_menubar",
        bool_str(ENABLE_MENUBAR.load(Ordering::SeqCst)),
    );
    push_prop(
        &mut out,
        "enable_fps_hud",
        &FPS_HUD_MODE.load(Ordering::SeqCst).to_string(),
    );
    push_prop(
        &mut out,
        "fps_hud_x",
        &(FPS_HUD_X10000.load(Ordering::SeqCst) as f32 / 10_000.0).to_string(),
    );
    push_prop(
        &mut out,
        "fps_hud_y",
        &(FPS_HUD_Y10000.load(Ordering::SeqCst) as f32 / 10_000.0).to_string(),
    );
    push_prop(
        &mut out,
        "enable_keystroke_hud",
        &KEYSTROKE_HUD_MODE.load(Ordering::SeqCst).to_string(),
    );
    push_prop(
        &mut out,
        "keystroke_hud_x",
        &(KEYSTROKE_HUD_X10000.load(Ordering::SeqCst) as f32 / 10_000.0).to_string(),
    );
    push_prop(
        &mut out,
        "keystroke_hud_y",
        &(KEYSTROKE_HUD_Y10000.load(Ordering::SeqCst) as f32 / 10_000.0).to_string(),
    );
    push_prop(
        &mut out,
        "scale",
        &(SCALE_NUM.load(Ordering::SeqCst) as f32 / 4.0).to_string(),
    );
    push_prop(
        &mut out,
        "menubarFocusKey",
        if ALT_FOCUS_MENUBAR.load(Ordering::SeqCst) { "alt" } else { "" },
    );
    push_prop(
        &mut out,
        "fullscreen",
        bool_str(FULLSCREEN.load(Ordering::SeqCst)),
    );
    push_prop(&mut out, "vsync", bool_str(VSYNC.load(Ordering::SeqCst)));

    // Unknown keys last, like property_list::save().
    let unknown = UNKNOWN_PROPS.lock().unwrap();
    for (k, v) in unknown.iter() {
        push_prop(&mut out, k, v);
    }
    drop(unknown);
    let path = settings_path();
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, out) {
        Ok(()) => log::debug!("settings: saved {}", path),
        Err(e) => log::warn!("settings: failed to save {}: {}", path, e),
    }
}
