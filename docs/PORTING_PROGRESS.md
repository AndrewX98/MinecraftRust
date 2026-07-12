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
| `bionic linker` | Full ELF dynamic linker | Local `.a` via `cc::Build` |
| `mcpelauncher-core` | Game loading, hooks, patching, mod loader | Local `.a` via `cc::Build` |
| `game-window` | X11/EGL window, input handling | Local `.a` via `cc::Build` |
| `linux-gamepad` | evdev joystick + SDL mappings | Local `.a` via `cc::Build` |
| `msa-daemon-client` | Microsoft Account auth | Local `.a` via `cc::Build` |
| `simpleipc` | Unix IPC + RPC framework | Local `.a` via `cc::Build` |
| `cll-telemetry` | Telemetry collection + upload | Local `.a` via `cc::Build` |
| `mcpelauncher-common` | Path resolution, OpenSSL safety | Local `.a` via `cc::Build` |
| `daemon-client-utils` | Daemon forking/inotify | Local `.a` via `cc::Build` |
| `file-util` | POSIX file operations | Local `.a` via `cc::Build` |
| `logger` | printf-style logging | Local `.a` via `cc::Build` |

## FakeLooper Porting

The FakeLooper implementation has been incrementally ported to Rust across 3 phases:

| Phase | C++ → Rust | Status |
|-------|-----------|--------|
| 1 | 6 hybris hook lambdas (`mc_register_android_hook` calls → Rust `mc_register_fake_looper_hooks`) | ✅ |
| 2 | `addFd`, `attachInputQueue`, `pollAll` → `fake_looper.rs` | ✅ |
| 3 | `prepare()` → `fake_looper.rs:120` | ✅ |

The C++ `fake_looper_stub.cpp` retains FakeLooper class state (`jniSupport`, `rustJniSupport`, `pendingWindow`), `initializeWindow()`, and FFI helpers called by the Rust hooks. The top-level Android native function hooks (`ALooper_prepare`, `ALooper_addFd`, `ALooper_pollAll`, `AInputQueue_attachLooper`, `ANativeActivity_finish`) are all Rust functions registered via hybris.

## Critical Path to Pure Rust

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
- FakeJni is still linked and used for `FakeLooper::onGameActivityClose` (exit callback) and any C++ JNI stubs that remain

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
| `window_callbacks_stub.cpp` | 713 | Key mapping, gamepad, pointer lock → Rust `rust_bridge.rs` |
| `jnivm_class_wrappers.cpp` | 648 | Registers 10 Java classes with libjnivm-sys (coexists with Rust `jnivm_class_wrappers.rs`) |
| `http_client_stubs.cpp` | 441 | Stub HTTP client for XAL |
| `jni_bridge_stub.cpp` | 375 | Android hooks, window creation, game loading, C++ wrappers for Rust `jni_support_start_game_with_baron` (FakeJni, PathHelper, XboxLiveHelper FFI) |
| `text_input_handler_stub.cpp` | 233 | Text input state management |
| `fake_assetmanager_stub.cpp` | 214 | Asset manager for game resource loading |
| `fake_looper_stub.cpp` | 152 | FakeLooper class state + FFI helpers (hooks are Rust `fake_looper.rs`) |
| `fake_egl_stub.cpp` | 161 | Delegates to Rust eglut |
| `core_patches_stub.cpp` | 141 | Vtable patching, cursor lock |

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
| Game window | ~30% | 100% (eglut done, gamepad remaining) |
| JNI classes | ~85% | 100% (57/57 MainActivity methods done, store/audio/http/websocket/xbox ported; http/websocket callback wiring remaining) |
| mcpelauncher-core | ~0% | 100% (game loading, hooks, patching, mod loading) |
| Startup orchestration | ~60% | 100% |
| FakeLooper | ~70% | 100% |
| Build system | 100% | 100% (no cmake) |
| IPC/Telemetry | ~0% | 100% (Rust crates exist, C++ bridge still active) |

(Raw line counts: Rust 17K, C++ 84K, C Headers 76K, C 23K — ~8.5% Rust by total code. The percentages above are per-component estimates of critical-path functionality ported so far.)
