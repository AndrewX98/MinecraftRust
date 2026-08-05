# Port: JniSupport (`jni_support.cpp`) → Rust

**Goal:** Run the game end-to-end on the Rust `libjnivm-sys` VM (`ga->vm` + `ga->env` both Rust), then delete the FakeJni/Baron chain. This is the "Critical Path to Pure Rust" blocker from `docs/PORTING_PROGRESS.md`.

## What gets deleted (~5,500 lines)

| File | Lines | Role | Dies with |
|------|-------|------|-----------|
| `src/jni/jni_support.cpp` | 677 | FakeJni startup orchestration, class registration, callback dispatch | this port |
| `src/jni/main_activity.cpp` | 547 | 40+ MainActivity methods (100% ported to `main_activity.rs`) | FakeJni `registerClass<MainActivity>()` gone |
| `src/jni/jni_descriptors.cpp` | 305 | FakeJni class descriptors | registerClass gone |
| `src/jnivm_class_wrappers.cpp` | 721 | C++ class registration (redundant — Rust `jnivm_class_wrappers.rs` active) | linker deps from jni_support.cpp gone |
| `src/jni/pulseaudio_stub.cpp` + `uuid_stub.cpp` | 59 | FakeJni types | VM chain gone |
| `src/jni_bridge_stub.cpp` | 354 | shrinks to ~50 lines (FakeJni LocalFrame/attachLibrary wrappers, JniSupport globals) | VM chain gone |
| libjnivm C++ (`jnivm/*.cpp` 14, `internal/*` 6, `codegen/*` 5, `fake-jni/*` 3, `baron/*` 1) | 2,990 | C++ JNI VM (fully duplicated by `libjnivm-sys`) | VM chain gone |

**Not in scope (separate task, ~514 lines):** `src/jni/lib_http_client.cpp` (290) + `src/jni/lib_http_client_websocket.cpp` (224) — die only after Rust `jni/http_client.rs`/`websocket.rs` response callbacks are wired.

## Current state (what's already Rust)

- `jni_support.rs` (1,641 lines) — `jni_support_start_game` (line 606, the ACTIVE path) → calls `jni_support_start_game_with_baron` (line 373). Active: `register_natives`, event dispatch (`jni_support_send_key_down/up/motion_event`), `on_window_created/closed/resized`, `set_looper_running`, UUID/Locale/File/Store registrations.
- `jnivm_class_wrappers.rs` — ~9 classes registered on the Rust VM via `jnivm_find_class` + `jnivm_register_natives`.
- `libjnivm-sys` — full JavaVM vtable (`vm_funcs.rs`: `DestroyJavaVM`/`AttachCurrentThread`/`DetachCurrentThread`/`GetEnv`/`AttachCurrentThreadAsDaemon`) + JNIEnv vtable.
- `main_activity.rs`, `store.rs`, `audio.rs`, `http_client.rs`, `websocket.rs`, `xbox_live.rs`, `file_picker.rs` — JNI method bodies in Rust.

## The blocker

`jni_support_start_game_with_baron` still runs the whole startup on the **Baron/FakeJni** VM:
- `(*ga).vm = jni_support_get_java_vm_ptr(s)` (FakeJni VM) — line 448
- `(*ga).env = baron_env` (Baron LocalFrame env) — line 449
- `fake_jni_local_frame_create`/`destroy`, `fake_jni_jvm_attach_library` (libfmod/minecraftpe/PlayFab), `nativeRegisterThis` via `baron_env`

The game caches `ga->vm`/`ga->env` during `GameActivity_onCreate` and uses them for its entire life (FakeLooper dispatch, AppPlatform storage paths, UI callbacks). Known gap proving VM incompat: `jni_support.rs:417` — `getExternalStoragePath` must run via the Baron VM or paths break.

## Phases (each ends with a verification gate)

### Phase 1 — Class registration coverage audit (mechanical)

