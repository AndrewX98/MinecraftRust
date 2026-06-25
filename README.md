# MinecraftRust — Rust Minecraft Bedrock Launcher

Pure-Rust launcher for Minecraft Bedrock on Linux. C++ bridge is temporary scaffolding, every subsystem should end up pure Rust. **~8.5% Rust by total lines (17K of 200K+), but all game-facing JNI dispatch, class registration, and startup orchestration is Rust. Loads to main menu.**

## Architecture

**15 Rust crates** (~17,000 lines) + **C++ bridge** (~5,700 lines compiled locally via `cc::Build`, no cmake).

| Crate | Role |
|-------|------|
| **client** | Main binary — eglut, FakeEGL, CorePatches, JNI modules, event dispatch, FakeLooper |
| **libc-shim** | 602 pure Rust libc replacement symbols (FILE\*, pthreads, sockets, mmap, etc.) |
| **linker** | Pure Rust ELF linker (loads stub libs; game lib still uses C++ bionic linker) |
| **libjnivm-sys** | Pure Rust JNI VM (~250 function JNIEnv vtable) — active for ALL JNI dispatch after env switch |
| **eglut** | Pure Rust X11/EGL windowing + event loop |
| **game-window** | winit/glutin abstraction (not active — eglut path used) |
| **util** | Base64, arg parser, file utils, logging, properties |
| **apkinfo** | APK/AndroidManifest.xml parsing |
| **simple-ipc** | Pure Rust IPC client/server over Unix sockets |
| **daemon-utils** | Pure Rust daemon launcher utilities |
| **msa-daemon-client** | Pure Rust MSA authentication daemon client |
| **cll-telemetry** | Pure Rust telemetry/eventing client |
| **common** | Shared types for launcher daemon/client protocols |
| **minecraft-imported-symbols** | Game symbol constants and auto-generated arrays |
| **axml-parser** | Binary XML (AXML) parser for Android manifests |

Two JNI VMs coexist: Rust libjnivm-sys (active — env switch done, game JNI dispatch through Rust vtable) and C++ FakeJni/Baron (legacy — kept for `ga->vm` operations and FakeLooper exit callback).  
Two linkers coexist: Rust linker for stub libs + `libc.so`, C++ bionic linker for `libminecraftpe.so`.

See `docs/ARCHITECTURE.md`.

## Requirements

* Rust 2021 edition (stable)
* System libraries: `libstdc++`, `pthread`, `dl`, `m`, `z`, `GL`, `EGL`, `curl`, `crypto`, `ssl`, `SDL2`, `pulse(-simple)`, `X11`, `evdev`, `png`, `udev`
* Extracted Minecraft Bedrock APK (via mcpelauncher-manifest tools)
* Runtime data: `libsqliteX.so` and `gamecontrollerdb.txt` bundled in `runtime/`

## Build

```bash
cargo build -p client
```

All C++ bridge files compiled locally via `cc::Build` — no cmake, no external build tools.

## Usage

```
Program Help
-h  --help         Show this help information
-dg --game-dir     Directory with the game and assets (required)
-dd --data-dir     Directory to use for the data
-dc --cache-dir    Directory to use for cache
-v  --version      Print version info
```

```bash
# quick start
timeout 25 ./target/debug/client -dg /path/to/extracted/minecraft

# with explicit data/cache dirs
./target/debug/client \
  -dg ~/.local/MinecraftLauncher/extracted/1.26.3.1 \
  -dd ~/.local/share/mcpelauncher \
  -dc ~/.cache/mcpelauncher
```

If `-dd`/`-dc` are omitted, C++ `PathHelper` defaults to XDG directories (`~/.local/share/mcpelauncher/`, `~/.cache/mcpelauncher/`).

`libsqliteX.so` and `gamecontrollerdb.txt` are searched via `DEV_EXTRA_PATHS` relative to `runtime/` in the project root. Both bundled in-tree.

XAL cache lives in `~/.local/share/mcpelauncher/xal/` and `~/.local/MinecraftLauncher/xal/`. Delete those directories if auth fails.

## Porting Progress

| Category | Rust % | Target |
|----------|--------|--------|
| libc shim | 100% | 100% |
| JNI VM | 100% | 100% (bridge only remaining) |
| EGL | 100% | 100% |
| FakeLooper | ~70% | 100% |
| ELF linker (bionic) | ~30% | 100% |
| Game window | ~30% | 100% |
| JNI classes | ~70% | 100% (all 57 MainActivity methods + 9 wrappers done) |
| mcpelauncher-core | ~0% | 100% (game loading, hooks, patching, mod loading) |
| Startup orchestration | ~60% | 100% |
| Build system | 100% | 100% (no cmake) |
| IPC/Telemetry | ~0% | 100% (Rust crates exist) |

15/25 JNI files ported to Rust. All 57 MainActivity methods and 9 wrapper classes (File, Context, Build, PackageInfo, etc.) replaced. Env switch from FakeJni to libjnivm-sys complete — game dispatches JNI through Rust vtable.

See `docs/PORTING_PROGRESS.md`.

## Status

* Game loads to main menu (loading bar 100%, main menu renders)
* Mouse (relative mode, pointer lock, cursor hide) and keyboard fully working
* Pure Rust JNI VM (libjnivm-sys) with zero compile errors
* No cmake dependency — fully self-contained build
* All C++ static libs compiled locally via `cc::Build`

### Remaining Issues

* CorePatches vtable warning (`_ZTV21AppPlatform_android23`) — non-fatal
* Missing asset files (`subdirs.txt`, `particles.brarchive`) — non-fatal
* GatheringServiceRequest 404s on `/api/v1.0/config/public` and `/api/v1.0/access`
* XAL ECDSA key cache can corrupt — remove `xal/` cache files containing `"Serialized to SharedPreferences"`

## Credits

This project builds on the work of the [mcpelauncher-manifest](https://github.com/minecraft-linux/mcpelauncher-manifest/) project by ChristopherHX and contributors. The original C++ launcher provided the game loading pipeline, bionic linker integration, JNI infrastructure, and hybris-based Android compatibility layer that this Rust version is progressively replacing. The mcpelauncher project made Minecraft Bedrock on Linux viable.

Key components ported from the original C++ codebase:

* **bionic linker** — full ELF dynamic linker for loading `libminecraftpe.so`
* **mcpelauncher-core** — game loading, hook injection, mod loader, crash handling
* **FakeJni / Baron** — Android JNI VM implementation (class registration, method dispatch)
* **libjnivm** — C++ JNI library that provides the JNIEnv vtable
* **game-window / linux-gamepad** — X11/EGL window management and evdev gamepad support
* **msa-daemon-client / simpleipc** — Microsoft Account authentication infrastructure
* **FakeLooper / FakeAssetManager** — Android native API stubs

## Documentation

All docs live in `docs/`:

| Document | Description |
|----------|-------------|
| `ARCHITECTURE.md` | High-level architecture, crate layout, two JVM/linker coexistence |
| `PORTING_PROGRESS.md` | Porting status per JNI file, static libs, bridge stubs |
| `JNI_VM.md` | JNI VM architecture — libjnivm-sys vs FakeJni/Baron, class registration |
| `STATIC_LIBS.md` | All 13 cc::Build instances, dependency graph, port complexity |
| `CXX_BRIDGE.md` | Rust/C++ FFI interface — extern "C", #[no_mangle], all bridge files |
| `STARTUP_FLOW.md` | Startup sequence from main() to game thread, step by step |
