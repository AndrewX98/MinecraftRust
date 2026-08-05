# Port: FakeLooper + WindowCallbacks + FakeInputQueue + CorePatches

**Status: Phases 1–5 DONE** — `FakeInputQueue`, `CorePatches`, `WindowCallbacks`, `FakeLooper`, and the `mcpelauncher-gamewindow` C++ lib are fully ported to Rust; their C++ files are deleted and `nm -C` shows zero remaining `FakeLooper`/`WindowCallbacks`/`FakeInputQueue`/`CorePatches`/`EGLUTWindow`/`GameWindowManager` C++ symbols. The window token lives in Rust `crate::game_window.rs` (Phase 5).

## Role

Android-style input/window orchestration for the game process:

- **FakeLooper** — fake `ALooper` (prepare/addFd/attachInputQueue/pollAll) that pumps the window event loop and drains X11 events into the input queue. Hooks registered for `libandroid.so` (`ALooper_prepare`, `ALooper_addFd`, `ALooper_pollAll`, `ALooper_pollOnce`, `AInputQueue_attachLooper`, `ANativeActivity_finish`).
- **FakeInputQueue** — fake `AInputQueue` holding `FakeKeyEvent`/`FakeMotionEvent` deques, consumed by the game through the `libandroid.so` accessor hooks (`AInputQueue_getEvent`, `AKeyEvent_get*`, `AMotionEvent_get*`).
- **WindowCallbacks** — the input-mode state machine (Keyboard/Mouse/Touch + gamepad), receives events from the window (currently the C++ `EGLUTWindow` trampolines → eglut), and forwards them to the game via `jni_support_send_*` (Rust) or the C++ `text_handler_*` FFI. Also hosts the registries the game fills through the `game_window_*` symbols.
- **CorePatches** — the `GameWindowHandle` (window + callbacks + mouseLocked), the `onWindowCreatedCallbacks` list, and the 8 `game_window_*` symbols registered into `libmcpelauncher_gamewindow.so` Rust linker state that the game calls directly.

## C++ to be removed

| File | Lines | Phase | Role |
|------|-------|-------|------|
| `fake_inputqueue_stub.cpp` | 112 | 1 ✅ | `FakeInputQueue` + `libandroid.so` input hooks (`initHybrisHooks`) |
| `core_patches_stub.cpp` | 153 | 2 ✅ | `CorePatches` state + `game_window_*` symbol registration |
| `window_callbacks_stub.cpp` | 754 | 3 ✅ | `WindowCallbacks` input mode machine + handlers + registries |
| `fake_looper_stub.cpp` | 159 | 4 ✅ | C++ `FakeLooper` class (statics, prepare/pollAll, initializeWindow) |
| `main_stubs.cpp` (shrinks) | 28 | 4 ✅ | dead `Keyboard`/`Mouse` statics + no-op patch stubs; keep `LauncherOptions options` (used by `jni_support.cpp`) |
| `jni_bridge_stub.cpp` (17 helpers) | ~120 | 4 ✅ | the `fake_looper_*` extern "C" helpers :165-275 |

Headers `manifest_headers/fake_inputqueue.h`, `manifest_headers/window_callbacks.h`, `manifest_headers/core_patches.h` are all deleted (the `window_callbacks_map_*` FFI stays — already Rust in `rust_bridge.rs`).

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

