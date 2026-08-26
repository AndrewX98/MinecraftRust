//! ImGui overlay — Rust port of mcpelauncher-manifest's `imgui_ui.cpp`
//! (minimal pass): init on first EGL make-current, per-frame draw inside the
//! FakeEGL swap path, input fed from eglut trampolines via shared state, and
//! a GLES2 renderer (`self::gles2`, port of `imgui_impl_opengl3` ES2).
//!
//! Threading model matches upstream: the imgui context is process-global but
//! only touched from GL threads (game render thread), serialized by the
//! `UI_STATE` mutex.

pub mod gles2;

use crate::window_callbacks::keycode;
use dear_imgui_rs::{
    Condition, ConfigFlags, Context, FontId, FontSource, Key, MouseButton, TextureId, Ui,
    WindowFlags,
};
use std::ffi::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

// ============================================================
// Shared input state (mirrors manifest's SharedInputState)
// ============================================================

#[derive(Default)]
struct RawInput {
    mouse_pos: Option<[f64; 2]>,
    buttons: [Option<bool>; 3], // left / right / middle edges
    wheel_dx: f64,
    wheel_dy: f64,
    keys: Vec<(i32, bool)>,
    text: String,
}

impl RawInput {
    const EMPTY: RawInput = RawInput {
        mouse_pos: None,
        buttons: [None, None, None],
        wheel_dx: 0.0,
        wheel_dy: 0.0,
        keys: Vec::new(),
        text: String::new(),
    };
}

static INPUT: Mutex<RawInput> = Mutex::new(RawInput::EMPTY);
static WANT_CAPTURE_MOUSE: AtomicBool = AtomicBool::new(false);
static WANT_CAPTURE_KEYBOARD: AtomicBool = AtomicBool::new(false);
static MOUSE_SEEN: AtomicBool = AtomicBool::new(false);
static LAST_MOUSE_POS: Mutex<[f64; 2]> = Mutex::new([0.0, 0.0]);
/// Time the pointer has been hovering at y==0 (menubar reveal, like
/// manifest's `mouseOnY0Since`).
static Y0_SINCE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
/// UI scale numerator in quarter steps (4 = 100% .. 20 = 500%), manifest's
/// `Settings::scale` — persisted via settings.rs (`mc_settings_get_scale`).
static RELOAD_FONT: AtomicBool = AtomicBool::new(false);
static MOVING_HUDS: AtomicBool = AtomicBool::new(false);
/// Virtual-key -> down state (index = VK code; only codes < 256 tracked).
static KEYS_DOWN: [AtomicBool; 256] = {
    #[allow(clippy::declare_interior_mutable_const)]
    const FALSE: AtomicBool = AtomicBool::new(false);
    [FALSE; 256]
};
static LMB_CLICKS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static RMB_CLICKS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
/// left / right / middle current down state.
static MOUSE_DOWN: [AtomicBool; 3] = {
    #[allow(clippy::declare_interior_mutable_const)]
    const FALSE: AtomicBool = AtomicBool::new(false);
    [FALSE; 3]
};

fn mouse_down(idx: usize) -> bool {
    MOUSE_DOWN[idx].load(Ordering::Relaxed)
}

