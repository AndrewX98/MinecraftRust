# AGENTS.md — MinecraftRust

Pure-Rust launcher for Minecraft Bedrock on Linux (replacing [mcpelauncher-manifest](https://github.com/minecraft-linux/mcpelauncher-manifest/)). All game-facing JNI dispatch and startup orchestration is Rust. Loads to main menu.

## Build & Run

```bash
cargo build -p client
./target/debug/client -dg /home/andrew/.local/MinecraftLauncher/extracted/1.26.20/
```

Run with Rust linker logs:
```bash
RUST_LOG=linker=info ./target/debug/client -dg /home/andrew/.local/MinecraftLauncher/extracted/1.26.20/
```

System deps: `libstdc++-dev`, `libpulse-dev`, `libx11-dev`, `libegl1-mesa-dev`, `libcurl4-openssl-dev`, `libssl-dev`, `libsdl2-dev`, `libudev-dev`, `libpng-dev`, `libevdev-dev`.

No `cmake`, no `make` — C++ bridge compiled via `cc::Build` in `cpp-bridge-sys`. The single static lib left is `mcpelauncher-client-jni` (`mcpelauncher-gamewindow` was **deleted in Phase 5** — window owned by Rust eglut, see `crate::game_window`; `mcpelauncher-client-bridge` was **deleted in Phase 10** — `capi.cpp` ported to Rust `client/capi.rs`; `mcpelauncher-core` was **fully ported to Rust** (Phases 6–10 + nightly `c_variadic`) — its last two files (`android_log_varargs.cpp` → `client/android_log_hook.rs`, `jnivm_mod_api.cpp` → `client/mod_api.rs`) are gone; `mcpelauncher-cll-telemetry` was **fully ported to Rust** — `crates/cll-telemetry/` wired via `client/cll_telemetry.rs`); `client/build.rs` only emits link directives. The IPC/auth chain (`simpleipc`, `daemon-client-utils`, `msa-daemon-client`) is **ported to Rust** (`crates/simple-ipc/`, `crates/daemon-utils/`, `crates/msa-daemon-client/`) — see `docs/PORT_SIMPLEIPC.md`. `build.rs` now performs **hash-based incremental compilation**: editing a single `.cpp` file rebuilds only that file in ~2s (but a `.h` include is NOT hashed — after header edits, force with `cargo clean -p cpp-bridge-sys`).

- **Initial build:** ~3 min (all C++ files compiled)
- **Single C++ file change:** ~2s (hash-based incremental)
- **Header/C++ change:** hash tracked — cpp-bridge-sys only rebuilds changed files, no full rebuild from scratch
- **Pure Rust change:** ~0.3s

After mass C++ or header changes, force full recompilation:
```bash
cargo clean -p cpp-bridge-sys
cargo build -p client
```

## Workspace (15 crates)

| Crate | Role |
|-------|------|
| **client** | Sole binary — eglut, FakeEGL, CorePatches, JNI, event dispatch |
| **cpp-bridge-sys** | C++ cc::Build compilation (2 static libs) — extracted from client/build.rs so linker-only changes don't re-archive C++ |
| **libc-shim** | 602 pure Rust libc replacements (FILE*, pthreads, sockets, mmap) |
| **linker** | Pure Rust ELF linker — the **only** loader (C++ bionic linker deleted in Phase 6) |
| **libjnivm-sys** | Pure Rust JNI VM (~250 fn JNIEnv vtable) |
| **eglut** (in `client/src/`) | Pure Rust X11/EGL windowing — active path (the `game-window` winit/glutin crate was removed) |
| others | util, apkinfo, axml-parser, simple-ipc, daemon-utils, msa-daemon-client, cll-telemetry, common, minecraft-imported-symbols |

## Architecture (must-know)

**Two JNI VMs coexist:**
- **Rust libjnivm-sys** — active for class creation, native registration, network status dispatch, **env switch done** (`(*ga).env` = `get_env()`)
- **C++ FakeJni/Baron** — game caches this VM's `vm`; still needed for FakeLooper callback dispatch. Dead code: `jni_descriptors.cpp`, `main_activity.cpp`, `jnivm_class_wrappers.cpp` still linked because `jni_support.cpp` references FakeJni registrations

Game entrypoint: `crates/client/src/jni_support.rs:493` (`jni_support_start_game`). The C++ `start_game_cpp()` bridge is no longer the primary path.

Startup (21 steps, detailed in `docs/STARTUP_FLOW.md`):
1. env_logger init
2. C++ path setup
3. Init version
4. Merge C+++Rust libc symbols → register with Rust linker
5. Load core libs, stub libs via Rust linker
6. Android hooks (FakeLooper, FakeAssetManager, FakeInputQueue) — Rust hooks registered
7. Create X11 window + GLES2
8. Load `libminecraftpe.so` via Rust linker (bionic linker deleted in Phase 6)
9. Both JNI VMs created, classes + natives registered on both
10. `jni_support_start_game` (Rust) calls `GameActivity_onCreate` via Baron bridge → game thread starts

**Single linker (Rust):**
- **Rust linker** (`linker/`) — loads libc, libdl, stub libs, and `libminecraftpe.so` with full ELF relocation; exports the `mcpelauncher_dispatch_*` surface natively

**Key EGL fix** (`rust_bridge.rs:940`): Real EGL context + surface created on the game thread (not main), avoiding Mesa X11 thread affinity `EGL_BAD_ACCESS`. Per-thread surfaces stored in TLS.

## Config

CLI args: `-dg` (game dir, required), `-dd` (data dir), `-dc` (cache dir). Defaults: XDG (`~/.local/share/mcpelauncher`, `~/.cache/mcpelauncher`).

Runtime files: `runtime/lib/x86_64/libsqliteX.so`, `runtime/gamecontrollerdb/gamecontrollerdb.txt` — searched via `DEV_EXTRA_PATHS` relative to project root.

## Status & Known Issues

- Game loads to main menu, mouse/keyboard work
- No CI, no tests, no formatter/linter config — `cargo build -p client` is the only check
- Rust edition 2021, resolver "2"
- **Sign-in is offline-only (by design)**: the Rust chain (`simple-ipc`/`daemon-utils`/`msa-daemon-client` → real `msa-daemon` binary) is wired and E2E-verified (`daemon_e2e.rs`), but the game never invokes it. `rust_bridge.rs:272-276` patches `XalInitialize → S_OK` (real XAL crashes with stubbed libHttpClient) and the XBL bootstrap natives are missing in the Rust port: `MainActivity.nativeInitializeXboxLive`, `MainActivity.nativeInitializeLibHttpClient`, `WebView.urlOperationSucceeded`. Game runs as PlayFab guest (MCToken has `mc-realms-button-no-msa`); clicking sign-in shows "Error Code! Mooshroom" because `Interop.invokeMSA` is never reached. Re-enabling login requires real HTTP+XAL work (see `PORT_MSA_DAEMON_CLIENT.md`).
- **XAL ECDSA key cache corruption**: delete `~/.local/share/mcpelauncher/xal/` and `~/.local/MinecraftLauncher/xal/` if auth fails. Look for files containing `"Serialized to SharedPreferences"`
- CorePatches vtable warning (`_ZTV21AppPlatform_android23`) — non-fatal
- Missing assets (`subdirs.txt`, `particles.brarchive`) — non-fatal
- GatheringServiceRequest 404s on `/api/v1.0/config/public`, `/api/v1.0/access`

## Docs (read these)

All in `docs/`:
- `ARCHITECTURE.md` — crate deps, two-VM coexistence, single Rust linker
- `STARTUP_FLOW.md` — 21-step annotated sequence
- `CXX_BRIDGE.md` — all ~154 extern "C" FFI symbols
- `JNI_VM.md` — libjnivm-sys vs FakeJni/Baron details
- `PORTING_PROGRESS.md` — per-file status for JNI + static libs
- `STATIC_LIBS.md` — 2 `cc::Build` targets, line counts, dep graph
- `PORT_FAKE_LOOPER.md` — phased plan (FakeLooper + WindowCallbacks + FakeInputQueue + CorePatches → Rust)
- `PORT_JNI_SUPPORT.md` — 5-phase plan to delete the FakeJni/Baron chain (run game on the Rust `libjnivm-sys` VM, ~5,500 lines)
- `ROADMAP_TO_FULL_RUST.md` — milestones 1–6 to zero C++ compilation (jni_support → http → dead stubs → live shims → variadic.c → drop cc)
- `PORT_MACOS.md` — phased plan for macOS support (Cocoa/GLFW windowing, kqueue shim, CI matrix, testing from Linux)

## Porting (if adding Rust code)

| To port | Where | Depends on |
|---------|-------|------------|
| JNI classes (7 files) | `crates/client/src/jni/` | `main_activity.cpp` → `store.cpp` → rest; all 57 MainActivity methods ported to Rust (`main_activity.rs`); 9 wrapper classes ported (`jnivm_class_wrappers.rs`); C++ files still linked due to FakeJni registration deps in `jni_support.cpp` (see `PORT_JNI_SUPPORT.md`) |
| FakeLooper/WindowCallbacks/FakeInputQueue/CorePatches | **done** — `fake_looper.rs` (thread_local `CURRENT`, hooks), `window_callbacks.rs`, `fake_inputqueue.rs`, `core_patches.rs`; `fake_looper_stub.cpp`/`window_callbacks_stub.cpp`/`core_patches_stub.cpp`/`fake_inputqueue_stub.cpp` + headers deleted (see `PORT_FAKE_LOOPER.md`, 5-phase plan) |
| Game window | **done** — `crate::game_window.rs` (Phase 5) creates the X11/EGL window directly via eglut and owns the window token + `game_window_*`/`mc_*`/`fake_looper_window_*` helpers | `mcpelauncher-gamewindow` C++ lib, `include/game-window/{game_window_manager.h,game_window_error_handler.h}`, `include/eglut/`, `manifest_libs/gamewindow/` all deleted; `client/build.rs` no longer links it |
| IPC/Telemetry client | `crates/simple-ipc`, `daemon-utils`, `msa-daemon-client`, `cll-telemetry` | simple-ipc/daemon-utils/msa-daemon-client **wired** (C++ chain removed — see PORT docs); cll-telemetry **wired** (`client/cll_telemetry.rs`, C++ lib deleted — see PORT_CLL_TELEMETRY.md) |