- **Phase 1 (a0e9ba24):** `fake_inputqueue.rs` owns all `FakeInputQueue` state + the ~17 `libandroid.so` input hooks; C++ `FakeInputQueue` became a thin forwarding wrapper; struct layout locked by unit tests.
- **Phase 2 (a457b287):** `core_patches.rs` owns `GameWindowHandle`, callback registries, and the 8 `game_window_*` symbols via `linker_add_symbols_to_library_rust`.
- **Phase 3 (028bdce3):** `window_callbacks.rs` owns the full input-mode state machine, handlers, registries, gamepad mappings; `registerCallbacks` sets the Rust eglut funcs; `window_callbacks_stub.cpp` + `core_patches_stub.cpp` deleted.
- **Phase 4 (this commit):** `fake_looper.rs` owns all looper state (thread_local `CURRENT: RefCell<Option<LooperState>>`, prepared latch, text-input latch, window token, input queue) and the 7 hooks (`prepare`/`add_fd`/`poll_all`/`poll_once`/`attach_input_queue`/`finish`/`mc_register_fake_looper_hooks`). Rust `prepare_impl` calls the C++ `mc_jni_support_on_window_created_cpp(window, queue)` (required — `jni_support.rs` reads the C++ JniSupport window), then the Rust `jni_support_on_window_created`. The 17 `fake_looper_*` FFI helpers are stripped from `jni_bridge_stub.cpp`, replaced by process globals (`g_jni_support`, `g_rust_jni_support`, `g_window`) + getters (`mc_get_jni_support`, `mc_get_rust_jni_support`, `mc_get_window_token`) and token-parameter helpers (`mc_window_show`, `fake_looper_window_poll_events`, `fake_looper_window_start/stop_text_input`, `game_window_make_current`, `game_window_swap_buffers`, `game_window_get_size`). `main_stubs.cpp` shrunk to `LauncherOptions` + no-op patch stubs; `Keyboard::*`/`Mouse::feed` dropped (last odr-users were `fake_looper_stub.cpp`). `fake_looper_stub.cpp`, `fake_looper.h`, `fake_inputqueue_stub.cpp`, `manifest_headers/fake_inputqueue.h` deleted; Rust `FakeInputQueue*` now identity-casts to `AInputQueue*`. **Gate:** `cargo build` ok, 26/26 tests, `nm -C` zero C++ class symbols, boots to main menu rendering 60fps.

## Phase plan (each phase ends with a verification gate)

### Phase 1 — FakeInputQueue → Rust ✅ DONE
- New `fake_inputqueue.rs` (~200 lines): `#[repr(C)]` event structs (axis as opaque 32-byte slot), `FakeInputQueue` (VecDeques, prealloc 100), `getEvent`/`finishEvent`/`addEvent`/`hasEvents`, and `mc_register_fake_input_queue_hooks` (all ~17 `libandroid.so` symbols incl. `AInputQueue_getEvent`, `AMotionEvent_getAxisValue` fallback to `dy`).
- Keep the C++ `FakeInputQueue` class as a **thin forwarding wrapper** (add/get/finish/hasEvents → Rust via extern "C") so C++ `WindowCallbacks`/`FakeLooper` keep working with the struct types while Rust owns storage + hooks. `mc_setup_android_hooks` (`jni_bridge_stub.cpp:130`) swaps `FakeInputQueue::initHybrisHooks` → `mc_register_fake_input_queue_hooks`.
- **Tests**: struct size/offset assertions, queue add/get/finish ordering, empty-queue `-1`, axis fallback.
- **Verify**: `cargo test -p client`; `cargo build -p client`; boot to main menu, mouse/touch work.

### Phase 2 — CorePatches → Rust ✅ DONE
- New `core_patches.rs` (~150 lines): `GameWindowHandle { window: *mut c_void, callbacks: *mut c_void, mouse_locked: bool }` at a stable address, `on_window_created_callbacks: Vec<Box<dyn FnMut()>>`, show/hide mouse (`setCursorLocked`), setFullscreen, setPendingDelayedPaste, install (→ existing Rust `core_patches_install_impl`), and registration of the 8 `game_window_*` symbols via `linker_add_symbols_to_library_rust`.
- `callbacks` is an opaque pointer; the `game_window_*` handlers forward through small `window_callbacks_*` extern "C" helpers still backed by the C++ WindowCallbacks until Phase 3.
- `fake_looper_stub.cpp` prepare swaps `CorePatches::setGameWindow(_callbacks)` → `core_patches_set_game_window(_callbacks)` (Rust). `mc_setup_android_hooks` line 161 `CorePatches::loadGameWindowLibrary()` → Rust registration.
- **Tests**: registry callback invocation (add keyboard callback → invoked on dispatch), mouse-lock flag toggling.
- **Verify**: `game_window_*` still resolve in-game (game UI input callbacks work); boot to main menu.