- Diff C++ `registerJniClasses()` (`jni_support.cpp:190-239`, ~40 classes: File, ClassLoader, Locale, BuildVersion, PackageInfo, PackageManager, Context, ContextWrapper, HardwareInfo, Activity, NativeActivity, NetworkMonitor, MainActivity, AccountManager, Account, StoreListener, NativeStoreListener, Store, StoreFactory, ExtraLicenseResponseData, XboxInterop, XboxLocalStorage, HttpClientRequest/Response/WebSocket, PackageSource(+Listener/Factory), ShaHasher, SecureRandom, WebView, BrowserLaunchActivity, JBase64, Arrays, Signature, PublicKey, Product, Purchase…) against Rust registrations.
- Add missing `ClassDef`/method tables to Rust. **Aggressive stubs OK** — the game only touches a handful at runtime (main menu = MainActivity, Context, NetworkMonitor, BuildVersion, HardwareInformation, JellyBeanDeviceManager, File, UUID, Locale, Store…); the rest are lookup-only.
- **Gate:** `cargo build -p client`, `cargo test -p client` (26), boot unchanged. No behavior change (C++ still active — dual-VM).

### Phase 2 — `registerMinecraftNatives` coverage

- Diff C++ (`jni_support.cpp:252-289`: MainActivity 22 natives, NetworkMonitor, NativeStoreListener, JellyBeanDeviceManager, HttpClientRequest, HttpClientWebSocket, WebView, BrowserLaunchActivity, NativeInputStream, NativeOutputStream, NetworkObserver, PlayIntegrity) vs Rust `jni_support_register_natives`.
- Register every native on the Rust VM with the exact signature string + correct `fnPtr`.
- **Gate:** boot with `RUST_LOG=...` — every `RegisterNatives` logs success; no `FindClass`/`GetMethodID` nulls in the log.

### Phase 3 — the env/vm switch ⚠️ (risk phase)

In `jni_support_start_game_with_baron` (and its caller), switch the game onto the Rust VM:
- `(*ga).vm = jnivm_get_vm()` (libjnivm-sys JavaVM); `(*ga).env = get_env()`.
- Drop `fake_jni_local_frame_create/destroy` + `set_baron_env`; use `get_env()` everywhere.
- Replace `fake_jni_jvm_attach_library(...)` — with the Rust VM, native methods resolve via `jnivm_register_natives` (already done in Phases 1–2) and the game's own `JNI_OnLoad` `RegisterNatives`; attachment may become a no-op — verify.
- `nativeRegisterThis`: `baron_env` FindClass/GetMethodID/CallVoidMethod → `get_env()` (libjnivm-sys).
- **Fix the known gap:** storage dir + `getExternalStoragePath` must resolve through the Rust VM (AppPlatform `CurrentFileStoragePath is now '…'` must not be empty).
- **Gate:** boot to main menu; storage path log line non-empty; mouse/keyboard; rendering. This is the step that decides whether the whole port is viable — if libjnivm-sys handles the game's `ga->vm` traffic, everything else is cleanup.

### Phase 4 — Port the callback dispatchers (they already run on Rust env)

Verify/replace the remaining C++ dispatchers with libjnivm-sys calls on MainActivity:
- `onWindowCreated` (`jni_support.cpp:592`) — already Rust (`jni_support_on_window_created`); ensure the C++ `activity->window` member is fed from Rust (already done in Phase 5 of the gamewindow port).
- `onWindowClosed`/`onWindowResized` — already Rust (`nativeShutdown`/`nativeResize` via `jni_call!`); confirm the C++ versions (`jni_support.cpp:608-619`) are no longer on the dispatch path.
- Text input (`onSetTextboxText`/`onCaretPosition`/`onReturnKeyPressed`/`onBackPressed`/`setLastChar`, `jni_support.cpp:621-666`) → Rust on `get_env()`; `TextInputHandler` already Rust (`text_input_handler.rs`).
- `setGameControllerConnected` (`jni_support.cpp:668`) → Rust `JellyBeanDeviceManager.onInputDeviceAdded/RemovedNative`.
- `sendUri`/`importFile` (`jni_support.cpp:479-540`) — Baron `fileOpen->invoke` file-picker flow → Rust via `jni/file_picker.rs` + MainActivity natives.
- `stopGame`/`waitForGameExit`/`requestExitGame` — already Rust (`set_looper_running` + `game_state` condvar); verify.
- **Gate:** boot; type in a text box; back/return keys; gamepad connect/disconnect; rendering.

