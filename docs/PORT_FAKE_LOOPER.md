# Port: FakeLooper + WindowCallbacks + FakeInputQueue + CorePatches

**Status: PLANNED** — the C++ `FakeLooper`/`WindowCallbacks`/`FakeInputQueue`/`CorePatches` classes are the last big user-facing input/window-state layer still in C++ (beyond the JNI/VM core). The Rust driver `crates/client/src/fake_looper.rs` already implements the `ALooper_*`/`AInputQueue_*` hooks and delegates 17 `fake_looper_*` FFI helpers to C++ (`jni_bridge_stub.cpp`). This port moves the whole stack to Rust in **incremental phases with a verification gate after each** (build + boot + new unit tests). The C++ `GameWindow` (gamewindow lib) stays as the window token through Phase 4; deleting it entirely is a separate follow-up (Phase 5).

## Role

Android-style input/window orchestration for the game process:

- **FakeLooper** — fake `ALooper` (prepare/addFd/attachInputQueue/pollAll) that pumps the window event loop and drains X11 events into the input queue. Hooks registered for `libandroid.so` (`ALooper_prepare`, `ALooper_addFd`, `ALooper_pollAll`, `ALooper_pollOnce`, `AInputQueue_attachLooper`, `ANativeActivity_finish`).
- **FakeInputQueue** — fake `AInputQueue` holding `FakeKeyEvent`/`FakeMotionEvent` deques, consumed by the game through the `libandroid.so` accessor hooks (`AInputQueue_getEvent`, `AKeyEvent_get*`, `AMotionEvent_get*`).
- **WindowCallbacks** — the input-mode state machine (Keyboard/Mouse/Touch + gamepad), receives events from the window (currently the C++ `EGLUTWindow` trampolines → eglut), and forwards them to the game via `jni_support_send_*` (Rust) or the C++ `text_handler_*` FFI. Also hosts the registries the game fills through the `game_window_*` symbols.
- **CorePatches** — the `GameWindowHandle` (window + callbacks + mouseLocked), the `onWindowCreatedCallbacks` list, and the 8 `game_window_*` symbols registered into `libmcpelauncher_gamewindow.so` Rust linker state that the game calls directly.

## C++ to be removed

| File | Lines | Phase | Role |
|------|-------|-------|------|
| `fake_inputqueue_stub.cpp` | 112 | 1 | `FakeInputQueue` + `libandroid.so` input hooks (`initHybrisHooks`) |
| `core_patches_stub.cpp` | 153 | 2 | `CorePatches` state + `game_window_*` symbol registration |
| `window_callbacks_stub.cpp` | 754 | 3 | `WindowCallbacks` input mode machine + handlers + registries |
| `fake_looper_stub.cpp` | 159 | 4 | C++ `FakeLooper` class (statics, prepare/pollAll, initializeWindow) |
| `main_stubs.cpp` (shrinks) | 28 | 4 | dead `Keyboard`/`Mouse` statics + no-op patch stubs; keep `LauncherOptions options` (used by `jni_support.cpp`) |
| `jni_bridge_stub.cpp` (17 helpers) | ~120 | 4 | the `fake_looper_*` extern "C" helpers :165-275 |

Plus `manifest_headers/fake_inputqueue.h`, `manifest_headers/window_callbacks.h`, `manifest_headers/core_patches.h` (`window_callbacks_map_*` FFI stays — already Rust in `rust_bridge.rs`).

## Key findings that shape the port