### Phase 3 — WindowCallbacks → Rust (the big one) ✅ DONE
- New `window_callbacks.rs` (~600 lines): `InputMode`, `GamepadData`, input-mode state machine, all handlers (onMouseButton/Position/RelativePosition/Scroll, onTouchStart/Update/End, onKeyboard, onKeyboardText, onDrop, onPaste, onGamepadState/Button/Axis), `sendMouseEvent`/`sendTouchEvent`, `startSendEvents`/`markRequeueGamepadInput`, delayed paste, `loadGamepadMappings` (runtime `gamecontrollerdb.txt` via `DEV_EXTRA_PATHS`), the registries, and static maps (delegate to the existing Rust `window_callbacks_map_*`).
- `registerCallbacks` sets the **Rust eglut funcs** (overwriting C++ EGLUTWindow trampolines); keyboard mapping via `eglut_sym`/keycode tables (ported from C++ EGLUTWindow); cursor/fullscreen via eglut `mouse.rs`; window size via `game_window_get_size` (C++ FFI, kept).
- C++ `fake_looper_stub.cpp` holds an opaque pointer via `window_callbacks_create`/`window_callbacks_register`; `gamepad/window.rs` (Rust) calls the Rust WindowCallbacks global directly, dropping the `window_callbacks_on_gamepad_*` C++ thunks.
- Delete `window_callbacks_stub.cpp`.
- **Tests**: key/gamepad/mouse mapping tables, input-mode switching.
- **Verify**: keyboard/mouse/gamepad work in-game; boot to main menu.

### Phase 4 — FakeLooper → Rust ✅ DONE
- `fake_looper.rs` (~380 lines) owns all looper state (thread_local `CURRENT`, `prepared`, text-input latch, window token, queue) and the 7 hooks; `prepare_impl` (initialize window via token, `mc_jni_support_on_window_created_cpp` + `jni_support_on_window_created`, create WindowCallbacks, register core patches, show, patches, makeCurrent), `fake_looper_finish`/`fake_looper_on_game_activity_close`.
- `mc_create_window_and_setup_graphics` keeps the GameWindow in a process-lifetime `shared_ptr` + `mc_get_window_token()`; `FakeLooper` class + its statics deleted.
- Deleted `fake_looper_stub.cpp` + `fake_looper.h` + `fake_inputqueue_stub.cpp` + `manifest_headers/fake_inputqueue.h`; stripped the 17 helpers from `jni_bridge_stub.cpp`; shrank `main_stubs.cpp` (kept `LauncherOptions options`).
- **Tests**: `EventEntry` fill/is_valid, poll ordering, text-input latch transitions (7 new → 26 total).
- **Verify (passing)**: boot + `RUST_LOG=linker=info`; `nm -C` shows no remaining `FakeLooper`/`WindowCallbacks`/`FakeInputQueue`/`CorePatches` C++ symbols.

