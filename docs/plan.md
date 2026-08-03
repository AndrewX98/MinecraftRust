# Plan: Fully Migrate C++ Bionic Linker → Pure Rust

## Goal

Make the **dynamic linker fully Rust-owned**: no `linker` / `linker-c` static archives from `cpp-bridge-sys`, no dual-linker mirroring, no C++ `soinfo` registration, no C++ `do_dlopen` fallback. All game and dependency loads (`libminecraftpe.so`, `libc++_shared.so`, `libfmod.so`, stubs, mods) go through `crates/linker/`.

**Out of scope (this plan):** porting the rest of the C++ bridge (JNI FakeJni, mcpelauncher-core patching, window, telemetry). Those can keep calling a thin **Rust FFI surface** until they are ported themselves. The deliverable is: *linker dependency graph is pure Rust*.

---

## Post-mortem: Phase 3.4 attempt (2026-08-02) — FAILED, reverted

A first attempt at migrating `libfmod.so` to the Rust linker crashed with
heap corruption at game startup:

```
Thread 12 "MINECRAFT MAIN " SIGSEGV
jnivm::NewDirectByteBuffer(env=..., buffer=0x25, capacity=140704442428609)
```

**What was changed (commit `ee7e65f` + uncommitted work, preserved in
`git stash@{0}` and tag `phase3.4-broken`):**
1. Removed `libfmod.so` from the C++ `linker_cpp_dlsym_fallback` cache
   (`capi.cpp`).
2. Changed `MinecraftUtils::loadMinecraftLib` from C++ `linker::dlopen`
   to a new `linker_rust_dlopen_fmod` that Rust-loads fmod with
   `load_library_internal` (runs its 12 INIT_ARRAY ctors) and then
   registers a C++ soinfo mirror via `mcpelauncher_linker_register_loaded_library`.
3. Made `is_cpp_preloaded_dependency` return `false` always, so the Rust
   linker no longer skipped fmod when resolving the game's `DT_NEEDED`.

**Why it crashed:** `libfmod.so` has 12 INIT_ARRAY ctors, `BIND_NOW`, and
heavy C++ ABI. Loading a second image through the Rust `reloc.rs` hot path
and stripping the C++ fallback cache in the same change left JUMP_SLOTs
unresolved; the game's DT_INIT/audio init jumped through garbage, corrupting
the heap (garbage args in `NewDirectByteBuffer`). This is exactly the
"double-map / half-broken ctor" risk in the Risk Register.

**Reverted to:** `c7a0d83` (phase 3.3) — working main menu. `main` reset
to it; `git stash@{0}` holds the 3.4 experiment.

**Lessons for re-doing fmod (folded into Phase 3 below):**
- Do **not** drop the C++ fallback entry for fmod until the Rust image is
  proven fully resolved (unresolved JUMP_SLOT count == 0 under gdb).
- Do **not** flip `is_cpp_preloaded_dependency` to false for fmod while C++
  still owns any of its symbols.
- Verify ctors ran exactly once in one healthy image (no mirror + remap).
- Phase 3 ordering is aggressive; do fmod **last**, after Phase 1 (unified
  Rust handles) so there is a single handle model to debug against.

---

## Current State (code truth, not stale docs)

### Dual-linker coexistence

| Layer | Owner today | Notes |
|-------|-------------|-------|
| Stub registration (`libc`, GLES, OpenSLES, …) | **Rust primary** (Phase 2 done); C++ mirrors gated off under `RUST_ONLY` | `capi.cpp` `mc_load_core_libraries` + `setupHybris` |
| Real ELF deps (`libc++_shared`, `libfmod`, `libpairipcore`, sqlite) | **Rust** (Phase 3 done) | `linker_rust_dlopen_*` in `loadMinecraftLib` |
| Game `libminecraftpe.so` | **Rust first**, C++ fallback | `linker_rust_dlopen_ext` → success returns Rust handle |
| Mod `.so` loading | **C++ still** | `mod_loader.cpp` `linker::dlopen_ext` — re-point in Phase 4 |
| Symbol lookup for unresolved JUMP_SLOTs | Rust `global_symbols` | `DLSYM_FALLBACK` removed (Phase 3 done) |
| Handles | **Unified Rust `usize`** (Phase 1 done) | dispatch wrappers handle C++-only leftovers |

