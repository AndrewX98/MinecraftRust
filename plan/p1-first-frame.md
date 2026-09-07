# P1 — first_frame 410ms → ~385ms (6 low-risk wins, ~25ms)

Baseline after P0 (release --features perf, `CLIENT_BIN=target/release/client`):
```
eglut_window 63-67 | goblin 0 | reloc 69-70 | linker_load_game 89-92 | total 259-263 | wall 409-421 (jitter 460)
P0 saved ~31ms linker (goblin 25+reloc 4) + 10ms eglut. Prod 6.5M 0 PERF lines.
```

## Goal
25ms additional wall on `release --features perf`, zero prod overhead. No game-thread behavior change.

## P1-1 — Skip main-thread `eglMakeCurrent/SwapInterval/release` (8ms)
**Files:** `crates/client/src/eglut/compat.rs:134-158` → `crates/client/src/game_window.rs:148-167` → `crates/client/src/rust_bridge.rs:1432-1442`

**Root:** `eglutCreateWindow` does `eglMakeCurrent(surf,surf,ctx)` + `eglSwapInterval(1)` + later `fake_egl_release_context()` (`game_window.rs:162` → `rust_bridge.rs:1433` `REAL_EGL_MAKE_CURRENT(EGL_NO_SURFACE)`). Costs 2 driver ioctl roundtrips on main thread; game thread re-binds via `game_window_make_current(draw,1)` → `eglutMakeCurrent(1)` (`rust_bridge.rs:918`, `game_window.rs:101`, `eglut/window.rs:192`) anyway — TLS `EGLContext` is per-thread.

**Fix:**
- `compat.rs:134-158`: keep `eglCreateContext` + `eglCreateWindowSurface`, **delete** `eglMakeCurrent` + `eglSwapInterval` + error eprintln. Store `context/surface/config` in `STATE.current_window` as now.
- `game_window.rs:157-162`: delete `fake_egl_release_context()` call (or keep as no-op if `STATE.current_window` already holds handles). `fake_egl_save_current_window_handle` must seed `SAVED_EGL_DISPLAY/CONFIG` from `STATE.egl_dpy / current_window.config` without querying `eglQueryContext` (see P1-4).
- Assert: `fake_egl_setup_gl_overrides` etc. still run.

**Risk:** Low-Med — driver `MakeCurrent` on wrong thread triggers `EGL_BAD_ACCESS` before; removing avoids Mesa thread-affinity bug. Must keep `SAVED_EGL_*` seeding via `STATE` fallback (already at `rust_bridge.rs:1393-1417`).

**Verify:** `eglut_window 63→~53` (-8ms), harness still `PASS n=0`.

## P1-2 — `XInternAtoms` batch (5ms)
**File:** `crates/client/src/eglut/window.rs:57-68` (`XdndDrop`×9)

**Root:** 9 serial `XInternAtom` each 0.5-1ms roundtrip (Unix domain). `eglut_init_inner:57` before `eglutCreateWindow`.

**Fix:** Single `XInternAtoms(dpy, names[9], False, atoms[9])`:
```rust
let mut names = [b"XdndDrop\0".as_ptr()..., ...];
let mut atoms = [0;9];
XInternAtoms(dpy, names.as_mut_ptr() as *mut *mut c_char, 9, 0, atoms.as_mut_ptr());
STATE.xdnd_drop = atoms[0]; ...
```
Keep order identical; fallback to per-atom on `XInternAtoms` null.

**Risk:** Very Low — wire identical, one roundtrip vs 9.

**Verify:** `eglut_window` extra -4-5ms, no drop-paste regression.

## P1-3 — Dedup `libEGL.so` dlopen + 14 `dlsym` (4ms)
**Files:** `crates/client/src/game_window.rs:174 real_egl_get_proc_address()` + `crates/client/src/rust_bridge.rs:1164 fake_egl_install_library()` (macro `dlsym_egl!` ×14)

**Root:** Two `dlopen("libEGL.so",RTLD_LAZY)` filesystem probes (`ld.so.cache`+`openat`) + refcount bump, plus 14 serial `dlsym` under `ld.so` lock.

**Fix:** `OnceLock<*mut c_void>` global `EGL_HANDLE`:
```rust
static EGL_HANDLE: OnceLock<*mut c_void>
fn egl_handle() -> *mut c_void { *EGL_HANDLE.get_or_init(|| dlopen("libEGL.so",RTLD_LAZY)) }
```
`real_egl_get_proc_address` uses `egl_handle()` + `dlsym(eglGetProcAddress)`, `fake_egl_install_library` reuses same handle. Or fetch `eglGetProcAddress` via `dlsym(RTLD_DEFAULT)` without opening file. Keep `OnceLock` for 14 symbols cache.

**Risk:** Very Low.

## P1-4 — Skip `SAVED_EGL_CONFIG` reconstruct query (3ms)
**File:** `crates/client/src/rust_bridge.rs:1372-1385` `fake_egl_save_current_window_handle`