### Phase 5 — delete the `mcpelauncher-gamewindow` C++ lib ✅ DONE
- Rust eglut creates the window directly (no `GameWindowManager`); EGL surface token switches from `GameWindow*` to the Rust eglut window token; `game_window_make_current`/`swap_buffers`/`get_size` move to Rust; delete the 6 gamewindow .cpp files + `include/game-window` (trimmed `game_window.h` keeps only enums) + `include/eglut` + gamepad C++.
- New `game_window.rs` (~120 lines): `create_window` (`eglutInitX11ClassInstanceName` → `eglutInit` → `eglutInitWindowSize` → `eglutInitAPIMask(EGLUT_OPENGL_ES2_BIT)` → `eglutCreateWindow("Minecraft")`); the token is `eglutGetWindowHandle()` as `*mut c_void` (doubles as `ANativeWindow*`/`GameWindow*`/FakeEGL surface). Exports `mc_get_window_token`, `mc_create_default_window`, `mc_window_show`, `fake_looper_window_poll_events`, `fake_looper_window_start/stop_text_input`, `game_window_make_current`/`swap_buffers`/`get_size`, `mc_get_window_size`, `mc_set_clipboard_text`, `mc_get_key_from_key_code`, and `mc_create_window_and_setup_graphics` (`XInitThreads` → `create_window` → seed FakeEGL → relocate GLESv2). `real_egl_get_proc_address()` `dlopen`s `libEGL.so` for the FakeEGL proc-address resolver (the FakeEGL wrapper self-deadlocks).
- C++ stripped: `jni_bridge_stub.cpp` (GameWindowManager + window helpers removed, only `g_jni_support`/`g_rust_jni_support` remain); `main_activity.h` `window` member is now `void*`, `getScreenWidth/Height` call `extern "C" mc_get_window_size`; `main_activity.cpp` `setClipboard`/`getKeyFromKeyCode` via `mc_set_clipboard_text`/`mc_get_key_from_key_code`, FilePickerFactory errors via `Log::warn`.
- Deleted: `src/manifest_libs/gamewindow/` (18 files), `include/game-window/{game_window_manager.h,game_window_error_handler.h}`, `include/eglut/` (eglut.h, eglut_x11.h). `cpp-bridge-sys/build.rs` gamewindow `cc::Build` block + `client/build.rs` `-lmcpelauncher-gamewindow` link removed. `fake_egl_query_surface` EGL_WIDTH/HEIGHT falls back through `mc_get_window_size` to keep it referenced.
- **Verify (passing)**: `cargo build -p client`; `cargo test -p client` (26/26); `nm -C` zero gamewindow symbols; boots to main menu with rendering (`eglSwapBuffers ok`). **Header-edit gotcha:** hash-based incremental does NOT track `.h` includes — after editing `main_activity.h` etc., force `cargo clean -p cpp-bridge-sys` or the C++ still runs the old inline code.

## Done when

- `nm -C target/debug/client` shows no `FakeLooper`, `WindowCallbacks`, `FakeInputQueue`, or `CorePatches` C++ symbols. ✅ (Phase 4 complete)
- `cargo test -p client` passes (per-phase unit tests). ✅ (26/26)
- Game boots to main menu with keyboard/mouse/gamepad working after each phase. ✅
- `mcpelauncher-client-jni` static lib reduced to the JNI/VM core — `mcpelauncher-gamewindow` removed entirely in Phase 5.

## Depends on / used by

- Depends on: Rust eglut (`crates/client/src/eglut/`) for window callbacks; `jni_support.rs` for `jni_support_send_*`/`on_window_created`/`set_looper_running`; `text_input_handler.rs` for `text_handler_*`; `rust_bridge.rs` for `window_callbacks_map_*` + `core_patches_install_impl`; `gamepad/window.rs` (Rust joystick) for gamepad events.
- Used by: the game via `libandroid.so` looper/input hooks and the `game_window_*` symbols; startup via `mc_setup_android_hooks`/`mc_create_window_and_setup_graphics`.
- After Phase 4, the remaining compiled C++ is the JNI/VM core (`libjnivm` FakeJni/Baron + `jni_support.cpp`/`main_activity.cpp`/`jnivm_class_wrappers.cpp`/`jni_descriptors.cpp`/`jni_bridge_stub.cpp`/`lib_http_client*.cpp`) + live stubs (`http_client_stubs`, `fake_assetmanager`, `fake_audio`, `fake_egl`, `xal_webview_factory`, `text_input_handler` bridge).