pub(crate) fn key_down(vk: i32) -> bool {
    vk >= 0 && vk < 256 && KEYS_DOWN[vk as usize].load(Ordering::Relaxed)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn recent_clicks(v: &mut Vec<u64>) -> usize {
    let cutoff = now_ms().saturating_sub(1000);
    v.retain(|t| *t > cutoff);
    v.len()
}

fn ui_scale() -> f32 {
    crate::settings::mc_settings_get_scale()
}

pub fn record_mouse_pos(x: f64, y: f64) {
    let mut inp = INPUT.lock().unwrap();
    inp.mouse_pos = Some([x, y]);
    *LAST_MOUSE_POS.lock().unwrap() = [x, y];
    MOUSE_SEEN.store(true, Ordering::Relaxed);
    let mut since = Y0_SINCE.lock().unwrap();
    if y != 0.0 {
        *since = None;
    } else if since.is_none() {
        *since = Some(std::time::Instant::now());
    }
}

/// X11 button ids as reported by eglut: 1 = left, 2 = middle, 3 = right.
pub fn record_mouse_button(xbtn: i32, down: bool) {
    let idx = match xbtn {
        1 => 0,
        3 => 1,
        2 => 2,
        _ => return,
    };
    if down && xbtn == 1 {
        LMB_CLICKS.lock().unwrap().push(now_ms());
    } else if down && xbtn == 3 {
        RMB_CLICKS.lock().unwrap().push(now_ms());
    }
    MOUSE_DOWN[idx].store(down, Ordering::Relaxed);
    INPUT.lock().unwrap().buttons[idx] = Some(down);
}

pub fn record_mouse_wheel(dx: f64, dy: f64) {
    let mut inp = INPUT.lock().unwrap();
    inp.wheel_dx += dx;
    inp.wheel_dy += dy;
}

pub fn record_key(vk: i32, down: bool) {
    INPUT.lock().unwrap().keys.push((vk, down));
    if vk >= 0 && vk < 256 {
        let slot = &KEYS_DOWN[vk as usize];
        if slot.load(Ordering::Relaxed) != down {
            slot.store(down, Ordering::Relaxed);
            log::trace!("imgui_ui: key {} -> {}", vk, if down { "down" } else { "up" });
        }
    }
}

pub fn record_text(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    INPUT.lock().unwrap()
        .text.push_str(&String::from_utf8_lossy(bytes));
}

pub fn want_capture_mouse() -> bool {
    WANT_CAPTURE_MOUSE.load(Ordering::Relaxed)
}

pub fn want_capture_keyboard() -> bool {
    WANT_CAPTURE_KEYBOARD.load(Ordering::Relaxed)
}

fn enabled() -> bool {
    crate::settings::mc_settings_get_enable_imgui()
}

// ============================================================
// UI state
// ============================================================

struct UiState {
    ctx: Context,
    renderer: gles2::Gles2Renderer,
    last_frame: Instant,
    show_about: bool,
    file_picker_open: bool,
    file_picker_path: String,
    /// Session-only menubar visibility (manifest's static `showMenuBar`);
    /// cleared by "Hide Menubar", not persisted. Alt-focus (with
    /// "Use Alt to Focus Menubar" on) reopens it.
    menubar_visible_session: bool,
    #[cfg(debug_assertions)]
    prev_alt_dbg: bool,
    /// Menubar window focused/hovered (manifest's `menuFocused`).
    menu_focused: bool,
    /// Previous frame's Alt-focus request (manifest's `lastwantfocusnextframe`).
    prev_want_focus: bool,
    fps_smoothed: f32,
    font_texture: TextureId,
    fonts: [FontId; 4],     // default / medium / large / very large
    font_sizes: [f32; 4],   // matching pixel sizes
}

struct SendUi(UiState);
// SAFETY: the imgui Context holds raw pointers to C++ state; it is only ever
// dereferenced while holding UI_STATE's mutex from a thread with an EGL
// context current (the game render thread). The Send impl exists solely to
// allow storing it in the global mutex.
unsafe impl Send for SendUi {}

static UI_STATE: Mutex<Option<SendUi>> = Mutex::new(None);
static INIT_ATTEMPTED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn eglutGetWindowSize(w: *mut i32, h: *mut i32);
    fn eglutGetMousePointerLocked() -> i32;
    fn eglutToggleFullscreen();
    fn glGetString(name: u32) -> *const c_char;
}

const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;

unsafe fn gl_string(name: u32) -> &'static str {
    let p = glGetString(name);
    if p.is_null() {
        return "?";
    }
    std::ffi::CStr::from_ptr(p).to_str().unwrap_or("?")
}

/// Resolves the Minecraft UI font like manifest's `ReloadFont()`: one of
/// these ships with every version newer than 1.0.
fn find_font_file() -> Option<Vec<u8>> {
    let game_dir = crate::path_helper::get_game_dir();
    for path in [
        format!("{game_dir}/assets/assets/fonts/Mojangles.ttf"),
        format!("{game_dir}/assets/fonts/Mojangles.ttf"),
        format!("{game_dir}/assets/fonts/SegoeWP.ttf"),
    ] {
        if crate::path_helper::file_exists(&path) {
            match std::fs::read(&path) {
                Ok(data) => return Some(data),
                Err(e) => log::warn!("ImGui: failed to read font {path}: {e}"),
            }
        }
    }
    None
}

/// Builds the font atlas at the four scaled sizes (port of `ReloadFont`).
unsafe fn reload_fonts(state: &mut UiState) {
    let scale = ui_scale();
    let size = |px: i32| (px as f32 * scale).ceil();
    let ttf = find_font_file();
    // SAFETY: ttf_data requires a complete TTF in the slice; fs::read gives us that.
    let sources_for = |px: i32| unsafe {
        if let Some(data) = &ttf {
            FontSource::ttf_data_with_size(data, size(px))
        } else {
            FontSource::default_font()
        }
    };
    // Re-acquire the legacy lease (mode was claimed at init).
    let Ok(legacy) = state.ctx.font_atlas().try_claim_legacy_renderer() else {
        log::warn!("ImGui: font atlas not in legacy renderer mode");
        return;
    };
    legacy.clear_fonts();
    let f_default = legacy.add_font(&[sources_for(15)]);
    let f_medium = legacy.add_font(&[sources_for(18)]);
    let f_large = legacy.add_font(&[sources_for(24)]);
    let f_very_large = legacy.add_font(&[sources_for(36)]);
    state.fonts = [f_default, f_medium, f_large, f_very_large];
    state.font_sizes = [size(15), size(18), size(24), size(36)];
    legacy.build();
    // Upload while still holding the lease: tex_data()/set_texture_id()
    // live on the LegacyFontAtlas capability.
    if let Some(tex) = legacy.tex_data() {
        let new_tex = state.renderer.upload_font_texture(
            tex.width() as i32,
            tex.height() as i32,
            tex.pixels().unwrap_or(&[]),
        );
        if state.font_texture.id() != 0 {
            state.renderer.delete_texture(state.font_texture);
        }
        state.font_texture = new_tex;
        // SAFETY: plain GLuint texture id handed to imgui's legacy path.
        unsafe { legacy.set_texture_id(new_tex) };
    }
}