> **Phase 1, 2, 3 are DONE (commits `c348ac5`, `346b91e9`).** Only the C++ *orchestrator* in
> `loadMinecraftLib` (Phase 4) and the remaining C++ `linker::` call sites / mod re-pointing
> stand between the present state and Phase 6 deletion. Lines 53–83 records the *state those
> phases fixed*; Phase 4–6 is the focus now.

### Rust linker readiness

- **~11.3k LOC** across 24 modules in `crates/linker/` — most bionic subsystems already ported (loader, phdr, relocate engines, soinfo, TLS, libdl, config, solist, CFI, GDB, packed reloc iter).
- Live game path uses simpler `reloc.rs` + hook-preferring resolver; fuller `relocate.rs` exists with unit tests but is not the hot path.
- Explicit anti-double-map: skips reloading C++-preloaded deps (`is_cpp_preloaded_dependency`) — double mapping caused constructor SIGSEGVs.
- Deferred constructors: game DT_INIT runs later via `linker_rust_call_init_functions` from `jni_support`.

### C++ still compiled for linker

`cpp-bridge-sys/build.rs` target **`linker`** (~35 TUs) + **`linker-c`** (strlcpy/strlcat):

- Core: `linker.cpp` (~3.8k), `linker_phdr`, `linker_soinfo`, `linker_relocate`, `dlfcn`, namespaces, config, CFI, GDB, …
- Support: android-base, liblog, zip_archive, async_safe_log
- Wrapper: `mcpelauncher-linker/src/linker.cpp` (`mcpelauncher_linker_register_loaded_library`, `linker::init`)

### Docs lag

`ARCHITECTURE.md` / `AGENTS.md` / `STARTUP_FLOW.md` still say “C++ only loads the game.” Trust **code** + update `docs/LINKER_PORTING_PROGRESS.md` as the tracker.

---

## Success Criteria

1. **`libminecraftpe.so` loads only via Rust** — C++ fallback path deleted or `#error` if reached.
2. **All real deps load only via Rust** — no C++ `linker::dlopen` for fmod / libc++ / pairip / sqlite.
3. **No `DLSYM_FALLBACK` to C++** — Rust global/symbol tables are complete.
4. **No `mcpelauncher_linker_register_loaded_library`** — HookManager and remaining C++ consumers use Rust handles via FFI, or are ported.
5. **`cpp-bridge-sys` no longer builds `linker` / `linker-c`**; client does not `-llinker`.
6. **Smoke: `cargo build -p client` + run to main menu** with existing game dir (`-dg`).
7. Optional but recommended: unit tests in `crates/linker` for load of a small fixture `.so` with deps + hooks.

---

## Architecture Target

```
                    ┌─────────────────────────────┐
                    │  client (Rust) / remaining  │
                    │  C++ bridge (JNI, utils)    │
                    └─────────────┬───────────────┘
                                  │  pure C ABI
                                  ▼
                    ┌─────────────────────────────┐
                    │  crates/linker (ONLY linker) │
                    │  load_library / dlopen_ext  │
                    │  stubs + real ELF + hooks   │
                    │  soinfo + TLS + libdl       │
                    └─────────────────────────────┘
                                  │
                    mmap PT_LOAD, relocs, DT_NEEDED
                                  ▼
                    libminecraftpe.so + game deps
```

**Handle model:** keep Rust sequential `Handle = usize` as the only handle. Expose stable C ABI:

```c
// Proposed stable ABI (names already partially exist)
size_t  linker_init_rust(void);
size_t  linker_load_library_rust(...);      // stubs
size_t  linker_rust_dlopen / dlopen_ext(...);
void*   linker_rust_dlsym(size_t handle, const char*);
size_t  linker_rust_get_library_base(size_t);
void    linker_rust_get_library_code_region(...);
void    linker_add_symbols_to_library_rust(...); // relocate/hot-patch stubs
int     linker_rust_dlclose(size_t);
int     linker_rust_dl_iterate_phdr(...);
char*   linker_rust_dlerror(void);
void    linker_rust_add_search_path(const char*);
bool    linker_rust_call_init_functions(const char* soname);
```

C++ `namespace linker` becomes thin wrappers over these symbols (temporary), then deleted as call sites move to Rust.

---

## Migration Phases

### Phase 0 — Baseline & instrumentation (½–1 day)

