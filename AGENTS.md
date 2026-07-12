# AGENTS.md — MinecraftRust

Pure-Rust launcher for Minecraft Bedrock on Linux (replacing [mcpelauncher-manifest](https://github.com/minecraft-linux/mcpelauncher-manifest/)). ~8.5% Rust by total lines (17K of 200K+), but all game-facing JNI dispatch and startup orchestration is Rust. Loads to main menu.

## Build & Run

```bash
cargo build -p client
./target/debug/client -dg /path/to/extracted/minecraft
```

System deps: `libstdc++-dev`, `libpulse-dev`, `libx11-dev`, `libegl1-mesa-dev`, `libcurl4-openssl-dev`, `libssl-dev`, `libsdl2-dev`, `libudev-dev`, `libpng-dev`, `libevdev-dev`.

No `cmake`, no `make` — C++ bridge compiled via `cc::Build` in `cpp-bridge-sys`. All 13 static libs built there; `client/build.rs` only emits link directives.

**WARNING: `cargo build -p client` takes ~3 minutes on initial build, ~2.5 min on C++ changes, ~0.3s on pure Rust changes.** Do not run full builds unless necessary. After editing C++ sources, force C++ recompilation with:
```bash
cargo clean -p cpp-bridge-sys
cargo build -p client
```

## Workspace (15 crates)

| Crate | Role |
|-------|------|
| **client** | Sole binary — eglut, FakeEGL, CorePatches, JNI, event dispatch |
| **cpp-bridge-sys** | C++ cc::Build compilation (13 static libs) — extracted from client/build.rs so linker-only changes don't re-archive C++ |
| **libc-shim** | 602 pure Rust libc replacements (FILE*, pthreads, sockets, mmap) |
| **linker** | Pure Rust ELF linker (stub libs only — game lib still uses C++ bionic linker) |
| **libjnivm-sys** | Pure Rust JNI VM (~250 fn JNIEnv vtable) |
| **eglut** (in `client/src/`) | Pure Rust X11/EGL windowing — active path; `game-window` crate (winit/glutin) is NOT active |
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
5. Load core libs, stub libs via Rust linker + C++ bionic linker
6. Android hooks (FakeLooper, FakeAssetManager, FakeInputQueue) — Rust hooks registered
7. Create X11 window + GLES2
8. Load `libminecraftpe.so` via C++ bionic linker
9. Both JNI VMs created, classes + natives registered on both
10. `jni_support_start_game` (Rust) calls `GameActivity_onCreate` via Baron bridge → game thread starts

**Two linkers:**
- **Rust linker** (`linker/`) — loads libc, libdl, stub libs
- **C++ bionic linker** (37 files, compiled by build.rs) — loads `libminecraftpe.so` with full ELF relocation

**Key EGL fix** (`rust_bridge.rs:940`): Real EGL context + surface created on the game thread (not main), avoiding Mesa X11 thread affinity `EGL_BAD_ACCESS`. Per-thread surfaces stored in TLS.

## Config

CLI args: `-dg` (game dir, required), `-dd` (data dir), `-dc` (cache dir). Defaults: XDG (`~/.local/share/mcpelauncher`, `~/.cache/mcpelauncher`).

Runtime files: `runtime/lib/x86_64/libsqliteX.so`, `runtime/gamecontrollerdb/gamecontrollerdb.txt` — searched via `DEV_EXTRA_PATHS` relative to project root.

## Status & Known Issues

- Game loads to main menu, mouse/keyboard work
- No CI, no tests, no formatter/linter config — `cargo build -p client` is the only check
- Rust edition 2021, resolver "2"
- **XAL ECDSA key cache corruption**: delete `~/.local/share/mcpelauncher/xal/` and `~/.local/MinecraftLauncher/xal/` if auth fails. Look for files containing `"Serialized to SharedPreferences"`
- CorePatches vtable warning (`_ZTV21AppPlatform_android23`) — non-fatal
- Missing assets (`subdirs.txt`, `particles.brarchive`) — non-fatal
- GatheringServiceRequest 404s on `/api/v1.0/config/public`, `/api/v1.0/access`

## Docs (read these)

All in `docs/`:
- `ARCHITECTURE.md` — crate deps, two-VM/two-linker coexistence
- `STARTUP_FLOW.md` — 21-step annotated sequence
- `CXX_BRIDGE.md` — all ~154 extern "C" FFI symbols
- `JNI_VM.md` — libjnivm-sys vs FakeJni/Baron details
- `PORTING_PROGRESS.md` — per-file status for JNI + static libs
- `STATIC_LIBS.md` — 13 `cc::Build` targets, line counts, dep graph

## Porting (if adding Rust code)

| To port | Where | Depends on |
|---------|-------|------------|
| JNI classes (7 files) | `crates/client/src/jni/` | `main_activity.cpp` → `store.cpp` → rest; all 57 MainActivity methods ported to Rust (`main_activity.rs`); 9 wrapper classes ported (`jnivm_class_wrappers.rs`); C++ files still linked due to FakeJni registration deps in `jni_support.cpp` |
| FakeLooper remaining | `fake_looper.rs` vs `fake_looper_stub.cpp` | window callbacks |
| Game window | eglut vs `crates/game-window/` | eglut works, winit path inactive |
| IPC/Telemetry client | `crates/simple-ipc`, `daemon-utils`, `msa-daemon-client`, `cll-telemetry` | Rust versions exist; C++ bridge still active |
