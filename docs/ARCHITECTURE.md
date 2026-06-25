# Architecture Overview

## Goal
Full Rust launcher for Minecraft Bedrock on Linux. C++ bridge is temporary scaffolding — every subsystem should eventually be pure Rust.

## Codebase Split

| Component | Lines | Lang | Status |
|-----------|-------|------|--------|
| Rust code (all workspace crates) | ~15,200 | Rust | 70% |
| C++ bridge/stubs compiled by build.rs | ~3,300 | C++ | scaffold |
| C++ JNI files still compiled | ~2,400 | C++ | not ported |
| Static libs compiled locally by build.rs | ~27 MB .a | C++ | all from local source |

## Rust Crates

| Crate | Role | Extern C/C++ Deps |
|-------|------|-------------------|
| **client** | Main binary. eglut, FakeEGL, GLCorePatch, CorePatches, JNI class modules | 6 extern C bridge functions; links local build.rs .a files |
| **libc-shim** | 602 Rust replacement libc symbols (FILE\*, pthreads, sockets, mmap, etc.) | `variadic.c` for variadic fns |
| **linker** | Pure Rust ELF linker (loads .so, resolves symbols, relocates) | None |
| **libjnivm-sys** | Pure Rust JNI VM (~250 function vtable for JNIEnv) | None |
| **eglut** | Pure Rust X11/EGL windowing + event loop | `libEGL.so` via dlopen, `libX11` via `x11` crate |
| **game-window** | winit/glutin abstraction | Not active — eglut path used instead |
| **util** | Base64, arg parser, file utils, logging, properties | None |
| **apkinfo** | APK/AndroidManifest.xml parsing | None |
| **simple-ipc** | Pure Rust IPC client/server over Unix sockets | None |
| **daemon-utils** | Pure Rust daemon launcher utilities | None |
| **msa-daemon-client** | Pure Rust MSA authentication daemon client | None |
| **cll-telemetry** | Pure Rust telemetry/eventing client | None |
| **common** | Shared types for launcher daemon/client protocols | None |

## Two JNI VMs Coexist

1. **libjnivm-sys VM** (Rust `jni_support.rs` + `libjnivm-sys` crate) — The **active** VM for game JNI dispatch. `main.rs:110` calls `jni_support::jni_support_start_game()` (Rust), which creates the Baron VM for `vm` operations, registers classes via `register_all_classes()` (Rust) and `register_all_jnivm_classes()` (C++), sets `(*ga).env` to the Rust JNI env, and dispatches `GameActivity_onCreate`. All game calls to `CallVoidMethod`, `FindClass`, `RegisterNatives` go through the Rust 250-function vtable.

2. **FakeJni VM** (C++ `jni_support.cpp`) — The **legacy** VM. Created first during startup. Game receives the Baron VM through `gameActivity.vm` for operations like `AttachCurrentThread`. Still needed by `FakeLooper::onGameActivityClose` for exit callback dispatch and by `jni_support.cpp` for `registerClass<T>()` registrations that keep the C++ linker happy. Game JNI dispatch was switched from Baron to the Rust VM in Phase 5.

## Key Architectural Insight

The Rust `jni_support::jni_support_start_game()` function (1,122 lines) is now the **active game startup path**, called from `main.rs:110`. The C++ `start_game_cpp()` bridge is kept for compatibility but is no longer the primary path. The C++ FakeJni VM is still needed for `FakeLooper::onGameActivityClose` callback dispatch, meaning `jni_support.cpp` and FakeLooper-dependent C++ files must remain compiled for now.

## Two Linkers Coexist

1. **Rust linker crate** (1,156 lines) — Loads `libc.so` symbols (merged C++ + Rust libc symbols), `libdl.so`, handles initial symbol registration. Called by `main.rs:36` before C++ bridge. Also loads stub libs (libEGL.so, libGLESv2.so, libfmod.so, libHttpClient.Android.so, etc.) via their DT_NEEDED dependencies.

2. **C++ bionic linker** (3.8 MB, compiled locally by build.rs) — The heavy lifter for the game library. `MinecraftUtils::loadMinecraftLib()` uses this to load `libminecraftpe.so` with full ELF relocation, DT_NEEDED resolution, and hook injection. The Rust linker can't load the game library yet.