1. **Direct-input path is dead code.** `SymbolsHelper` was deleted → `Mouse::feed` = nullptr and `Keyboard::_states` = nullptr → `useDirectMouseInput`/`useDirectKeyboardInput` are always `false` (`window_callbacks_stub.cpp:40-41`). The Rust port **drops** all `Mouse::feed`/`Keyboard::_states` writes and their branches.
2. **GameWindow event callbacks are replaceable by Rust eglut funcs.** Rust eglut (`eglut/callbacks.rs`) exposes every needed slot (reshape, close, mouse, mouse_raw, mouse_button, keyboard, special, touch_*, drop, paste, focus, idle). Scroll arrives as X buttons 4/5 in `mouse_button_cb` (same as C++ eglut). Cursor/fullscreen already have Rust equivalents: `eglutSetMousePointerVisibility`, `eglutSetMousePointerLocked`, `eglutFullscreen`, `eglutWindowed` (`eglut/mouse.rs`).
3. **FakeInputQueue events are opaque to the game.** The game only reads events through the `libandroid.so` accessor hooks, never by direct field access. So the struct layout only needs to be internally consistent between the Rust queue storage and the Rust hooks. `FakeMotionEvent`'s `std::function axisFunction` is only ever constructed empty (never set), so Rust stores it as an opaque 32-byte slot and the axis hook falls back to `dy` — matching current behavior.
4. **The `game_window_*` symbols are live.** The game calls `game_window_add_keyboard_callback`/`game_window_add_mouse_*_callback`/`game_window_add_window_creation_callback`/`game_window_add_swap_buffers_callback`/`game_window_get_primary_window`/`game_window_is_mouse_locked`/`game_window_get_input_mode` via `libmcpelauncher_gamewindow.so` Rust linker state. The Rust CorePatches must keep these symbols + registries and a stable-address `GameWindowHandle` (`Box::leak`).
5. **Text input handler is already Rust** (`text_input_handler.rs`); `jni_support_get_text_input_handler` returns it via `jnivm_get_text_input_handler()`. WindowCallbacks keeps calling the `text_handler_*` FFI.
6. **GameWindow lifetime.** `mc_create_window_and_setup_graphics` (C++) creates the GameWindow and `FakeLooper::setWindow` held it in a `shared_ptr`. After the port, a process-lifetime `shared_ptr<GameWindow>` in `jni_bridge_stub.cpp` owns it and Rust holds the raw token via `mc_get_window_token()`.
7. **Settings stays as-is.** `settings_stub.cpp` is frozen state in this build (menubarsize/fullscreen never change — imgui not compiled, `save()` no-op). Rust reads it via small `mc_settings_*` FFI rather than porting it.

## Compatibility notes

