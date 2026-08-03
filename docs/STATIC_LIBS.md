# Static Libraries Analysis

All former cmake-built static libraries are now compiled locally by **9 `cc::Build` instances** in `build.rs`. No prebuilt cmake archives are linked. The C++ infrastructure is still compiled from source and linked as `.a` files, but the compilation is fully within `MinecraftRust/`.

## Libraries Compiled by build.rs

| Library | Role | Objects | Complexity |
|---------|------|---------|-----------|
| `mcpelauncher-core` | Game loading, hooks, patching, mod loader | 9 objects | **LARGE** |
| `simpleipc` | Unix IPC + RPC framework | 14 objects | LARGE (skippable) |
| `cll-telemetry` | Telemetry collection + upload | 15 objects | LARGE (skippable) |
| `msa-daemon-client` | Microsoft Account auth | 2 objects | **MEDIUM** |
| `linux-gamepad` | evdev joystick + SDL mappings | 5 objects | **MEDIUM** |
| `game-window` | X11/EGL window, input handling | 7 objects | **MEDIUM** |
| `daemon-client-utils` | Daemon forking/inotify | 1 object | SMALL (skippable) |
| `mcpelauncher-client-bridge` | Rust ↔ C++ bridge (capi.cpp) | 1 object | SMALL |
| `mcpelauncher-client-jni` | JNI stubs, class wrappers, libjnivm C++ | ~35+ objects | **LARGE** |

## Detailed Analysis

### `mcpelauncher-core` (4.2 MB) — CRITICAL

Central orchestration hub. **9 source files:**

| File | Lines | Role |
|------|-------|------|
| `minecraft_utils.cpp` | 1007 | **The most important file.** `getLibCSymbols()`, `loadLibM()`, `loadFMod()`, `setupHybris()`, `setupApi()`, `loadMinecraftLib()` (master game loader), `setupGLES2Symbols()` |
| `hook.cpp` | 265 | `HookManager`. ELF relocation table manipulation for function hooking |
| `mod_loader.cpp` | 184 | `ModLoader`. Loads .so mods, resolves ELF deps, calls `mod_preinit`/`mod_init` |
| `crash_handler.cpp` | 129 | Signal handlers for SIGSEGV/SIGABRT/SIGFPE/SIGBUS/SIGILL |
| `patch_utils.cpp` | 97 | Pattern-based memory scanning, x86/ARM instruction patching |
| `hybris_utils.cpp` | 55 | Load OS-native libraries via dlopen, register with bionic linker |
| `hybris_android_log_hook.cpp` | 61 | `__android_log_print` impl, registered as `liblog.so` symbols |
| `minecraft_version.cpp` | 34 | Version code parsing (962112004 → 1.21.120.4) |
| `fmod_utils.cpp` | 36 | Hook FMOD::System::init for custom sample rate |

**Used at runtime?** YES — every code path. The entire game loading pipeline calls into this library.

**Port complexity: LARGE.** Deeply coupled with bionic linker soinfo internals. The `minecraft_utils.cpp` monolith is 1007 lines of gnarly C++.

### `linker` — DELETED in Phase 6 ✅

The C++ bionic linker (previously ~37 C++ files + 2 C files, 3.8 MB) is **gone**. Its `linker`/`linker-c` cc::Build targets and the entire `crates/client/src/mcpelauncher-linker/` source tree were removed in Phase 6 (commit `15d07a2e` + follow-up). The game binary now links **zero** bionic linker symbols.

**The Rust linker crate** (`crates/linker/`) is now the **only loader**:
- Loads `libc.so` symbols (merged C++ + Rust libc symbols) via `main.rs:36` (`linker::load_library("libc.so", &libc_syms)`)
- Loads `libminecraftpe.so` with full ELF relocation, DT_NEEDED resolution, and hook injection (via `minecraft_load.rs`, the Rust port of `MinecraftUtils::loadMinecraftLib`)
- Loads stub libs (libEGL.so, libGLESv2.so, libfmod.so, libaaudio.so, libHttpClient.Android.so, etc.)
- Exports the full `mcpelauncher_dispatch_*` surface (`dlopen`/`dlsym`/`dlclose`/`dladdr`/`relocate`/`unload_library`/`get_library_base`/`get_library_code_region`) and `mcpelauncher_linker_{resolve,get}_rust_handle` natively

### `game-window` (916 KB) — MEDIUM

**6 files compiled** (from 24 available, eglut path):