**Root:** Always tries `REAL_EGL_QUERY_CONTEXT(EGL_CONFIG_ID)` + `REAL_EGL_CHOOSE_CONFIG([CONFIG_ID])` before using cached `STATE.current_window.config` (`compat.rs:70` `eglutChooseConfig`). Two driver calls.

**Fix:** Prefer cached path first: if `!STATE.egl_dpy.is_null() && STATE.current_window.is_some()` set `SAVED_EGL_DISPLAY/CONFIG` directly from `STATE`, skip `query_ctx/choose_cfg` unless cache empty. Keep query as fallback only when `STATE.current_window.is_none()` (pre-P0 fallback path).

**Risk:** Very Low — `eglutCreateWindow` always populates `current_window.config`.

## P1-5 — GLES 166-symbol relocate: kill per-symbol Mutex+alloc (5ms)
**Files:** `crates/client/src/startup.rs:299-319` `mc_relocate_glesv2_symbols` → `crates/client/src/rust_bridge.rs:1081 fake_egl_get_proc_address` (`HOST_PROC_OVERRIDES` Mutex)

**Root:** 166× `resolve(cname)` → `fake_egl_get_proc_address` locks `HOST_PROC_OVERRIDES` Mutex + `CString::new` + `HashMap` insert + `add_symbols`. Gated on `linker_load_game` sequential wall.

**Fix (A, no parallelism):**
- Snapshot `HOST_PROC_ADDR_FN` outside loop; only lock `HOST_PROC_OVERRIDES` on hit.
- `HashMap::with_capacity(166)`, reuse single `CString` buffer or `CStr` → avoid 166 allocs.
- `linker::add_symbols` once.
- Optional: move `mc_relocate_glesv2_symbols` off critical path — it must precede `linker_load_game` `BIND_NOW` for `libGLESv2.so` GOT, but can overlap with `goblin_parse` portion if threaded (`std::thread::scope`). Defer to P2 if needed.

**Risk:** Low.

## P1-6 — Delete double `register_global_exports` (5ms) + small linker micros
**File:** `crates/linker/src/lib.rs:1813` and `1942` inside `load_library_internal_no_ctors` + `load_library_internal`

**Root:** `register_global_exports(&mut state,&loaded.soinfo)` called **twice** in `no_ctors` path (before deps + after reloc). Loops all `~18k` dynsym entries (`CStr::from_ptr`+`to_str`+`String` clone + `global_symbols.entry`). Second pass is redundant: deps' exports already registered when deps loaded; game is leaf. Contributes ~4ms to `linker_load_game` 24ms overhead. Also `external_symbols` `HashMap<String,usize>` clone per load (6 libs ×80 entries).

**Fix:**
- Delete `register_global_exports` at `lib.rs:1813` (keep only post-reloc at `1942` after `mprotect(READ|WRITE)`+`reloc`). Add `debug_assert!` that deps already visible.
- Optional micro (within same PR): cache `soinfo.external_symbols` as `HashMap<String,usize>` once, avoid `k.clone()` per lib; `PHDR` `format!("{dir}/{name}")` → `PathBuf::join`.

**Risk:** Low — verify `load_dependencies` order: game `DT_NEEDED` are stubs (`libm` etc.) already loaded; no circular need for game's exports at dep load time.

## Order & projection
```
P1-1 main-thread MakeCurrent   -8ms  eglut 63→55
P1-2 XInternAtoms              -5ms  →50
P1-3 dedup dlopen               -4ms  →46
P1-5 GLES loop                 -4ms  (inside eglut span, overlaps)
P1-4 SAVED_EGL_CONFIG           -3ms  →43
P1-6 double register            -5ms  linker 92→87
-------------------------------------------
Total P1: wall 410 → ~385 (-25ms), linker 92→87, eglut 63→43 (floor ~26ms XOpen+eglInitialize)
```

**Sequence:** P1-2 → P1-3 → P1-4 → P1-1 (test EGL path last) → P1-5 → P1-6. Each isolated, reviewable.

## Measurement
```sh
cargo build -p client --release --features perf
MINECRAFT_GAME_DIR=/home/andrew/.local/MinecraftLauncher/extracted/1.26.20 \
  CLIENT_BIN=target/release/client \
  cargo test -p client --test first_frame -- --ignored --nocapture 2>&1 | grep -E "eglut|goblin|reloc|linker_load|total_to|first_frame"
# prod 0-overhead:
cargo build -p client --release
RUST_LOG=error timeout 6 target/release/client -dg ... 2>&1 | grep -c PERF # 0
cargo build -p linker && cargo build -p client
```

## Out of scope (P2)
FakeLooper `poll(timeout)` blocking, `HOST_DLSYM_CACHE` RwLock, zero-copy hash tables (`to_vec`→borrow), shader binary cache, `mprotect` coalesce, `eprintln!` gating — tracked separately.