**Why:** dual path can hide which linker actually loaded the game.

1. Log (and optionally env-gate) whether game load used Rust success vs C++ fallback.
2. Add `MCPELAUNCHER_LINKER_RUST_ONLY=1` that **aborts on C++ fallback** — use as regression gate for later phases.
3. Dump post-load solist from both sides (Rust `linker_show_state_rust`, C++ if still present) for one successful main-menu run; record unresolved symbol counts if any.
4. Refresh `docs/LINKER_PORTING_PROGRESS.md` section 29 to match code (Rust-first game load).

**Exit:** confirmed current runtime path (expect Rust game map + C++ soinfo registration + C++ preloads).

---

### Phase 1 — Unify on Rust handles for all *new* loads (2–4 days)

**Goal:** stop creating C++ soinfo for Rust-loaded ELFs; make remaining C++ APIs accept Rust handles.

1. **Extend C ABI** so C++ can query Rust soinfo fields without `prelink_image`:
   - Already partial: `linker_rust_get_soinfo_symbol_data`, `get_library_base`, `get_library_code_region`.
   - Add: `get_dynamic`, `get_strtab`/`symtab`, `get_phdr`/`phnum`, `dladdr` by address across Rust libs, `dl_iterate_phdr`.
2. **Port `HookManager`** (`mcpelauncher-core/src/hook.cpp`) off `soinfo_from_handle`:
   - Use Rust FFI to get `base` + dynamic section + symbol index.
   - Or port HookManager to Rust (preferred long-term; ~265 lines + ELF walk).
3. **Change `linker_rust_dlopen_ext`** to return **Rust handle** (not C++ soinfo handle). Update `loadMinecraftLib` / `HookManager::addLibrary` / `jni_bridge` `g_jni_game_handle` accordingly.
4. **Baron / `attachLibrary`** path that passes `{dlopen, dlsym, dlclose_unlocked}` — point function pointers at Rust `__loader_*` equivalents from `libdl.rs` (or keep C++ wrappers that forward).

**Risk:** any C++ path still calling `soinfo_from_handle` on a Rust handle will crash. Grep and convert all sites before flipping return type.

**Exit:** game loads with Rust handle end-to-end; `mcpelauncher_linker_register_loaded_library` unused (can stub-return null under feature flag).

---

### Phase 2 — Rust-primary stub & core library registration (2–3 days)

**Goal:** invert `mc_load_core_libraries`: Rust registers stubs first; C++ only mirrors if still needed (then stop mirroring).

1. Move stub registration order in `capi.cpp` (or rewrite in `capi.rs` / `main.rs`):
   - `linker_init_rust()` only (no `mcpelauncher_linker_cpp_init` once Phase 5 approaches).
   - `linker::load_library` → pure Rust for: `libc`, `libdl`, `libm` symbols, `libOpenSLES`, `libGLESv*`, `libstdc++`, `liblog`, `libandroid`, `libaaudio`, gamewindow, http stubs.
2. Port GLES relocate (`mc_relocate_glesv2_symbols`) to `linker_add_symbols_to_library_rust` only.
3. Port FakeEGL `linker_load_library` path (already half-Rust) to drop C++ `linker::load_library`.
4. Keep a temporary C++ mirror **only** if some unported C++ still requires C++ solist; delete mirror once grep shows zero C++ soinfo consumers for stubs.

**Exit:** with `RUST_ONLY`, stubs exist only in Rust state; `mirror_rust_load` deleted.

---

### Phase 3 — Load all real dependencies through Rust (3–6 days) — **highest risk**

**Goal:** remove `is_cpp_preloaded_dependency` and C++ preloads in `loadMinecraftLib`.

Order of migration (each must reach main menu alone):

| Order | Library | Why careful |
|-------|---------|-------------|
| 1 | `libsqliteX.so` / symbols as hooks | Already symbol-injected; simpler |
| 2 | `libpairipcore.so` | Optional integrity; may stub if unused |
| 3 | `libc++_shared.so` | C++ runtime; IFUNC/TLS/ctors; game depends heavily |
| 4 | Any other DT_NEEDED of the game not already stubbed | Discover via `readelf -d libminecraftpe.so` |
| **5** | **`libfmod.so` — LAST** | **Audio; 12 INIT_ARRAY ctors, BIND_NOW, heavy C++ ABI; hooks on `System::init` / `setOutput`. Crashed on first attempt (see post-mortem). Treat as highest risk.** |

