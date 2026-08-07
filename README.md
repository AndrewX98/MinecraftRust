# MinecraftRust : Rust Minecraft Bedrock Launcher

Pure-Rust launcher for Minecraft Bedrock on Linux. 100% Rust : no C++, no cmake, no external build tools. Loads to main menu.

## Platform & Support

* **Currently, only x86_64 Linux is supported** (macOS is untested/unsupported due to lack of hardware).
* **Xbox services are partially integrated, meaning multiplayer isn't available yet.** Sign-in runs as a PlayFab guest; online features are disabled.
* **No imgui** : no debug/overlay UI.
* **Aside from a slightly longer startup time, performance and behavior are identical** to the original C++ launcher.

## Architecture

**14 Rust crates** : a single pure-Rust binary, no C++ compilation.

| Crate | Role |
|-------|------|
| **client** | Main binary : eglut (X11/EGL windowing), FakeEGL, CorePatches, JNI dispatch, event loop, FakeLooper |
| **libc-shim** | Pure Rust libc replacement symbols (FILE\*, pthreads, sockets, mmap, etc.) |
| **linker** | Pure Rust ELF linker : the only loader for `libminecraftpe.so` and stub libs |
| **libjnivm-sys** | Pure Rust JNI VM (full JNIEnv vtable) |
| **corelib** | Core game loading, hook injection, patching, mod loader |
| **util** | Base64, arg parser, file utils, logging, properties |
| **apkinfo** | APK/AndroidManifest.xml parsing |
| **axml-parser** | Binary XML (AXML) parser for Android manifests |
| **simple-ipc** | Pure Rust IPC client/server over Unix sockets |
| **daemon-utils** | Pure Rust daemon launcher utilities |
| **msa-daemon-client** | Pure Rust MSA authentication daemon client |
| **cll-telemetry** | Pure Rust telemetry/eventing client |
| **common** | Shared types for launcher daemon/client protocols |
| **minecraft-imported-symbols** | Game symbol constants and auto-generated arrays |

* **Single Rust JNI VM** (`libjnivm-sys`) : handles all game JNI dispatch and class registration.
* **Single Rust ELF linker:** loads stub libs, `libc.so`, and `libminecraftpe.so` with full ELF relocation.

See `docs/ARCHITECTURE.md`.

## Requirements

* **Rust nightly** (e.g. `1.99.0-nightly`) : required for unstable features (`c_variadic`, etc.)
* System libraries: `libstdc++`, `pthread`, `dl`, `m`, `z`, `GL`, `EGL`, `curl`, `crypto`, `ssl`, `SDL2`, `pulse(-simple)`, `X11`, `evdev`, `png`, `udev`
* Extracted Minecraft Bedrock APK (via mcpelauncher-manifest tools or unzip)
* Runtime data: `libsqliteX.so` and `gamecontrollerdb.txt` bundled in `runtime/`

## Build

```bash
cargo build -p client
```

No cmake, no external build tools : Rust is the only dependency.

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
  -dg /path/to/extracted/minecraft \
  -dd ~/.local/share/mcpelauncher \
  -dc ~/.cache/mcpelauncher
```

If `-dd`/`-dc` are omitted, defaults are the XDG directories (`~/.local/share/mcpelauncher/`, `~/.cache/mcpelauncher/`).

`libsqliteX.so` and `gamecontrollerdb.txt` are searched via `DEV_EXTRA_PATHS` relative to `runtime/` in the project root. Both bundled in-tree.

## Status

* Game loads to main menu (loading bar 100%, main menu renders)
* Mouse (relative mode, pointer lock, cursor hide) and keyboard fully working

### Known Issues

* CorePatches vtable warning (`_ZTV21AppPlatform_android23`) : non-fatal
* Missing asset files (`subdirs.txt`, `particles.brarchive`) : non-fatal
* GatheringServiceRequest 404s on `/api/v1.0/config/public` and `/api/v1.0/access` : non-fatal (online features disabled)
* XAL ECDSA key cache can corrupt : remove `xal/` cache files containing `"Serialized to SharedPreferences"`

## Credits

This project builds on the work of the [mcpelauncher-manifest](https://github.com/minecraft-linux/mcpelauncher-manifest/) project by ChristopherHX and contributors, which made Minecraft Bedrock on Linux viable.

## Documentation

All docs live in `docs/`:

| Document | Description |
|----------|-------------|
| `ARCHITECTURE.md` | High-level architecture, crate layout, two-VM coexistence, single Rust linker |
| `JNI_VM.md` | JNI VM architecture, class registration |
| `STARTUP_FLOW.md` | Startup sequence from main() to game thread, step by step |