/// Creates the imgui context + renderer; called from `fake_egl_make_current`
/// once the game context is current for the first time.
pub fn init_once() {
    if INIT_ATTEMPTED.swap(true, Ordering::SeqCst) {
        return;
    }
    // SAFETY: an EGL/GLES2 context is current on this thread here.
    let Some(mut renderer) = (unsafe { gles2::Gles2Renderer::new() }) else {
        log::warn!("ImGui overlay disabled: GLES2 renderer init failed");
        return;
    };
    let mut ctx = Context::create();
    let _ = ctx.set_ini_filename(None::<std::path::PathBuf>);
    {
        let io = ctx.io_mut();
        let flags = io.config_flags();
        io.set_config_flags(flags | ConfigFlags::NAV_ENABLE_KEYBOARD);
    }
    // Claim legacy-renderer mode (static font texture, like the C++ backend)
    // and build the initial atlas so the first frame has a valid texture.
    let legacy = match ctx.font_atlas().try_claim_legacy_renderer() {
        Ok(l) => l,
        Err(_) => {
            log::warn!("ImGui overlay disabled: legacy font atlas unavailable");
            return;
        }
    };
    legacy.build();
    drop(legacy);
    let font0 = ctx
        .font_atlas()
        .add_font(&[FontSource::default_font()]);
    let tex_id = TextureId::new(0);
    *UI_STATE.lock().unwrap() = Some(SendUi(UiState {
        ctx,
        renderer,
        last_frame: Instant::now(),
        show_about: false,
        file_picker_open: false,
        file_picker_path: String::new(),
        menubar_visible_session: true,
        #[cfg(debug_assertions)]
        prev_alt_dbg: false,
        menu_focused: false,
        prev_want_focus: false,
        fps_smoothed: 60.0,
        font_texture: tex_id,
        fonts: [font0; 4],
        font_sizes: [15.0, 18.0, 24.0, 36.0],
    }));
    // Swap in the Minecraft font (Mojangles/SegoeWP) at the scaled sizes,
    // like manifest's ReloadFont() called from ImGuiUIInit.
    if let Some(SendUi(state)) = UI_STATE.lock().unwrap().as_mut() {
        // SAFETY: context is current on this thread (make-current path).
        unsafe {
            reload_fonts(state);
        }
        log::info!(
            "ImGui overlay ready: Dear ImGui {}, {} font, atlas tex {:?}",
            unsafe { std::ffi::CStr::from_ptr(dear_imgui_sys::igGetVersion()) }.to_string_lossy(),
            if find_font_file().is_some() { "Mojangles" } else { "default" },
            state.font_texture,
        );
    }
}