For each:

1. Load via Rust `dlopen` / `load_library_internal` **before** game load.
2. Register exports into Rust `global_symbols` (already done for non-stub loads).
3. Remove from `is_cpp_preloaded_dependency`.
4. Remove corresponding C++ `linker::dlopen` in `minecraft_utils.cpp`.
5. Clear `DLSYM_FALLBACK` usage for that library’s symbols; fail loud if unresolved JUMP_SLOTs remain.

**fmod-specific procedure (do NOT repeat the 3.4 mistakes):**

1. **Keep `libfmod.so` in the C++ `linker_cpp_dlsym_fallback` cache** the whole time — only drop it once the Rust image is proven complete.
2. Land on **one** load: pick Rust as owner and make C++ *stop* loading fmod entirely (no mirror via `mcpelauncher_linker_register_loaded_library`, no C++ soinfo). Single owner, single image, ctors run once.
3. Under gdb, confirm after fmod load: unresolved JUMP_SLOT count == 0, and its 12 ctors ran (break on `DT_INIT_ARRAY`). If relocs are incomplete, fix the reloc engine (see IFUNC/TLS hardening) **before** touching fallback cache.
4. Only then flip `is_cpp_preloaded_dependency` for fmod to false and remove the C++ `linker::dlopen`.
5. Consider disabling FMOD hooks (`MCPELAUNCHER_PATCH_FMOD=0`) as a bisect aid to separate "fmod image broken" from "hook broken".

**Hard requirements for this phase:**

- **Ctor policy:** match C++ behavior — run DT_INIT for deps at load time; keep game ctors deferred until JNI ready.
- **No double map:** never load the same soname twice in one process.
- **Hook preference:** `external_symbols` / mcpelauncher hooks must still win over self-exports (Swappy, mouse, FMOD, sqlite, telemetry).
- **IFUNC:** ensure `reloc.rs` hot path handles IRELATIVE / IFUNC like `relocate.rs` (or switch hot path to `relocate.rs`).
- **TLS:** verify `libc++_shared` / game TLS relocs; fix `TPOFF64` if TP base is wrong on Linux host.

**Optional hardening:** prefer consolidating on **one** reloc engine (`relocate.rs`) for the live path to avoid dual implementations drifting.

**Exit:** game + all real deps loaded only by Rust; `DLSYM_FALLBACK` unset / removed; main menu green with `MCPELAUNCHER_LINKER_RUST_ONLY=1`.

---

### Phase 4 — Port `MinecraftUtils::loadMinecraftLib` orchestration to Rust (2–4 days)

**Goal:** the last C++ *orchestrator* of the linker API moves to Rust; C++ may keep unrelated utils temporarily.

1. Port hook assembly (mouse, fullscreen, close, FMOD, webrtc ifaddrs, telemetry, sqlite symbol list) into a Rust module, e.g. `client/src/minecraft_load.rs` or `linker` helper.
2. Port FmodUtils setup that needs `dlsym` on fmod handle.
3. Call sequence becomes pure Rust from `main.rs` / `capi.rs`:
   - search path → stubs → real deps → `dlopen_ext(libminecraftpe, hooks)` → HookManager (Rust) → return handle.
4. Leave C++ `loadMinecraftLib` as a thin deprecated wrapper or delete when `jni_bridge` no longer calls it.

**Related consumers to convert in the same PR series:**

- `mod_loader.cpp` — **the only remaining genuine *real-ELF* user of the C++ bionic linker**.
  Once the game + all deps load via Rust, `linker::dlopen_ext(mod .so)` (line 21) is the
  last call that actually maps+relocates an ELF through bionic. It must move to the Rust
  `dlopen_ext` before Phase 6 can delete the C++ loader:
  1. `linker::dlopen_ext(path, 0, &extinfo)` → `linker_rust_dlopen_ext` (same hook-array ABI,
     already exercised by the game load).
  2. `localFrame.getJniEnv().getVM().attachLibrary(path, "", {linker::dlopen, linker::dlsym, linker::dlclose})`
     → the function pointers must point at the Rust `__loader_*` equivalents from `libdl.rs`
     (dlopen/dlsym/dlclose/dlclose_unlocked), so Baron queries the Rust linker.
  3. `linker::dlopen`/`dlsym`/`dlclose` on the returned handle → `mcpelauncher_dispatch_*`.
