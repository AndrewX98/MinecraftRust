# Port: macOS support

**Status: PLANNED** — goal is *playable on Mac* (reaches main menu, mouse/keyboard), both
architectures (`aarch64-apple-darwin` primary, `x86_64-apple-darwin` via build matrix).
Development happens on Linux; verification via cross-compile checks + GitHub Actions macOS
runners (see [Testing from Linux](#testing-from-linux)).

Upstream reference: `mcpelauncher-manifest` builds on macOS today — every item below cites
the C++ file it was derived from.

---

## Why this is tractable

The Rust port compiles **zero C++** (no `.cpp` files remain in the workspace). Upstream's
macOS pain (osx-elf-header, osx10_10_compat.c, Objective-C shims) mostly does not apply.
What remains:

| Area | Rust port state | Work needed |
|------|-----------------|-------------|
| linker | pure Rust, self-defined ELF types, libc `mmap`/`mprotect` | audit `/proc`, TLS, W^X on arm64 |
| libc-shim | 602 replacements; `epoll_*` pass through to host | kqueue-backed epoll on mac |
| simple-ipc / daemon-utils / msa-daemon-client | tokio `UnixStream` + reqwest | none expected |
| cll-telemetry | reqwest/tokio | none expected |
| audio | `cpal` (cross-platform) | none expected |
| paths | XDG-only (`path_helper.rs`) | mac variant |
| windowing | `eglut/` hardwired to X11+EGL | abstraction trait + Cocoa/GLFW backend (**big**) |
| GL | FakeEGL resolver = host EGL proc addr; `graphics_api` hardcoded ES2 (`main.rs:62`) | desktop-GL default on mac; optional ANGLE/MoltenVK |
| FMOD | hook surface ported (`fmod_utils.rs`) | host-dylib-first load order + `-df` flag |
| natives | `runtime/lib/x86_64/…` vendored for Linux | vendor mac stub libs |

## What upstream does on macOS (source of truth)

| Concern | Upstream implementation | File(s) in mcpelauncher-manifest |
|---------|------------------------|----------------------------------|
| Window backend | **GLFW/Cocoa replaces EGLUT** on APPLE (`GAMEWINDOW_SYSTEM_DEFAULT=GLFW`) | `game-window/BuildSettings.cmake`, `window_glfw.cpp:216,235,407` |
| Desktop GL | `mustUseDesktopGL() == true` → GLES2 calls mapped to desktop GL | `mcpelauncher-client/src/gl_core_patch.cpp:103` |
| ANGLE/MoltenVK | game ≥1.26.10: load `../Frameworks/mvk-angle/libEGL.dylib`, set `ANGLE_DEFAULT_PLATFORM=vulkan` + `VK_ICD_FILENAMES` | `main.cpp:250-260` (weak `elg_lib`) |
| Input quirks | scroll = `(dx+dy)*127` (not just dy); **Cmd** acts as Ctrl (paste/copy) | `window_callbacks.cpp:263,430` |
| epoll | link `epoll-shim`; kqueue IO handler in IPC | `epoll-shim/`, `simpleipc/src/unix/kqueue_io_handler.cpp` |
| Paths | Foundation-based app-bundle/data dirs | `path_helper_osx.mm`, `EnvPathUtil_MacOS.mm` |
| File picker | Cocoa backend instead of zenity | `file-picker/src/file_picker_cocoa.mm` |
| FMOD | load host `lib/native/{arch}/libfmod.dylib` first, fall back to android fmod+pulse/sdl; `-df/--disable-fmod` flag | `main.cpp:161,447-455,530-533`, `fmod_utils.cpp` |
| Natives | `mcpelauncher-mac-bin/lib/{arm64-v8a,x86_64}/` stub libs (libc.so, liblog.so, libjnivmsupport.so, libsqliteX.so) | `CMakeLists.txt:60-66` |
| Misc | broken-VSync workaround (timed sleep after swap); ELF headers provided by osx-elf-header | `window_glfw.cpp:216-240`, `osx-elf-header/` |

---

## Phases

### Phase 0 — Cross-compile baseline + CI gate *(do first)*
1. `rustup target add aarch64-apple-darwin x86_64-apple-darwin`
2. Fix everything `cargo check --target {aarch64,x86_64}-apple-darwin -p <crate>` flags.
   Known hotspots:
   - `crates/client/build.rs:4-19` — unconditional Linux dylib list (`stdc++`, `GL`,
     `EGL`, `X11`, `pulse*`, `evdev`, `png`, `udev`). Gate per-target; on mac most are
     unnecessary (frameworks come in via cpal/reqwest deps).
   - `crates/linker/src/utils.rs:236` — `/proc/self/exe` probe.
   - `crates/libc-shim/src/misc.rs:174-178` — `libc::epoll_*` don't exist on mac (Phase 2).
3. `.github/workflows/macos.yml`: matrix over `macos-15` (M1) and `macos-13` (Intel),
   `cargo build -p client --release`. This gates "compiles on real macOS" forever.

### Phase 2 — Portable core crates
4. libc-shim: implement `epoll_create/create1/ctl/wait` over **kqueue**
   (`#[cfg(target_os = "macos")]`), ~100–150 lines mirroring upstream `epoll-shim`.
5. linker audit: `MAP_ANONYMOUS` (exists on mac), TLS (`tls.rs`) vs mach thread-local
   storage, W^X (`MAP_JIT` + `pthread_jit_write_protect_np`) only if we ever map RWX,
   `/proc/self/exe` → guarded fallback.
6. Verify daemon chain E2E still passes on Linux after changes (regression guard).

### Phase 3 — Paths, pickers, natives
7. `path_helper.rs`: target-gated defaults — data `~/Library/Application Support/mcpelauncher`,
   cache `~/Library/Caches/mcpelauncher`; app-dir detection without Foundation
   (executable-path heuristic / `CF_BUNDLE_PATH` env set by the `.app` launcher).
8. `file_picker.rs`: zenity stays Linux-only; mac branch shells out to `osascript`
   (zero new deps).
9. Vendored natives: `runtime-mac/lib/{arm64-v8a,x86_64}/libsqliteX.so` + stub libs
   copied from upstream `mcpelauncher-mac-bin` (same license terms as upstream ships);
   select by target OS in `DEV_EXTRA_PATHS` handling.

### Phase 4 — Windowing abstraction + Cocoa backend *(the big one)*
10. Extract a small windowing trait from `eglut/` (window creation, GL swap, event pump,
    clipboard, mouse grab, resize callbacks). Linux keeps the existing X11+EGL impl
    untouched behind it.
11. Mac backend options, in preference order:
    - **`glfw` crate** (matches upstream exactly): Cocoa + `NSOpenGLContext`, gives us
      `glfwGetProcAddress` for the GL resolver. Smallest risk.
    - raw `objc2` + `cocoa` crates: no new heavyweight dep, but far more code.
12. Wire backend's `get_proc_address` into `HOST_PROC_ADDR_FN` (`rust_bridge.rs:581`)
    so FakeEGL resolves desktop-GL symbols instead of EGL.
13. Port input parity from `window_callbacks.cpp`: Cmd↔Ctrl mapping, scroll sign flip,
    clipboard paste path.

### Phase 5 — Graphics API selection + FMOD
14. On mac default `graphics_api = OPENGL` and return `true` from
    `mc_glcorepatch_must_use_desktop_gl` (`rust_bridge.rs:135` already exists — matches
    upstream `gl_core_patch.cpp:103`); keep ES2 fallback flow like upstream
    `main.cpp:479-506`.
15. Optional ANGLE/MoltenVK path for game ≥1.26.10: detect
    `<appdir>/../Frameworks/mvk-angle/{libEGL.dylib,MoltenVK_icd.json}`, set env vars
    before EGL init (mirrors `main.cpp:250-260`). Defer until desktop-GL works.
16. FMOD: try host `lib/native/{arch}/libfmod.dylib` through the Rust linker before
    android fmod; add `-df/--disable-fmod` flag (upstream `main.cpp:161`).

### Phase 6 — Packaging
17. Minimal `MinecraftRust.app` bundle (Info.plist, Frameworks dir for MoltenVK later),
    universal binary via `lipo` of both arch builds in CI.

---

## Testing from Linux

| Tier | Method | What it proves |
|------|--------|----------------|
| 1 | `cargo check --target {aarch64,x86_64}-apple-darwin` locally | type/API portability, fast iteration (~free) |
| 2 | GitHub Actions `macos-{15,13}` runners (repo is on GitHub: `AndrewX98/MinecraftRust`) | full build+link on real Apple toolchain |
| 2b | CI smoke run: launch headless-ish, then `screencapture shot.png` + upload artifact — macOS runners expose a real GUI session, so the window renders (Intel runners fall back to Apple Software Renderer, GL 4.1 core — enough for the desktop-GL menu path) | you literally *see* the main menu as a CI artifact |
| 3 | `sickcodes/docker-osx` QEMU/KVM VM locally | interactive screen, but software-GL only (no Metal in a VM), slow, and against Apple's EULA on non-Apple hardware |
| 4 | any real/borrowed Mac (Screen Sharing/VNC from Remmina) | final validation incl. Metal/ANGLE |

Practical loop while developing: Tier 1 for code, Tier 2b for eyes-on.

## Risks

- **GLFW-on-mac specifics**: retina scaling, key codes, VSync quirk (upstream sleeps
  manually when `brokenVSync` trips — expect to port that).
- **Software renderer limits**: Vibrant Visuals won't render under Apple Software
  Renderer; needs the MoltenVK/ANGLE route eventually.
- **No local hardware**: iterate-by-CI-log is slow; budget for it.
- **FMOD licensing**: ship dylibs the same way upstream does (they're game-shipped
  binaries); do not relicense or modify.
