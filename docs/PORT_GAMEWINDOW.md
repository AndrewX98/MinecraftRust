# Port: gamewindow

**Status:** ~80% done. Pure Rust `eglut` (`crates/client/src/eglut/`, 1,348 lines) is the active X11/EGL windowing path. C++ `gamewindow` is a thin facade over it.

## C++ to remove

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

## Existing Rust

- `crates/client/src/eglut/` — window, event, callbacks, mouse, egl, state, util, xinput (1,348 lines). Rust eglut exports the `eglut_*` C symbols the C++ wrapper calls.

## Steps

1. Replace `jni_bridge_stub.cpp:287 mc_create_window_and_setup_graphics()` with a Rust equivalent in `capi.rs` that: calls `XInitThreads`, creates the window via `eglut`, seeds `FakeEGL` state (`eglutGetWindowHandle`, surface/context) — currently the only C++ entrypoint that constructs `GameWindowManager` + `EGLUTWindow`.
2. Replace `window_callbacks_stub.cpp:699` (`GameWindowManager::getManager()`) and `fake_looper_stub.cpp:62` (`FakeLooper::setWindow`) with Rust-side window handle passing through the existing FakeLooper bridge.
3. Port `GameWindowManager` facade + `EGLUTWindowManager::createWindow` + `EGLUTWindow` methods (`makeCurrent`, `swapBuffers`, `pollEvents`, `setCursorDisabled`, `setFullscreen`, `setClipboardText`, `setSwapInterval`, `setIcon`) directly onto Rust eglut calls. All are 1:1 eglut passthroughs.
4. Move `JoystickManager::handleMissingGamePadMapping` logic into the Rust gamepad port (see `PORT_LINUX_GAMEPAD.md`).
5. Delete the C++ `gamewindow` target from `cpp-bridge-sys/build.rs`; remove `-GAMEWINDOW` deps.

## Done when

- `nm` shows no `EGLUTWindow`/`GameWindowManager` symbols.
- Window still creates, mouse/keyboard input works, game loads to main menu.