- `hybris_utils.cpp` — host `dlopen` + re-export symbols as Rust stubs (small; already calls
  `linker_load_library_rust`). The `linker::load_library`/`dlopen` mirrors here can be dropped.
- `minecraft_utils` API surface used by mods (`mcpelauncher_load_library`, `mcpelauncher_dlclose_unlocked`
  = `linker::dlclose_unlocked`) → Rust-forwarding shims.

**Exit:** no C++ call to `linker::dlopen*` / `load_library` / `relocate` remains (grep clean
except deleted tree); `mod_loader.cpp` loads mods through `linker_rust_dlopen_ext` + Rust
`__loader_*` dl* function pointers.

---

### Phase 5 — Feature gaps only if required by the game (1–5 days, opportunistic) — **DONE**

**Result (2026-08-03):** audited the real shipped libs; none require Phase 5 features.
`readelf` across `libminecraftpe.so` + all 8 game deps shows **zero** `DT_ANDROID_*` /
`DT_RELR`, zero `R_X86_64_IRELATIVE`, zero TLS relocs, zero `__loader_*`/`dl*` UND
symbols. All relocation work is `RELATIVE`/`JUMP_SLOT`/`GLOB_DAT`, which `reloc.rs`
already handles. Only change: wired `gdb_support::notify_gdb_of_soinfo_load()` into
both load paths so the game + deps appear in gdb `info sharedlibrary`. See
`docs/LINKER_PORTING_PROGRESS.md` Phase 5 section.

Implement only what real loads need; skip pure Android platform features.

| Feature | Needed for Bedrock on Linux? | Action |
|---------|------------------------------|--------|
| Zip / APK-contained `.so` | Usually **no** (extracted `lib/x86_64/`) | Defer; path parser already exists |
| Linker namespaces / `ld.config` | **No** for single-game load | Keep stubs; don’t block deletion |
| CFI shadow wiring | **No** on desktop | Leave unwired |
| Packed Android RELA / RELR | Uncommon in shipping MC libs | Wire if `readelf` shows `DT_ANDROID_*` / `DT_RELR` |
| Full GDB `r_debug` notify | Nice for debugging | Wire `notify_gdb_of_load` on dlopen if cheap |
| `__loader_*` export names | Yes if game/libs call them | Ensure `libdl.rs` symbols are what game binds |

**Exit:** no runtime dependency on missing features for main-menu path.

---

### Phase 6 — Delete C++ linker from the build (1–2 days)

**Prerequisite:** Phase 4 must re-point `mod_loader.cpp` (the last real-ELF C++ `dlopen_ext`
consumer) onto the Rust linker. After that, the bionic loader has *no* user.

1. Remove `linker` and `linker-c` targets from `crates/cpp-bridge-sys/build.rs`.
2. Remove `-llinker` / `-llinker-c` and any IFUNC defsym hacks only needed by bionic from `client/build.rs` (verify still needed for other TUs).
3. Delete or stop compiling:
   - `mcpelauncher-linker/src/linker.cpp`
   - all `bionic/linker/*` sources from cc::Build
   - zip/android-base files **only pulled for linker** (careful: other targets may still need android-base — audit)
4. Remove `mcpelauncher_linker_register_loaded_library`, `mcpelauncher_linker_cpp_init`, `linker_rust_set_dlsym_fallback`, dual-mirror helpers in `capi.cpp`.
5. Optionally keep the **source tree** under `mcpelauncher-linker/` as reference for a while, or move to `third_party/archive/` — not linked.
6. Update docs: `ARCHITECTURE.md` (single linker), `STATIC_LIBS.md`, `STARTUP_FLOW.md`, `AGENTS.md`, `LINKER_PORTING_PROGRESS.md` → mark complete.

**Exit:** `cargo build -p client` produces a binary with **no** bionic linker symbols (`nm` /
`readelf -s` check for `__loader_dlopen` from C++ vs Rust). Main menu still works, and
**mod loading works through `linker_rust_dlopen_ext`** (not nullptr to bionic `dlopen`).

---

## Suggested PR / commit sequence

Small, bisectable steps; each should leave main menu working.