fn map_key(code: i32) -> &'static [Key] {
    use Key::*;
    const A_Z: [Key; 26] =
        [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
    const DIGITS: [Key; 10] =
        [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
    const KEYPAD: [Key; 10] =
        [Keypad0, Keypad1, Keypad2, Keypad3, Keypad4, Keypad5, Keypad6, Keypad7, Keypad8, Keypad9];
    const FKEYS: [Key; 12] =
        [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    match code {
        keycode::BACKSPACE => &[Backspace],
        keycode::TAB => &[Tab],
        keycode::ENTER => &[Enter],
        keycode::LEFT_SHIFT => &[LeftShift, ModShift],
        keycode::RIGHT_SHIFT => &[RightShift, ModShift],
        keycode::LEFT_CTRL => &[LeftCtrl, ModCtrl],
        keycode::RIGHT_CTRL => &[RightCtrl, ModCtrl],
        keycode::PAUSE => &[Pause],
        keycode::CAPS_LOCK => &[CapsLock],
        keycode::ESCAPE => &[Escape],
        keycode::PAGE_UP => &[PageUp],
        keycode::PAGE_DOWN => &[PageDown],
        keycode::END => &[End],
        keycode::HOME => &[Home],
        keycode::LEFT => &[LeftArrow],
        keycode::UP => &[UpArrow],
        keycode::RIGHT => &[RightArrow],
        keycode::DOWN => &[DownArrow],
        keycode::INSERT => &[Insert],
        keycode::DELETE => &[Delete],
        keycode::NUM_LOCK => &[NumLock],
        keycode::SCROLL_LOCK => &[ScrollLock],
        c if (keycode::NUM_0..=keycode::NUM_9).contains(&c) => &DIGITS[(c - keycode::NUM_0) as usize..(c - keycode::NUM_0 + 1) as usize],
        c if (0x60..=0x69).contains(&c) => &KEYPAD[(c - 0x60) as usize..(c - 0x60 + 1) as usize],
        c if (keycode::A..=keycode::Z).contains(&c) => &A_Z[(c - keycode::A) as usize..(c - keycode::A + 1) as usize],
        c if (keycode::FN1..=keycode::FN12).contains(&c) => &FKEYS[(c - keycode::FN1) as usize..(c - keycode::FN1 + 1) as usize],
        keycode::SEMICOLON => &[Semicolon],
        keycode::EQUAL => &[Equal],
        keycode::COMMA => &[Comma],
        keycode::MINUS => &[Minus],
        keycode::PERIOD => &[Period],
        keycode::SLASH => &[Slash],
        keycode::GRAVE => &[GraveAccent],
        keycode::LEFT_BRACKET => &[LeftBracket],
        keycode::BACKSLASH => &[Backslash],
        keycode::RIGHT_BRACKET => &[RightBracket],
        keycode::APOSTROPHE => &[Apostrophe],
        keycode::MENU => &[Menu],
        keycode::LEFT_ALT => &[LeftAlt, ModAlt],
        keycode::RIGHT_ALT => &[RightAlt, ModAlt],
        _ => &[],
    }
}

/// Builds and renders one imgui frame over the just-finished game frame.
/// Called from `fake_egl_swap_buffers` before the actual swap.
pub fn draw_frame() {
    if !enabled() {
        WANT_CAPTURE_MOUSE.store(false, Ordering::Relaxed);
        WANT_CAPTURE_KEYBOARD.store(false, Ordering::Relaxed);
        return;
    }
    let Ok(mut guard) = UI_STATE.try_lock() else { return };
    let Some(SendUi(state)) = guard.as_mut() else { return };

    let dt = state.last_frame.elapsed().as_secs_f32().clamp(1.0 / 1000.0, 0.25);
    state.last_frame = std::time::Instant::now();
    state.fps_smoothed += (1.0 / dt - state.fps_smoothed) * 0.05;
    let mut ui_want_keyboard_next = false;

    if RELOAD_FONT.swap(false, Ordering::Relaxed) {
        // SAFETY: context is current on this thread (swap path).
        unsafe {
            reload_fonts(state);
        }
    }

    let mut w = 0;
    let mut h = 0;
    unsafe { eglutGetWindowSize(&mut w, &mut h) };

    // Feed recorded events (manifest ImGuiUIDrawFrame equivalent).
    {
        let io = state.ctx.io_mut();
        io.set_display_size([w.max(0) as f32, h.max(0) as f32]);
        io.set_delta_time(dt);

        let mut inp = INPUT.lock().unwrap();
        if MOUSE_SEEN.load(Ordering::Relaxed) {
            let pos = inp
                .mouse_pos
                .take()
                .unwrap_or(*LAST_MOUSE_POS.lock().unwrap());
            io.add_mouse_pos_event([pos[0] as f32, pos[1] as f32]);
        }
        for (i, edge) in inp.buttons.iter_mut().enumerate() {
            if let Some(down) = edge.take() {
                let btn = match i {
                    0 => MouseButton::Left,
                    1 => MouseButton::Right,
                    _ => MouseButton::Middle,
                };
                io.add_mouse_button_event(btn, down);
            }
        }
        if inp.wheel_dx != 0.0 || inp.wheel_dy != 0.0 {
            io.add_mouse_wheel_event([inp.wheel_dx as f32, inp.wheel_dy as f32]);
            inp.wheel_dx = 0.0;
            inp.wheel_dy = 0.0;
        }
        for (vk, down) in inp.keys.drain(..) {
            for key in map_key(vk) {
                io.add_key_event(*key, down);
            }
        }
        if !inp.text.is_empty() {
            for ch in inp.text.chars() {
                io.add_input_character(ch);
            }
            inp.text.clear();
        }
    }

    // Menubar visibility, mirroring manifest's autoShowMenubar: enabled by
    // settings; always when the pointer is free; after hovering the top edge
    // for 500 ms while pointer-locked; while the menubar itself is focused or
    // hovered; and Alt focuses it when "Use Alt to Focus Menubar" is on
    // (imgui_ui.cpp:618-655).
    let locked = unsafe { eglutGetMousePointerLocked() != 0 };
    let top_hover = Y0_SINCE
        .lock()
        .unwrap()
        .map(|t| t.elapsed() >= std::time::Duration::from_millis(500))
        .unwrap_or(false);
    let alt_down = key_down(keycode::LEFT_ALT) || key_down(keycode::RIGHT_ALT);
    #[cfg(debug_assertions)]
    if alt_down != state.prev_alt_dbg {
        log::warn!("DBG alt_down={} setting={}", alt_down, crate::settings::mc_settings_get_alt_focus_menubar());
        state.prev_alt_dbg = alt_down;
    }
    // Manifest: IsKeyPressed(ImGuiKey_ModAlt) is true only on the press frame.
    let want_focus_next = crate::settings::mc_settings_get_alt_focus_menubar()
        && alt_down
        && !state.prev_want_focus;
    state.prev_want_focus = want_focus_next;
    if want_focus_next {
        ui_want_keyboard_next = true;
        // Reopen a menubar hidden via "Hide Menubar" and keep it up after
        // Alt is released (focus alone drops once ImGui moves focus away).
        state.menubar_visible_session = true;
    }
    let show_menubar = crate::settings::mc_settings_get_enable_menubar()
        && (state.menubar_visible_session || want_focus_next)
        && (!locked || top_hover || state.menu_focused || want_focus_next);
    #[cfg(debug_assertions)]
    {
        static ALT_TRACE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
        let t = ALT_TRACE.load(Ordering::Relaxed);
        if t > 0 {
            ALT_TRACE.store(t - 1, Ordering::Relaxed);
            log::warn!(
                "DBG trace locked={} session={} menu_focused={} want_focus={} show={}",
                locked, state.menubar_visible_session, state.menu_focused, want_focus_next, show_menubar
            );
        }
    }

    let ui = state.ctx.frame();
    if ui_want_keyboard_next {
        ui.set_next_frame_want_capture_keyboard(true);
    }

    if show_menubar {
        let mut menubar_h = 0.0f32;
        ui.main_menu_bar(|| {
            // Manifest imgui_ui.cpp:624-640: track focus/hover to keep the
            // bar open, and on the Alt-focus frame focus the "File" menu with
            // the nav cursor visible.
            state.menu_focused = ui.is_window_focused_with_flags(
                dear_imgui_rs::FocusedFlags::ROOT_AND_CHILD_WINDOWS,
            ) || ui.is_window_hovered_with_flags(
                dear_imgui_rs::WindowHoveredFlags::ROOT_AND_CHILD_WINDOWS,
            );
            if want_focus_next {
                // SAFETY: inside BeginMainMenuBar; igGetCurrentWindow is the
                // menubar window, matching manifest's SetFocusID use.
                unsafe {
                    let id = ui.get_id("File").raw();
                    dear_imgui_sys::igSetFocusID(id, dear_imgui_sys::igGetCurrentWindow());
                }
                ui.set_nav_cursor_visible(true);
                state.menu_focused = true;
            }
            // ---- File ------------------------------------------------
            ui.menu("File", || {
                // Upstream gates "Open" behind #ifndef NDEBUG.
                #[cfg(debug_assertions)]
                if ui.menu_item("Open") {
                    state.file_picker_open = true;
                }
                if ui.menu_item("Hide Menubar") {
                    // Hide for this session only (no confirm popup).
                    state.menubar_visible_session = false;
                }
                let alt_focus = crate::settings::mc_settings_get_alt_focus_menubar();
                if ui.menu_item_enabled_selected_no_shortcut(
                    "Use Alt to Focus Menubar",
                    alt_focus,
                    true,
                ) {
                    crate::settings::mc_settings_set_alt_focus_menubar(!alt_focus);
                    crate::settings::mc_settings_save();
                }
                if ui.menu_item("Close") {
                    unsafe { libc::_exit(0) };
                }
            });
            // ---- Mods ------------------------------------------------
            ui.menu("Mods", || {
                let autofocus =
                    crate::settings::mc_settings_get_enable_keyboard_autofocus_patches_1_20_60();
                if ui.menu_item_enabled_selected_no_shortcut(
                    "Enable Keyboard AutoFocus Patches for 1.20.60+",
                    autofocus,
                    true,
                ) {
                    crate::settings::mc_settings_set_enable_keyboard_autofocus_patches_1_20_60(!autofocus);
                    crate::settings::mc_settings_save();
                }
                let paste =
                    crate::settings::mc_settings_get_enable_keyboard_autofocus_paste_patches_1_20_60();
                // Like upstream, the paste toggle is only enabled while the
                // base autofocus patches are on.
                if ui.menu_item_enabled_selected_no_shortcut(
                    "Enable Keyboard AutoFocus Paste Patches for 1.20.60+",
                    paste,
                    autofocus,
                ) {
                    crate::settings::mc_settings_set_enable_keyboard_autofocus_paste_patches_1_20_60(!paste);
                    crate::settings::mc_settings_save();
                }
                // Upstream gates this behind __x86_64__; the patch itself is a
                // no-op until ported (flag persisted like the others).
                #[cfg(target_arch = "x86_64")]
                {
                    let intel = crate::settings::mc_settings_get_enable_intel_sprint_strafe();
                    if ui.menu_item_enabled_selected_no_shortcut(
                        "Enable Sprint strafe patch for Intel CPUs (requires restart)",
                        intel,
                        true,
                    ) {
                        crate::settings::mc_settings_set_enable_intel_sprint_strafe(!intel);
                        crate::settings::mc_settings_save();
                    }
                }
            });
            // ---- View ------------------------------------------------
            ui.menu("View", || {
                hud_mode_menu(ui, "Show FPS-Hud", HudKind::Fps);
                hud_mode_menu(ui, "Show Keystroke-Mouse-Hud", HudKind::Keystroke);
                ui.menu("UI Scale", || {
                    let cur_num = (crate::settings::mc_settings_get_scale() * 4.0).round() as i32;
                    for i in 4..=20i32 {
                        if ui.menu_item_enabled_selected_no_shortcut(
                            format!("{}%", 25 * i),
                            cur_num == i,
                            true,
                        ) {
                            crate::settings::mc_settings_set_scale(i as f32 / 4.0);
                            RELOAD_FONT.store(true, Ordering::Relaxed);
                            crate::settings::mc_settings_save();
                        }
                    }
                });
                let moving = MOVING_HUDS.load(Ordering::Relaxed);
                if ui.menu_item_enabled_selected_no_shortcut("Move huds", moving, true) {
                    MOVING_HUDS.store(!moving, Ordering::Relaxed);
                    if moving {
                        // Leaving move mode — persist the final dragged positions
                        // (upstream imgui_ui.cpp:719).
                        crate::settings::mc_settings_save();
                    }
                }
            });
            // ---- Video -----------------------------------------------
            ui.menu("Video", || {
                let vsync = crate::settings::mc_settings_get_vsync();
                if ui.menu_item_enabled_selected_no_shortcut("Use VSync", vsync, true) {
                    // SAFETY: EGL call, context current on this thread.
                    unsafe { crate::eglut::window::eglutSwapInterval(if vsync { 0 } else { 1 }) };
                    crate::settings::mc_settings_set_vsync(!vsync);
                    crate::settings::mc_settings_save();
                }
                let fullscreen = crate::settings::mc_settings_get_fullscreen();
                if ui.menu_item_enabled_selected_no_shortcut(
                    "Toggle Fullscreen",
                    fullscreen,
                    true,
                ) {
                    // SAFETY: eglut state flip, no GL involved.
                    unsafe { eglutToggleFullscreen() };
                    crate::settings::mc_settings_set_fullscreen(!fullscreen);
                    crate::settings::mc_settings_save();
                }
            });
            // ---- Help ------------------------------------------------
            ui.menu("Help", || {
                if ui.menu_item_enabled_selected_no_shortcut("About", state.show_about, true) {
                    state.show_about = !state.show_about;
                }
            });
            menubar_h = ui.window_size()[1];
        });
        // Keep the game's input offset in sync (window_callbacks subtracts
        // this from mouse/touch Y, like Settings::menubarsize upstream).
        crate::settings::mc_settings_set_menubarsize(menubar_h as i32);
    } else {
        crate::settings::mc_settings_set_menubarsize(0);
        state.menu_focused = false;
    }

    // File picker (manifest imgui_ui.cpp:840-848, #ifndef NDEBUG only).
    #[cfg(debug_assertions)]
    if state.file_picker_open {
        let mut open = state.file_picker_open;
        ui.window("filepicker")
            .opened(&mut open)
            .build(|| {
                ui.input_text("Path", &mut state.file_picker_path).build();
                ui.button("Open");
            });
        state.file_picker_open = open;
    }

    let fonts = state.fonts;
    let font_sizes = state.font_sizes;
    let fps = state.fps_smoothed;
    draw_fps_hud(fps, ui, locked);
    draw_keystroke_hud(fonts, font_sizes, ui, locked);

    if state.show_about {
        ui.window("About")
            .opened(&mut state.show_about)
            .position([80.0, 80.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("MinecraftRust launcher");
                ui.text(format!(
                    "Dear ImGui {} / imgui_ui.rs overlay",
                    unsafe { std::ffi::CStr::from_ptr(dear_imgui_sys::igGetVersion()) }
                        .to_string_lossy()
                ));
                ui.separator();
                ui.text(format!("OS: {}", std::env::consts::OS));
                ui.text(format!("Arch: {}", std::env::consts::ARCH));
                // SAFETY: GL strings are valid while the context lives.
                unsafe {
                    ui.text(format!("GL Vendor: {}", gl_string(GL_VENDOR)));
                    ui.text(format!("GL Renderer: {}", gl_string(GL_RENDERER)));
                    ui.text(format!("GL Version: {}", gl_string(GL_VERSION)));
                }
            });
    }

    // Ends the Ui borrow; render_legacy produces the draw data (legacy path:
    // no managed texture requests, font atlas already carries our GLuint id).
    let (want_mouse, want_keyboard) = {
        let io = state.ctx.io();
        (io.want_capture_mouse(), io.want_capture_keyboard())
    };
    let frame = state.ctx.render_legacy();
    WANT_CAPTURE_MOUSE.store(want_mouse, Ordering::Relaxed);
    WANT_CAPTURE_KEYBOARD.store(want_keyboard, Ordering::Relaxed);

    // SAFETY: context is current on this thread (we are inside the swap path).
    unsafe {
        state.renderer.render(frame.draw_data());
    }
}

// ============================================================
// HUDs (port of the fps-hud / keystroke-mouse-hud windows)
// ============================================================

#[derive(Copy, Clone, PartialEq)]
enum HudKind {
    Fps,
    Keystroke,
}

fn hud_mode_get(kind: HudKind) -> i32 {
    match kind {
        HudKind::Fps => crate::settings::mc_settings_get_fps_hud_mode(),
        HudKind::Keystroke => crate::settings::mc_settings_get_keystroke_hud_mode(),
    }
}

fn hud_mode_set(kind: HudKind, v: i32) {
    match kind {
        HudKind::Fps => crate::settings::mc_settings_set_fps_hud_mode(v),
        HudKind::Keystroke => crate::settings::mc_settings_set_keystroke_hud_mode(v),
    }
}

/// "None / Always / Ingame" visibility submenu (manifest canShowHud modes).
fn hud_mode_menu(ui: &Ui, label: &str, kind: HudKind) {
    ui.menu(label, || {
        for (name, mode) in [("None", 0), ("Always", 1), ("Ingame", 2)] {
            if ui.menu_item_enabled_selected_no_shortcut(name, hud_mode_get(kind) == mode, true) {
                hud_mode_set(kind, mode);
                crate::settings::mc_settings_save();
            }
        }
    });
}

/// Mode 2 = ingame only (pointer locked, like CorePatches::isMouseLocked).
fn hud_visible(kind: HudKind, locked: bool) -> bool {
    match hud_mode_get(kind) {
        1 => true,
        2 => locked,
        _ => false,
    }
}

fn draw_fps_hud(fps: f32, ui: &Ui, locked: bool) {
    if !hud_visible(HudKind::Fps, locked) {
        return;
    }
    let moving = MOVING_HUDS.load(Ordering::Relaxed);
    let mut flags = WindowFlags::NO_DECORATION
        | WindowFlags::ALWAYS_AUTO_RESIZE
        | WindowFlags::NO_SAVED_SETTINGS
        | WindowFlags::NO_FOCUS_ON_APPEARING
        | WindowFlags::NO_NAV;
    if !moving {
        flags |= WindowFlags::NO_MOVE | WindowFlags::NO_MOUSE_INPUTS;
    }
    let text = format!(
        "{:.3} ms/frame ({:.1} FPS)",
        1000.0 / fps,
        fps
    );
    let ts = ui.calc_text_size(&text);
    let est_w = ts[0] + 16.0;
    let est_h = ts[1] + 16.0;
    let mut w = 0;
    let mut h = 0;
    unsafe { eglutGetWindowSize(&mut w, &mut h) };
    let fx = crate::settings::mc_settings_get_fps_hud_x() as f32 / 10_000.0;
    let fy = crate::settings::mc_settings_get_fps_hud_y() as f32 / 10_000.0;
    let pos = [
        (fx * (w as f32 - est_w)).clamp(0.0, (w as f32 - est_w).max(0.0)),
        (fy * (h as f32 - est_h)).clamp(0.0, (h as f32 - est_h).max(0.0)),
    ];
    // No window border, like upstream's PushStyleVar(WindowBorderSize, 0).
    let _nb = ui.push_style_var(dear_imgui_rs::StyleVar::WindowBorderSize(0.0));
    let mut b = ui.window("##fps-hud").flags(flags).bg_alpha(0.35);
    if !moving {
        b = b.position(pos, Condition::Always);
    }
    b.build(|| {
        ui.text(&text);
        if moving {
            // Persist the dragged position as viewport fractions.
            let p = ui.window_pos();
            let s = ui.window_size();
            let vw = (w as f32 - s[0]).max(1.0);
            let vh = (h as f32 - s[1]).max(1.0);
            crate::settings::mc_settings_set_fps_hud_x(
                ((p[0] / vw).clamp(0.0, 1.0) * 10_000.0) as u32,
            );
            crate::settings::mc_settings_set_fps_hud_y(
                ((p[1] / vh).clamp(0.0, 1.0) * 10_000.0) as u32,
            );
        }
    });
}

const KEY_W: i32 = 87;
const KEY_A: i32 = 65;
const KEY_S: i32 = 83;
const KEY_D: i32 = 68;
const KEY_SPACE: i32 = 32;

/// WASD + space + LMB/RMB CPS overlay (manifest's keystroke-mouse-hud).
fn draw_keystroke_hud(fonts: [FontId; 4], font_sizes: [f32; 4], ui: &Ui, locked: bool) {
    if !hud_visible(HudKind::Keystroke, locked) {
        return;
    }
    let moving = MOVING_HUDS.load(Ordering::Relaxed);
    let scale = ui_scale();
    let small_pad = 5.0 * scale;

    // Metrics from the very-large font.
    let (kx, ky) = {
        let _f = ui.push_font(fonts[3]);
        let ts = ui.calc_text_size("W");
        (ts[0] + 15.0 * scale * 2.0, ts[1] + 5.0 * scale * 2.0)
    };

    let width = kx * 3.0 + small_pad * 2.0;
    let cps_h = ky * 1.15;
    let space_h = ky * 0.8;
    let height = ky * 2.0 + small_pad * 3.0 + space_h + cps_h;

    let mut w = 0;
    let mut h = 0;
    unsafe { eglutGetWindowSize(&mut w, &mut h) };
    let fx = crate::settings::mc_settings_get_keystroke_hud_x() as f32 / 10_000.0;
    let fy = crate::settings::mc_settings_get_keystroke_hud_y() as f32 / 10_000.0;
    let pos = [
        (fx * (w as f32 - width)).clamp(0.0, (w as f32 - width).max(0.0)),
        (fy * (h as f32 - height)).clamp(0.0, (h as f32 - height).max(0.0)),
    ];

    let mut flags = WindowFlags::NO_DECORATION
        | WindowFlags::NO_SAVED_SETTINGS
        | WindowFlags::NO_FOCUS_ON_APPEARING
        | WindowFlags::NO_NAV;
    if !moving {
        flags |= WindowFlags::NO_MOVE | WindowFlags::NO_MOUSE_INPUTS;
    }

    let lmb_cps = recent_clicks(&mut LMB_CLICKS.lock().unwrap());
    let rmb_cps = recent_clicks(&mut RMB_CLICKS.lock().unwrap());
    let lmb_down = mouse_down(0);
    let rmb_down = mouse_down(1);

    let _nb = ui.push_style_var(dear_imgui_rs::StyleVar::WindowBorderSize(0.0));
    let mut b = ui.window("##keystroke-hud").flags(flags).bg_alpha(0.0);
    if !moving {
        b = b.position(pos, Condition::Always);
    }
    // Fixed-size transparent canvas; everything is drawn via the draw list at
    // absolute coordinates (avoids content-region/padding offset issues).
    const CANVAS_PAD: f32 = 8.0;
    b.size([width + CANVAS_PAD * 2.0, height + CANVAS_PAD * 2.0], Condition::Always)
        .build(|| {
            let wp = ui.window_pos();
            let origin = [wp[0] + CANVAS_PAD, wp[1] + CANVAS_PAD];
            let dl = ui.get_window_draw_list();
            // FrameBg is gray(0.7) like upstream's PushStyleColor, with
            // SetNextWindowBgAlpha values 0.70 pressed / 0.20 up / 0.15 idle.
            const GRAY: f32 = 0.7;
            let bg_up = [GRAY, GRAY, GRAY, 0.20];
            let bg_down = [GRAY, GRAY, GRAY, 0.70];
            let bg_idle = [GRAY, GRAY, GRAY, 0.15];

            // WASD grid.
            for (col, row, vk, label) in [
                (1usize, 0usize, KEY_W, "W"),
                (0, 1, KEY_A, "A"),
                (1, 1, KEY_S, "S"),
                (2, 1, KEY_D, "D"),
            ] {
                let p = [
                    origin[0] + col as f32 * (kx + small_pad),
                    origin[1] + row as f32 * (ky + small_pad),
                ];
                dl.add_rect(
                    p,
                    [p[0] + kx, p[1] + ky],
                    if key_down(vk) { bg_down } else { bg_up },
                )
                .filled(true)
                .build();
                let ts = {
                    let _f = ui.push_font(fonts[3]);
                    ui.calc_text_size(label)
                };
                dl.add_text_with_font(
                    fonts[3],
                    font_sizes[3],
                    [p[0] + (kx - ts[0]) * 0.5, p[1] + (ky - ts[1]) * 0.5],
                    [1.0f32, 1.0, 1.0, 1.0],
                    label,
                    0.0,
                    None,
                );
            }

            // Space bar with a line through the middle.
            let space_y = 2.0 * (ky + small_pad);
            let sp = [origin[0], origin[1] + space_y];
            dl.add_rect(
                sp,
                [sp[0] + width, sp[1] + space_h],
                if key_down(KEY_SPACE) { bg_down } else { bg_idle },
            )
            .filled(true)
            .build();
            dl.add_line_h(
                sp[0] + width / 3.0,
                sp[0] + width * 2.0 / 3.0,
                sp[1] + space_h / 2.0,
                [1.0f32, 1.0, 1.0, 1.0],
                2.0,
            );

            // LMB / RMB clicks-per-second boxes.
            let cps_y = space_y + space_h + small_pad;
            let box_w = (width - small_pad) / 2.0;
            for (i, label, cps, down) in
                [(0i32, "LMB", lmb_cps, lmb_down), (1, "RMB", rmb_cps, rmb_down)]
            {
                let p = [origin[0] + i as f32 * (box_w + small_pad), origin[1] + cps_y];
                dl.add_rect(
                    p,
                    [p[0] + box_w, p[1] + cps_h],
                    if down { bg_down } else { bg_idle },
                )
                .filled(true)
                .build();
                // Large label on top, medium count below (upstream
                // fontLargeSize + fontMediumSize, both CenterText'd).
                let ls = {
                    let _f = ui.push_font(fonts[2]);
                    ui.calc_text_size(label)
                };
                let count_txt = format!("{cps} CPS");
                let cs = {
                    let _f = ui.push_font(fonts[1]);
                    ui.calc_text_size(&count_txt)
                };
                dl.add_text_with_font(
                    fonts[2],
                    font_sizes[2],
                    [p[0] + (box_w - ls[0]).max(0.0) * 0.5, p[1] + 5.0 * scale],
                    [1.0f32, 1.0, 1.0, 1.0],
                    label,
                    0.0,
                    None,
                );
                dl.add_text_with_font(
                    fonts[1],
                    font_sizes[1],
                    [
                        p[0] + (box_w - cs[0]).max(0.0) * 0.5,
                        p[1] + 5.0 * scale + ky * 0.55,
                    ],
                    [1.0f32, 1.0, 1.0, 1.0],
                    &count_txt,
                    0.0,
                    None,
                );
            }

            if moving {
                // Persist the dragged position as viewport fractions.
                let vw = (w as f32 - width).max(1.0);
                let vh = (h as f32 - height).max(1.0);
                crate::settings::mc_settings_set_keystroke_hud_x(
                    ((wp[0] / vw).clamp(0.0, 1.0) * 10_000.0) as u32,
                );
                crate::settings::mc_settings_set_keystroke_hud_y(
                    ((wp[1] / vh).clamp(0.0, 1.0) * 10_000.0) as u32,
                );
            }
        });
}
