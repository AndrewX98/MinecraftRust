# Porting Progress

## Legend
- ✅ Rust replacement active, C++ file removed from build
- 🟡 Partial (registration removed, file stays compiled)
- 🔴 Blocked (can't remove without breaking game)
- ⏳ Not started

## JNI Files (`mcpelauncher-client/src/jni/*.cpp`)

There are 25 JNI C++ files in total. 15 are excluded from build, 10 remain.

### Already Ported (15 files — excluded from build)

| File | Rust Module | Status |
|------|-------------|--------|
| `locale.cpp` | `jni_support.rs::locale` | ✅ |
| `uuid.cpp` | `jni_support.rs::uuid` | 🟡 (C++ file stays — used by `main_activity.cpp`) |
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

### Still Compiled (10 files)

| File | Lines | Role | Status | Depends On |
|------|-------|------|--------|------------|
| `jni_support.cpp` | 673 | FakeJni startup orchestration, class registration | ⏳ | — |
| `main_activity.cpp` | 539 | 40+ Android API methods | ⏳ | `jni_support.cpp` port |
| `store.cpp` | 96 | In-app purchase stubs | 🔴 | Full startup orchestration port |
| `xbox_live.cpp` | 128 | MSA sign-in, XBL auth | ⏳ | — |
| `lib_http_client.cpp` | 290 | Curl-based HTTP requests | ⏳ | — |
| `lib_http_client_websocket.cpp` | 224 | Curl-based WebSocket | ⏳ | — |
| `pulseaudio.cpp` | 71 | PulseAudio output | ⏳ | — |
| `sdl3audio.cpp` | 56 | SDL3 audio output | ⏳ | — |
| `uuid.cpp` | 30 | UUID generation | 🟡 | Rust registers JNI (`jni_support.rs:641`), C++ stays because `main_activity.cpp` calls `UUID::randomUUID()` as a C++ function directly (not through JNI dispatch) |
| `jni_descriptors.cpp` | 315 | FakeJni class descriptors | ⏳ | Dies with `jni_support.cpp` port |

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
jni_support.cpp  ──blocker──>  main_activity.cpp  ──blocker──>  store.cpp
       │
       └──>  jni_descriptors.cpp  (dies when jni_support.cpp ported)
       
Independent:  xbox_live.cpp, lib_http_client*.cpp, pulseaudio, sdl3audio
```

The **bottleneck** is `jni_support.cpp` (673 lines). It contains:
- `registerJniClasses()` — 40+ `vm.registerClass<T>()` calls
- `registerMinecraftNatives()` — 13 native method registrations
- `startGame()` — the active startup path that creates `MainActivity` and calls `GameActivity_onCreate`
- `onWindowCreated/Closed/Resized`, text input, back/return key callbacks

A Rust version exists in `jni_support.rs` (1122 lines). Key functions ported:

| Function | Location | Status |
|----------|----------|--------|
| `jni_support_new()` / `jni_support_destroy()` | `jni_support.rs:198` | ✅ Active — creates libjnivm-sys VM |
| `jni_support_start_game()` | `jni_support.rs:493` | ✅ Active — `main.rs:110` calls this, not C++ |
| `jni_support_start_game_with_baron()` | `jni_support.rs:359` | ✅ Active — orchestrates Baron LocalFrame, calls `GameActivity_onCreate`, dispatches `onStart`/`onNativeWindowCreated` |
| `jni_support_register_natives()` | `jni_support.rs:236` | ✅ Active — registers 13+ Java native classes via `jnivm_register_natives` |
| Event dispatch (`sendKeyDown`/`sendKeyUp`/`sendMotionEvent`) | `jni_support.rs:450` | ✅ Active — forwards to `GameActivityCallbacks` |

The Rust `jni_support_start_game_with_baron()` bridges to the C++ FakeJni VM for Baron JNI operations (the game caches Baron's `vm`/`env` pointers). The C++ `jni_support_start_game_cpp` path (in `jni_bridge_stub.cpp`) is fallback only — never called from `main.rs`.

## Bridge Stubs (27 files, ~5,200 lines)

These will shrink automatically as the Rust ports progress. Biggest files:

| File | Lines | Bridges To |
|------|-------|------------|
| `window_callbacks_stub.cpp` | 713 | Key mapping, gamepad, pointer lock → Rust `rust_bridge.rs` |
| `jnivm_class_wrappers.cpp` | 648 | Registers 10 Java classes with libjnivm-sys |
| `http_client_stubs.cpp` | 441 | Stub HTTP client for XAL |
| `jni_bridge_stub.cpp` | 375 | Android hooks, window creation, game loading, C++ wrappers for Rust `jni_support_start_game_with_baron` (FakeJni, PathHelper, XboxLiveHelper FFI) |
| `text_input_handler_stub.cpp` | 233 | Text input state management |
| `fake_assetmanager_stub.cpp` | 214 | Asset manager for game resource loading |
| `fake_looper_stub.cpp` | 152 | FakeLooper class state + FFI helpers (hooks are Rust `fake_looper.rs`) |
| `fake_egl_stub.cpp` | 161 | Delegates to Rust eglut |
| `core_patches_stub.cpp` | 141 | Vtable patching, cursor lock |

## Overall Estimate

| Category | Rust % | C++ % |
|----------|--------|-------|
| libc shim | 100% | 0% |
| JNI VM | 100% | 0% (bridge only) |
| EGL | 100% | 0% |
| ELF linker | ~30% | ~70% |
| Game window | ~30% | ~70% |
| JNI classes | ~30% | ~70% |
| IPC/Telemetry | 0% (local C++) | 100% |
| Startup orchestration | ~60% | ~40% |
| FakeLooper | ~70% | ~30% |
| Build system | 100% (no cmake) | 0% |
| **Overall** | **~70%** | **~30%** |