| PR | Content |
|----|---------|
| **PR1** | Phase 0 instrumentation + `RUST_ONLY` abort gate (off by default) |
| **PR2** | Rust soinfo query FFI + HookManager off C++ soinfo |
| **PR3** | `linker_rust_dlopen_ext` returns Rust handle; stop C++ registration |
| **PR4** | Rust-primary stubs; delete mirror for stubs |
| **PR5a–d** | One PR per real dep moved to Rust (sqlite → pairip → libc++ → rest). **fmod is PR5e and comes last** |
| **PR5e** | `libfmod.so` — follow the fmod-specific procedure above; bisect with `MCPELAUNCHER_PATCH_FMOD=0` |
| **PR6** | Remove `DLSYM_FALLBACK` + C++ game fallback |
| **PR7** | Port `loadMinecraftLib` orchestration to Rust |
| **PR8** | mod_loader / hybris / remaining `linker::` call sites |
| **PR9** | Delete C++ linker from build + doc update |

---

## Risk Register

| Risk | Mitigation |
|------|------------|
| Double-mapped deps → half-broken ctors / SIGSEGV | Single owner rule; assert soname uniqueness |
| Unresolved JUMP_SLOTs after dropping C++ fallback | Preload all DT_NEEDED; log first N unresolved; compare against C++ baseline dump |
| Hook loses to self-export (Swappy, FMOD) | Keep hook-preferring resolver; regression log “hook bound vs self” |
| `libfmod.so` ctors/JUMP_SLOT corruption (**hit 2026-08-02**) | Do fmod last; keep C++ fallback entry until Rust image proven resolved; single owner, no mirror; bisect with `MCPELAUNCHER_PATCH_FMOD=0`; see post-mortem |
| `libc++_shared` TLS/IFUNC subtlety | Prefer full `relocate.rs` for real ELFs; run under gdb on first failure |
| HookManager / Baron break on handle type change | Phase 1 before flipping return type; grep all `soinfo_from_handle` |
| Ctor order vs JNI | Keep game init deferred; run dep inits immediately |
| Build still pulls zip/android-base for other libs | Audit `cpp-bridge-sys` includes before mass-delete |
| Docs / agents instruct wrong path | Doc PR with Phase 6 |

---

## Testing Strategy

Project has no CI suite; verification is:

1. **`cargo test -p linker`** after each reloc/TLS change.
2. **`cargo build -p client`** always.
3. **Manual smoke:**  
   `./target/debug/client -dg /path/to/extracted/minecraft` → main menu, mouse/keyboard.
4. **Gate env:** `MCPELAUNCHER_LINKER_RUST_ONLY=1` from Phase 3 onward.
5. **Diagnostics:** `RUST_LOG=linker=info,MinecraftUtils=info` for load order; optional `MCPELAUNCHER_LINKER_VERBOSITY` equivalent on Rust.
6. **Compare maps:** `/proc/self/maps` for single mapping of `libminecraftpe` / `libc++_shared` / `libfmod`.

---

## Effort Estimate

| Phase | Effort | Cumulative |
|-------|--------|------------|
| 0 Instrumentation | 0.5–1 d | ~1 d |
| 1 Handle unification + HookManager | 2–4 d | ~5 d |
| 2 Stub ownership flip | 2–3 d | ~8 d |
| 3 Real dep migration | 3–6 d | ~14 d |
| 4 Orchestrator port | 2–4 d | ~18 d |
| 5 Feature gaps (if needed) | 0–5 d | ~18–23 d |
| 6 Delete C++ linker | 1–2 d | **~3–5 weeks** focused work |

Integration risk dominates; the Rust module port is largely already done.

---

## Non-Goals / Explicit Deferrals

- Porting entire `mcpelauncher-core` (crash handler, patch utils) — only linker-touching parts.
- Full Android namespace / APEX / greylist semantics.
- Zip-in-APK loading unless a real product path needs it.
- Replacing host glibc; Rust linker only loads **Android game** ELFs + stubs.
- Removing FakeJni / other C++ — separate tracks.

---

## Immediate Next Step After Plan Approval

Start **Phase 0 + PR2 design**: inventory every `linker::` and `soinfo_from_handle` call site, add `RUST_ONLY` gate, and sketch the minimal C ABI HookManager needs so Phase 1 can land without a big-bang rewrite of `minecraft_utils.cpp`.
