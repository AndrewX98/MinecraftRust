# Porting Progress

## Legend
- ✅ Rust replacement active, C++ file removed from build
- 🟡 Partial (registration removed, file stays compiled)
- 🔴 Blocked (can't remove without breaking game)
- ⏳ Not started

## JNI Files (`mcpelauncher-client/src/jni/*.cpp`)

20 JNI C++ files are excluded from build (via excluded_jni set). 5 remain compiled.

### Already Ported (20 files — excluded from build)

| File | Rust Module | Status |
|------|-------------|--------|
| `locale.cpp` | `jni_support.rs::locale` | ✅ |
| `uuid.cpp` | `uuid_stub.cpp` + Rust in `jni_support.rs` | 🟡 (stub, Rust registration exists) |
| `cert_manager.cpp` | `jni_support.rs::certificate` | ✅ |
| `ecdsa.cpp` | `jni_support.rs::ecdsa_impl` | ✅ |
| `jbase64.cpp` | stub | ✅ |
| `arrays.cpp` | stub | ✅ |
| `asset_manager.cpp` | stub | ✅ |
| `package_source.cpp` | stub | ✅ |
| `securerandom.cpp` | stub | ✅ |
| `signature.cpp` | stub | ✅ |
| `accounts.cpp` | stub | ✅ |
| `playfab.cpp` | stub | ✅ |
| `fmod.cpp` | stub | ✅ |
| `webview.cpp` | stub | ✅ |
| `shahasher.cpp` | stub | ✅ |
| `http_stub.cpp` | n/a (dead code) | ✅ |
| `store.cpp` | `store.rs` + `store_stub.cpp` (stub) | ✅ |
| `pulseaudio.cpp` | `pulseaudio_stub.cpp` + Rust `audio.rs` | ✅ |
| `sdl3audio.cpp` | `sdl3audio_stub.cpp` + Rust `audio.rs` | ✅ |
| `http_client.rs` | new Rust module (`lib_http_client.cpp` still compiled) | 🟡 |
| `websocket.rs` | new Rust module (`lib_http_client_websocket.cpp` still compiled) | 🟡 |
| `xbox_live.cpp` | `jni/xbox_live.rs` + `xbox_live_stub.cpp` (FakeJni bodies for descriptors) | ✅ |

### Still Compiled (5 files)

| File | Lines | Role | Status | Depends On |
|------|-------|------|--------|------------|
| `jni_support.cpp` | 673 | FakeJni startup orchestration, class registration | 🟡 | — |
| `main_activity.cpp` | 539 | 40+ Android API methods (all ported to Rust `main_activity.rs`) | 🟡 | `jni_support.cpp` FakeJni `registerClass<MainActivity>()` call |
| `lib_http_client.cpp` | 290 | Curl-based HTTP requests | ⏳ | — |
| `lib_http_client_websocket.cpp` | 224 | Curl-based WebSocket | ⏳ | — |
| `jni_descriptors.cpp` | 315 | FakeJni class descriptors | 🟡 | Dies with `jni_support.cpp` port — `registerMinecraftNatives()` calls `MainActivity::getDescriptor()` etc. |

## Static Libraries (all compiled locally via build.rs, no cmake prebuilts)

All 11 former cmake-built static libs are now compiled locally by `cc::Build` instances in `build.rs`. None link against `mcpelauncher-manifest/` prebuilts.

| Library | Role | Status |
|---------|------|--------|
| `bionic linker` | ~~Full ELF dynamic linker~~ | **DELETED Phase 6** — Rust `crates/linker/` is the only loader |
| `mcpelauncher-core` | Game loading, hooks, patching, mod loader | **100% PORTED / DELETED** (2026-08-05) — last 2 files (`android_log_varargs.cpp`, `jnivm_mod_api.cpp`) ported to Rust via nightly `c_variadic`; `jnivm_register_method` stubbed → false |
| `game-window` | X11/EGL window, input handling | **DELETED Phase 5** (2026-08-05) — window owned by Rust `crate::game_window.rs` + eglut; gamepad C++ ported to `gamepad/joystick.rs` |
| `linux-gamepad` | evdev joystick + SDL mappings | Local `.a` via `cc::Build` |
| `msa-daemon-client` | Microsoft Account auth | Local `.a` via `cc::Build` |
| `simpleipc` | Unix IPC + RPC framework | Local `.a` via `cc::Build` |
| `cll-telemetry` | Telemetry collection + upload | Local `.a` via `cc::Build` |
| `mcpelauncher-common` | Path resolution, OpenSSL safety | Local `.a` via `cc::Build` |
| `daemon-client-utils` | Daemon forking/inotify | Local `.a` via `cc::Build` |
| `file-util` | POSIX file operations | **PORTED** — Rust `util::file_util` + `file_util_*`/`env_path_util_*` FFI |
| `logger` | printf-style logging | **PORTED** — Rust `util::logger` + `logger_stub.cpp` shim |

## FakeLooper Porting

The FakeLooper implementation has been incrementally ported to Rust across 4 phases:

| Phase | C++ → Rust | Status |
|-------|-----------|--------|
| 1 | 6 hybris hook lambdas (`mc_register_android_hook` calls → Rust `mc_register_fake_looper_hooks`) | ✅ |
| 2 | `addFd`, `attachInputQueue`, `pollAll` → `fake_looper.rs` | ✅ |
| 3 | `prepare()` → `fake_looper.rs:120` | ✅ |
| 4 | full `FakeLooper` class deleted → Rust `fake_looper.rs` owns all state (thread_local `CURRENT`, prepared/text-input latches, window token, queue); `fake_looper_stub.cpp`, `fake_looper.h`, `fake_inputqueue_stub.cpp`, `manifest_headers/fake_inputqueue.h` deleted | ✅ |
| 5 | `mcpelauncher-gamewindow` C++ lib deleted → `crate::game_window.rs` creates the eglut window and owns the window token + `game_window_*`/`mc_*`/`fake_looper_window_*` helpers; `manifest_libs/gamewindow/`, `include/game-window/{game_window_manager.h,game_window_error_handler.h}`, `include/eglut/` deleted; build.rs link directives removed | ✅ |

The top-level Android native function hooks (`ALooper_prepare`, `ALooper_addFd`, `ALooper_pollAll`, `AInputQueue_attachLooper`, `ANativeActivity_finish`) are all Rust functions registered via hybris. `jni_bridge_stub.cpp` keeps process globals for the C++/Rust JniSupport, exposed via `mc_get_jni_support`/`mc_get_rust_jni_support`. The window token and its helpers live entirely in Rust `crate::game_window.rs` (`mc_get_window_token`, `mc_create_default_window`, `mc_window_show`, `fake_looper_window_poll_events`, `fake_looper_window_start/stop_text_input`, `game_window_make_current/swap_buffers/get_size`, `mc_get_window_size`, `mc_set_clipboard_text`, `mc_get_key_from_key_code`). Rust `FakeInputQueue*` identity-casts to `AInputQueue*`.

## Critical Path to Pure Rust

> Plan: `docs/PORT_JNI_SUPPORT.md` — 5 phases (registration audit → native coverage → env/vm switch → callback dispatch → delete FakeJni/Baron chain). Unlocks ~5,500 lines. Full sequence after that: `docs/ROADMAP_TO_FULL_RUST.md` (M1 jni_support → M2 http → M3 dead stubs → M4 live shims → M5 variadic.c → M6 drop cc).

```
jni_support.cpp  ──blocker──>  main_activity.cpp  ──blocker──>  jni_descriptors.cpp

Independent:  lib_http_client*.cpp (http_client.rs + websocket.rs exist, callbacks not wired)
```

The **bottleneck** is `jni_support.cpp` (673 lines). It contains:
- `registerJniClasses()` — 40+ `vm.registerClass<T>()` calls
- `registerMinecraftNatives()` — 13 native method registrations (still called during startup)
- `startGame()` — the old C++ startup path (no longer active — Rust `jni_support_start_game()` is used instead)
- `onWindowCreated/Closed/Resized`, text input, back/return key callbacks

A Rust version exists in `jni_support.rs` (1122 lines). Key functions ported:

| Function | Location | Status |
|----------|----------|--------|
| `jni_support_new()` / `jni_support_destroy()` | `jni_support.rs:198` | ✅ Active — creates libjnivm-sys VM |
| `jni_support_start_game()` | `jni_support.rs:493` | ✅ Active — `main.rs:110` calls this, not C++ |
| `jni_support_start_game_with_baron()` | `jni_support.rs:359` | ✅ Bridges to C++ FakeJni for `GameActivity_onCreate` via Baron LocalFrame |
| `jni_support_register_natives()` | `jni_support.rs:236` | ✅ Active — registers 13+ Java native classes via `jnivm_register_natives` |
| Event dispatch (`sendKeyDown`/`sendKeyUp`/`sendMotionEvent`) | `jni_support.rs:450` | ✅ Active — forwards to `GameActivityCallbacks` |

### Env Switch (Phase 5 — Complete)

`(*ga).env` now points to `get_env()` (libjnivm-sys) instead of `baron_env` (FakeJni). This means:
- All game JNI dispatch (`CallVoidMethod`, `CallStaticVoidMethod`, `FindClass`, etc.) goes through the Rust libjnivm-sys vtable
- `main_activity.rs` (57 methods) and `jnivm_class_wrappers.rs` (21 methods across 9 classes) are handling real game calls
- FakeJni is still linked and used for the `GameActivity_onCreate` bridge and any C++ JNI stubs that remain; the exit callback is `fake_looper_on_game_activity_close` (C++ helper in `jni_bridge_stub.cpp`, called from Rust `fake_looper_hook_finish`).

### C++ Global Getters/Setters (Phase 5 clean-up)

`jnivm_globals.rs` provides `#[no_mangle] extern "C"` replacements for the C++ global getter/setter functions that were previously in `jnivm_class_wrappers.cpp`:
- `jnivm_set/get_main_window`
- `jnivm_set/get_storage_dir`
- `jnivm_set/get_text_input_handler`
- `jnivm_set/get_asset_manager`
- `jnivm_set/get_stbi_load_from_memory/image_free`

These are called from Rust startup (`jni_support_start_game`) and C++ bridge code.

## Bridge Stubs (27 files, ~5,200 lines)

These will shrink automatically as the Rust ports progress. Biggest files:

| File | Lines | Bridges To |
|------|-------|------------|
| `window_callbacks_stub.cpp` | 713 | ❌ deleted (Phase 3) — full Rust `window_callbacks.rs` |
| `jnivm_class_wrappers.cpp` | 648 | Registers 10 Java classes with libjnivm-sys (coexists with Rust `jnivm_class_wrappers.rs`) |
| `http_client_stubs.cpp` | 441 | Stub HTTP client for XAL |
| `jni_bridge_stub.cpp` | 182 | FakeJni/Baron JNI support FFI wrappers + process globals (JniSupport getters); android hooks (`mc_setup_android_hooks` → `capi::setup_android_hooks`), `mc_dlsym`, `rust_load_stub`/`rust_add_symbols`/`mc_register_android_hook` + dead `jni_support_start_game_cpp`/`jni_support_get_text_input_handler` ported to Rust (Phase 12) |
| `text_input_handler_stub.cpp` | 233 | Text input state management |
| `fake_assetmanager_stub.cpp` | 214 | Asset manager for game resource loading |
| `fake_egl_stub.cpp` | 161 | Delegates to Rust eglut |
| `core_patches_stub.cpp` | 141 | ❌ deleted (Phase 2) — full Rust `core_patches.rs` |

## New Rust Files

| File | Lines | Role |
|------|-------|------|
| `crates/client/src/main_activity.rs` | ~1300 | All 57 MainActivity JNI methods (getScreenWidth, createUUID, showKeyboard, etc.) |
| `crates/client/src/jnivm_class_wrappers.rs` | ~380 | 21 methods across 9 Java classes (File, Context, Build, PackageInfo, etc.) |
| `crates/client/src/jnivm_globals.rs` | ~80 | `#[no_mangle]` extern "C" getter/setter functions for C++ global state |
| `crates/client/src/jni/store.rs` | ~367 | In-app purchase JNI stubs (replaces `store.cpp`) |
| `crates/client/src/jni/audio.rs` | ~350 | PulseAudio + SDL3 audio output JNI (replaces `pulseaudio.cpp` + `sdl3audio.cpp`) |
| `crates/client/src/jni/http_client.rs` | ~599 | HTTP client JNI (coexists with `lib_http_client.cpp`) |
| `crates/client/src/jni/websocket.rs` | ~393 | WebSocket JNI (coexists with `lib_http_client_websocket.cpp`) |
| `crates/client/src/jni/xbox_live.rs` | ~300 | XboxInterop + XboxLocalStorage JNI (replaces `xbox_live.cpp`; stub auth, always fails offline-safe) |

## Overall Estimate

| Category | Rust % | Target |
|----------|--------|--------|
| libc shim | 100% | 100% |
| JNI VM | 100% | 100% (bridge only remaining) |
| EGL | 100% | 100% |
| ELF linker (bionic) | ~30% | 100% (Rust linker crate exists, needs full relocation) |
| Game window | 100% | 100% (eglut + `game_window.rs`, gamepad ported — Phase 5) |
| JNI classes | ~85% | 100% (57/57 MainActivity methods done, store/audio/http/websocket/xbox ported; http/websocket callback wiring remaining) |
| mcpelauncher-core | 100% | 100% (deleted — ported via nightly c_variadic; jnivm_register_method stubbed) |
| Startup orchestration | ~60% | 100% |
| FakeLooper | 100% | 100% (5 phases done — Phases 1–5) |
| Build system | 100% | 100% (no cmake) |
| IPC/Telemetry | ~0% | 100% (Rust crates exist, C++ bridge still active) |

(Raw line counts: Rust 17K, C++ 84K, C Headers 76K, C 23K — ~8.5% Rust by total code. The percentages above are per-component estimates of critical-path functionality ported so far.)
