# Port: gamewindow

**Status: ✅ DONE (Phase 5, 2026-08-05).** The C++ `mcpelauncher-gamewindow` static lib is deleted. Pure Rust `eglut` (`crates/client/src/eglut/`) + `crate::game_window.rs` create and own the X11/EGL window directly.

## What was removed

| File | Lines | Role |
|------|-------|------|
| `manifest_libs/gamewindow/window_eglut.cpp` | 453 | `EGLUTWindow` — calls `eglut_*` C funcs (Rust-backed) |
| `manifest_libs/gamewindow/window_manager_eglut.cpp` | 45 | `EGLUTWindowManager` factory |
| `manifest_libs/gamewindow/joystick_manager.cpp` | 54 | `JoystickManager::handleMissingGamePadMapping` (NDEBUG debug helper) |
| `manifest_libs/gamewindow/joystick_manager_linux_gamepad.cpp` | 149 | `LinuxGamepadJoystickManager` — window↔gamepad glue |
| `manifest_libs/gamewindow/window_with_linux_gamepad.cpp` | 17 | `WindowWithLinuxJoystick` base |
| `manifest_libs/gamewindow/game_window_manager.cpp` | 8 | `GameWindowManager::getManager()` singleton |
| `manifest_libs/gamewindow/game_window_error_handler.cpp` | 5 | error handler stub |
| headers (`.h`) | — | `window_eglut.h`, `window_manager_eglut.h`, `joystick_manager*.h`, `window_with_linux_gamepad.h`, GLFW variants (unused) |
| `include/game-window/{game_window_manager.h,game_window_error_handler.h}` | — | manager + error-handler headers |
| `include/eglut/{eglut.h,eglut_x11.h}` | — | C eglut headers (the C funcs were already Rust-backed) |
| `include/game-window/game_window.h` | trimmed | now only the `GraphicsApi`/`KeyAction`/`MouseButtonAction` enums (consumed by `text_input_handler.h`, `main.h`, `jni_support.h`) + `key_mapping.h` (kept) |

## Rust replacement

- `crates/client/src/game_window.rs` (~120 lines) owns the window token:
  - `create_window(title)`: `eglutInitX11ClassInstanceName` → `eglutInit(0, null)` → `eglutInitWindowSize(eglutScreenWidth(), eglutScreenHeight())` → `eglutInitAPIMask(EGLUT_OPENGL_ES2_BIT)` → `eglutCreateWindow("Minecraft")`; token = `eglutGetWindowHandle()` as `*mut c_void` (doubles as `ANativeWindow*`/`GameWindow*`/FakeEGL surface handle).
  - Exports: `mc_get_window_token`, `mc_create_default_window` (gamepad mappings → create_window → `fake_egl_setup_gl_overrides`), `mc_window_show` (`eglutShowWindow`), `fake_looper_window_poll_events` (`eglutPollEvents`), `fake_looper_window_start/stop_text_input` (no-ops), `game_window_make_current` (`eglutMakeCurrent(1|-1)`), `game_window_swap_buffers` (`eglutSwapBuffers`), `game_window_get_size`/`mc_get_window_size` (`eglutGetWindowSize`), `mc_set_clipboard_text` (`eglutSetClipboardText`), `mc_get_key_from_key_code` (always 0), `mc_create_window_and_setup_graphics` (`XInitThreads` → create_window → seed FakeEGL → `mc_relocate_glesv2_symbols`).
  - `real_egl_get_proc_address()` `dlopen`s `libEGL.so` (`RTLD_LAZY|RTLD_LOCAL`) and `dlsym`s `eglGetProcAddress` — same real function the C++ `EGLUTWindowManager::getProcAddrFunc` returned; passing the FakeEGL wrapper instead self-deadlocks (wrapper locks `HOST_PROC_OVERRIDES`, which `fake_egl_setup_gl_overrides` already holds).
- `main_activity.h`: `window` member is `void*`; `getScreenWidth/Height`/`getDisplayWidth/Height` call `extern "C" mc_get_window_size`. `main_activity.cpp`: `setClipboard`/`getKeyFromKeyCode` via `mc_set_clipboard_text`/`mc_get_key_from_key_code`; FilePickerFactory error calls → `Log::warn`.
- `fake_egl_query_surface` EGL_WIDTH/HEIGHT falls back through `game_window::mc_get_window_size` (keeps the symbol referenced).
- `cpp-bridge-sys/build.rs`: gamewindow `cc::Build` block removed. `client/build.rs`: `-lmcpelauncher-gamewindow` link directive removed.

## Steps taken

1. ~~Replace `jni_bridge_stub.cpp` `mc_create_window_and_setup_graphics()` with Rust~~ ✅ done — `capi.rs::create_window_and_setup_graphics()` → `crate::game_window::mc_create_window_and_setup_graphics()`.
2. ~~Replace `GameWindowManager::getManager()` window-handle passing~~ ✅ done in Phase 4 — window token passing is Rust `fake_looper.rs`.
3. ~~Port `GameWindowManager` facade + `EGLUTWindowManager::createWindow` + `EGLUTWindow` methods onto Rust eglut~~ ✅ done — `game_window.rs` is a thin wrapper over `crate::eglut`.
4. ~~Move `JoystickManager`/gamepad glue to the Rust gamepad port~~ ✅ done — `gamepad/mod.rs` + `gamepad/joystick.rs` (`LinuxJoystickManager`) already existed; gamepad C++ deleted.
5. ~~Delete the C++ `gamewindow` target from build.rs + `-GAMEWINDOW` deps~~ ✅ done.

## Done when

- `nm` shows no `EGLUTWindow`/`GameWindowManager` symbols. ✅
- Window still creates, mouse/keyboard input works, game loads to main menu. ✅ (boot to main menu + `eglSwapBuffers ok`)