| File | Lines | Role |
|------|-------|------|
| `window_eglut.cpp` | 454 | X11/EGLUT window: creation, mouse (abs + rel), keyboard (X11→Minecraft keycode), touch, paste, drop, focus, swap, vsync, fullscreen |
| `joystick_manager_linux_gamepad.cpp` | 150 | Gamepad connect/disconnect/button/axis event dispatch |
| `window_manager_eglut.cpp` | 46 | EGLUTWindowManager factory |
| `window_with_linux_gamepad.cpp` | 18 | Bridge gamepad events to window |
| `game_window_manager.cpp` | — | Framework, createManager |
| `game_window_error_handler.cpp` | — | Error handling |

**Port complexity: MEDIUM.** ~670 lines of C++. Key challenge is the X11 keycode mapping tables (~200 lines). The Rust eglut module already handles the X11/EGL part; remaining is gamepad integration.

### `linux-gamepad` (1.2 MB) — MEDIUM

**5 files:** `gamepad.cpp`, `gamepad_mapping.cpp`, `gamepad_manager.cpp`, `linux_joystick_manager.cpp`, `linux_joystick.cpp`.

**Role:** Polls `/dev/input/event*` via evdev, maps to SDL gamecontrollerdb, dispatches events.

**Port complexity: MEDIUM.** Clean separation of concerns. Rust `gilrs` crate could replace most of this.

### `msa-daemon-client` (1.4 MB) — MEDIUM

**2 files:** `service_client.cpp` (59 lines), `token.cpp` (24 lines).

**Role:** RPC client for MSA daemon. Methods: `getAccounts()`, `addAccount()`, `removeAccount()`, `pickAccount()`, `requestToken()`.

**Note:** The game currently loads `libHttpClient.Android.so` from disk and XAL works with valid cache data (per AGENTS.md). The daemon may only be needed for initial login or cache expiry. Could be replaced by in-process auth.

### `simpleipc` (7.5 MB) — LARGE (skippable)

**14 files:** Unix domain sockets, RPC layer, JSON/CBOR encoding, epoll I/O handler.

**Role:** Transport layer for communicating with the mcpelauncher-ui-qt daemon process. Used by `msa-daemon-client`, `daemon-client-utils`, file picker, Google credential request.

**Note:** If MSA auth is handled in-process (via loaded `libHttpClient.Android.so`), this entire library drops out.

### `cll-telemetry` (7.1 MB) — LARGE (skippable)

**15 files:** Event manager, HTTP client (libcurl), file/memory event batching, serialization, compression (zlib), scheduled upload.

**Role:** Telemetry collection and upload for Microsoft/CLL.

**Note:** Can be stubbed via `MCPELAUNCHER_DISABLE_TELEMETRY=true` or the existing stub path. The game runs fine without it.

### `mcpelauncher-common` (148 KB) — SMALL

**2 files:** `path_helper.cpp` (196 lines), `openssl_multithread.cpp` (19 lines).

**Role:** `PathHelper::findDataFile()`, `PathHelper::pathInfo` global. OpenSSL thread safety.

**Port complexity: SMALL.** Pure logic, no bionic deps. A focused Rust port would take a day.

### Smaller Libraries

| Library | File(s) | Role | Port |
|---------|---------|------|------|
| `daemon-client-utils` | `daemon_launcher.cpp` (194 lines) | Fork daemon, inotify wait | SMALL (skippable) |
| `logger` | `log.cpp` (22 lines) | printf-style logging | **PORTED** — Rust `util::logger` + thin `logger_stub.cpp` shim |

## Dependency Graph Between Libraries

```
mcpelauncher-common  (no deps)

linux-gamepad  (no deps, used by game-window)
game-window  →  linux-gamepad

mcpelauncher-core  →  mcpelauncher-common

simpleipc  (no deps)
daemon-client-utils  →  simpleipc, mcpelauncher-common
msa-daemon-client  →  simpleipc, daemon-client-utils

cll-telemetry  (standalone)
```

`logger` and `file-util` are no longer C++ static libs — both are ported to Rust (`crates/util/src/logger.rs`, `crates/util/src/file_util.rs`). The C++ bridge reaches them through FFI shims (`logger_stub.cpp` for `Log::vlog`, `env_path_util_*`/`file_util_*` for file-util).

All libraries are compiled locally by `cc::Build` instances in `build.rs` and linked as static archives in link order (dependents before dependencies).