- **FakeInputEvent layout** (`#[repr(C)])`: source@0, type@4, deviceId@8; key action@12/keyCode@16/metaState@20 (size 24); motion action@12/pointerId@16/x@20/y@24/axis-slot@32 (32 bytes opaque, was `std::function`)/btn@64/dy@68 (size 72). Locked by unit tests in Phase 1.
- **Input-mode machine**: the Keyboard/Mouse/Touch auto-switch logic, `useRawInput`, `forcedMode`, `inputModeSwitchDelay`, and `hasInputMode` semantics must be preserved or in-game look changes.
- **`startSendEvents` + `markRequeueGamepadInput`**: pollAll gating semantics (send-events latch, per-frame gamepad axis requeue) must be preserved.
- **eglut idle/display funcs**: the C++ `EGLUTWindow` ctor sets eglut idle/display funcs. Rust WindowCallbacks overwrites the input slots; the redraw/dispatch semantics must be preserved or rendering breaks (verify boot + main menu).

## Steps taken

None yet — this is the plan.

## Phase plan (each phase ends with a verification gate)

### Phase 1 — FakeInputQueue → Rust
- New `fake_inputqueue.rs` (~200 lines): `#[repr(C)]` event structs (axis as opaque 32-byte slot), `FakeInputQueue` (VecDeques, prealloc 100), `getEvent`/`finishEvent`/`addEvent`/`hasEvents`, and `mc_register_fake_input_queue_hooks` (all ~17 `libandroid.so` symbols incl. `AInputQueue_getEvent`, `AMotionEvent_getAxisValue` fallback to `dy`).
- Keep the C++ `FakeInputQueue` class as a **thin forwarding wrapper** (add/get/finish/hasEvents → Rust via extern "C") so C++ `WindowCallbacks`/`FakeLooper` keep working with the struct types while Rust owns storage + hooks. `mc_setup_android_hooks` (`jni_bridge_stub.cpp:130`) swaps `FakeInputQueue::initHybrisHooks` → `mc_register_fake_input_queue_hooks`.
- **Tests**: struct size/offset assertions, queue add/get/finish ordering, empty-queue `-1`, axis fallback.
- **Verify**: `cargo test -p client`; `cargo build -p client`; boot to main menu, mouse/touch work.

### Phase 2 — CorePatches → Rust
- New `core_patches.rs` (~150 lines): `GameWindowHandle { window: *mut c_void, callbacks: *mut c_void, mouse_locked: bool }` at a stable address, `on_window_created_callbacks: Vec<Box<dyn FnMut()>>`, show/hide mouse (`setCursorLocked`), setFullscreen, setPendingDelayedPaste, install (→ existing Rust `core_patches_install_impl`), and registration of the 8 `game_window_*` symbols via `linker_add_symbols_to_library_rust`.
- `callbacks` is an opaque pointer; the `game_window_*` handlers forward through small `window_callbacks_*` extern "C" helpers still backed by the C++ WindowCallbacks until Phase 3.
- `fake_looper_stub.cpp` prepare swaps `CorePatches::setGameWindow(_callbacks)` → `core_patches_set_game_window(_callbacks)` (Rust). `mc_setup_android_hooks` line 161 `CorePatches::loadGameWindowLibrary()` → Rust registration.
- **Tests**: registry callback invocation (add keyboard callback → invoked on dispatch), mouse-lock flag toggling.
- **Verify**: `game_window_*` still resolve in-game (game UI input callbacks work); boot to main menu.

### Phase 3 — WindowCallbacks → Rust (the big one)
- New `window_callbacks.rs` (~600 lines): `InputMode`, `GamepadData`, input-mode state machine, all handlers (onMouseButton/Position/RelativePosition/Scroll, onTouchStart/Update/End, onKeyboard, onKeyboardText, onDrop, onPaste, onGamepadState/Button/Axis), `sendMouseEvent`/`sendTouchEvent`, `startSendEvents`/`markRequeueGamepadInput`, delayed paste, `loadGamepadMappings` (runtime `gamecontrollerdb.txt` via `DEV_EXTRA_PATHS`), the registries, and static maps (delegate to the existing Rust `window_callbacks_map_*`).
- `registerCallbacks` sets the **Rust eglut funcs** (overwriting C++ EGLUTWindow trampolines); keyboard mapping via `eglut_sym`/keycode tables (ported from C++ EGLUTWindow); cursor/fullscreen via eglut `mouse.rs`; window size via `game_window_get_size` (C++ FFI, kept).
- C++ `fake_looper_stub.cpp` holds an opaque pointer via `window_callbacks_create`/`window_callbacks_register`; `gamepad/window.rs` (Rust) calls the Rust WindowCallbacks global directly, dropping the `window_callbacks_on_gamepad_*` C++ thunks.
- Delete `window_callbacks_stub.cpp`.
- **Tests**: key/gamepad/mouse mapping tables, input-mode switching.
- **Verify**: keyboard/mouse/gamepad work in-game; boot to main menu.

### Phase 4 — FakeLooper → Rust
- Rework `fake_looper.rs` (~380 lines) to own all looper state (thread_local current, `prepared`, text-input latch, window token, queue) and absorb the 17 `fake_looper_*` helpers; port `prepare` (initialize window via token, `jni_support_on_window_created` + C++ `JniSupport::onWindowCreated`, create WindowCallbacks, register core patches, show, patches, makeCurrent), `onGameActivityClose`, cleanup.
- `mc_create_window_and_setup_graphics` keeps the GameWindow in a process-lifetime `shared_ptr` + `mc_get_window_token()`; delete `FakeLooper` class + its statics.
- Delete `fake_looper_stub.cpp`; strip the 17 helpers from `jni_bridge_stub.cpp`; shrink `main_stubs.cpp` (keep `LauncherOptions options`).
- **Tests**: `EventEntry` fill/is_valid, poll ordering, text-input latch transitions.
- **Verify**: boot + `RUST_LOG=linker=info`; `nm -C` shows no remaining `FakeLooper`/`WindowCallbacks`/`FakeInputQueue`/`CorePatches` C++ symbols.

### Phase 5 (follow-up, separate milestone) — delete the `mcpelauncher-gamewindow` C++ lib
- Rust eglut creates the window directly (no `GameWindowManager`); EGL surface token switches from `GameWindow*` to the Rust eglut window; `game_window_make_current`/`swap_buffers`/`get_size` move to Rust; delete the 6 gamewindow .cpp files + `include/game-window` + `include/eglut` + gamepad C++.

## Done when

- `nm -C target/debug/client` shows no `FakeLooper`, `WindowCallbacks`, `FakeInputQueue`, or `CorePatches` C++ symbols.
- `cargo test -p client` passes (per-phase unit tests).
- Game boots to main menu with keyboard/mouse/gamepad working after each phase.
- `mcpelauncher-gamewindow` and `mcpelauncher-client-jni` static libs reduced to the JNI/VM core (Phase 5 removes the former entirely).

## Depends on / used by

- Depends on: Rust eglut (`crates/client/src/eglut/`) for window callbacks; `jni_support.rs` for `jni_support_send_*`/`on_window_created`/`set_looper_running`; `text_input_handler.rs` for `text_handler_*`; `rust_bridge.rs` for `window_callbacks_map_*` + `core_patches_install_impl`; `gamepad/window.rs` (Rust joystick) for gamepad events.
- Used by: the game via `libandroid.so` looper/input hooks and the `game_window_*` symbols; startup via `mc_setup_android_hooks`/`mc_create_window_and_setup_graphics`.
- After Phase 4, the remaining compiled C++ is the JNI/VM core (`libjnivm` FakeJni/Baron + `jni_support.cpp`/`main_activity.cpp`/`jnivm_class_wrappers.cpp`/`jni_descriptors.cpp`/`jni_bridge_stub.cpp`/`lib_http_client*.cpp`) + live stubs (`http_client_stubs`, `fake_assetmanager`, `fake_audio`, `fake_egl`, `xal_webview_factory`, `text_input_handler` bridge).