### Phase 5 — Delete the FakeJni/Baron chain

- Replace the C++ `JniSupport` object (`jni_support_new_cpp`/`jni_support_init_activity`/`jni_support_destroy_cpp`, `jni_bridge_stub.cpp:326`) with the Rust `JniSupport` struct (already mirrored in `jni_support.rs`); remove `registerJniClasses()` ctor work.
- Delete `main_activity.cpp`, `jni_descriptors.cpp`, `jnivm_class_wrappers.cpp`, `pulseaudio_stub.cpp`, `uuid_stub.cpp`.
- Strip `jni_bridge_stub.cpp` to ~50 lines: process globals (`g_jni_support`, `g_rust_jni_support`) + hybris hook registration + `JNI_OnLoad`-era attachment. Remove `fake_jni_local_frame_*`, `fake_jni_jvm_attach_library`, Baron/FakeJni includes.
- Delete libjnivm C++ from `cpp-bridge-sys/build.rs` (lines 337-359: all `jnivm/*.cpp`, `fake-jni/*`, `baron/jvm.cpp`) + drop the `libjnivm` include path + `client/build.rs` link directives. Remove the `client/include/libjnivm/` tree.
- **Gate:** `cargo build -p client`; `cargo test -p client`; `nm -C target/debug/client` shows zero `FakeJni`/`Baron`/`JniSupport::`/`MainActivity::` C++ symbols; boot to main menu with keyboard/mouse/render.

### Follow-up (separate) — HTTP/WebSocket

Wire Rust `jni/http_client.rs` + `jni/websocket.rs` response callbacks (currently "Partial — callbacks not wired"), then delete `lib_http_client.cpp` (290) + `lib_http_client_websocket.cpp` (224).

## Risks / unknowns

1. **Game `ga->vm` traffic** — the game calls `AttachCurrentThread`/`GetEnv`/`findClass` etc. through the cached VM on many threads (game thread, JNI_OnLoad threads, XSAPI/XBL worker threads). `libjnivm-sys` has the vtable, but `GetEnv` must return a working per-thread env and `AttachCurrentThread` must initialize TLS on each game thread. Phase 3 is where this gets proven.
2. **`getExternalStoragePath`** — a proven FakeJni-vs-Rust incompatibility (`jni_support.rs:417`). Must resolve via the Rust VM or storage paths break.
3. **Method-signature drift** — the game resolves methods by exact signature; any table typo = null method id = silent no-op. The boot-log `FindClass`/`GetMethodID` audit in Phases 1–2 is the safety net.
4. **`attachLibrary` semantics** — Baron's `attachLibrary(lib, {}, {dlopen,dlsym,dlclose})` tells the VM how to resolve natives in game libs. The Rust VM registers natives directly (`jnivm_register_natives`), so this likely becomes unnecessary — but the game's own `JNI_OnLoad` must still see a working env on the thread that runs it.
5. **Threading model** — FakeJni `LocalFrame` was created per-call in C++ and torn down after dispatch. libjnivm-sys env is TLS; ensure no dangling env use after frame teardown (the C++ `startGame` did all `onStart`/`onNativeWindowCreated` inside one frame).

## Preconditions

- Read `docs/JNI_VM.md` (two-VM coexistence) + `docs/ARCHITECTURE.md`.
- Add a startup registration-coverage report: log every `FindClass`/`GetMethodID` miss with the class/method name so Phase 1–2 gaps are visible in one boot log.
- Build/test conventions: `cargo build -p client`, `cargo test -p client --tests` (26), boot gate `timeout 45 ./target/debug/client -dg /home/andrew/.local/MinecraftLauncher/extracted/1.26.20/`.
