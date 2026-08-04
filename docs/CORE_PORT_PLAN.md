# Port Plan — mcpelauncher-core (9 C++ files → Rust)

**Status:** Linking-only plan. The launcher already runs hybrid: some core logic is in
Rust (`minecraft_load.rs`, `fmod_utils.rs`, `path_helper.rs`, `rust_bridge.rs`
CorePatches), most is still C++ driven through `capi.cpp`. This plan finishes the job
file-by-file while keeping the game reachable at main menu after **every** phase.

## The hybrid mechanism (use in every phase)

Same trick as `PORT_FILE_UTIL.md` / the phase-4 `loadMinecraftLib` port:

1. Create `crates/client/src/mcpe_core/<name>.rs` exposing **`#[no_mangle] pub extern "C" fn`**
   with the **exact same symbol names** the C++ file exports (so `capi.cpp` and the other
   still-C++ files link unchanged).
2. Remove that `.cpp` from the `mcpelauncher-core` list in `cpp-bridge-sys/build.rs`
   (`core_sources`, around line 240).
3. Build + run to main menu. The Rust twin now satisfies every caller.
4. Verify no dangling symbols: `nm -C <target>/debug/deps/*.rlib | grep <old-name>` clean.
5. Only then delete the `.cpp` and its `include/mcpelauncher/<name>.h`.

Keep the guardrail from `plan.md`: **a phase is not done unless `cargo build -p client`
passes and the game reaches the main menu.**

## Dependency graph (who calls what — grounds the ordering)

```
capi.cpp ──► MinecraftUtils::{getLibCSymbols,loadLibM,setupHybris}
          ──► MinecraftVersion::init
          ──► __android_log_*            (hybris_android_log_hook.cpp)

minecraft_utils.cpp ──► HybrisUtils::{loadLibraryOS,stubSymbols}
                    ──► HookManager (createHook/addLibrary/removeLibrary/deleteHook/applyHooks)
                    ──► MinecraftVersion  (publishes package_version_* into getApi)
                    ──► FmodUtils
                    ──► PatchUtils (indirect, via rust_bridge CorePatches)
                    ──► path_helper_* / env_path_util_*   (already Rust)
                    ──► get_shimmed_symbols / linker_rust_* / mcpelauncher_dispatch_*  (already Rust)

mod_loader.cpp ──► HookManager::instance, MinecraftUtils::getApi, linker_rust_* 
patch_utils.cpp ──► mcpelauncher_dispatch_dlsym / get_library_code_region
hook.cpp       ──► mcpelauncher_dispatch_*, linker_rust_*, mcpelauncher_linker_*
fmod_utils.cpp ──► mcpelauncher_dispatch_dlsym ; fake_audio.cpp:218 (setSampleRate)
crash_handler.cpp ──► mcpelauncher_dispatch_dladdr   (currently NOT called anywhere)
```

Read of the 9 files + call sites (2026-08-04) → see above mapping; `CrashHandler` and
`ModLoader` have **no active caller** outside their own file — viable candidates to defer
or delete wholesale.

---

## Phase 0 — Test harness (do once, before anything)

Goal: the only phase where you build test infrastructure instead of game code.

- Add a `#[cfg(test)] mod tests` inside each `mcpe_core/*.rs` module. Rust integration
  tests are compiled into the `client` binary's test harness; C++ `mc_*` wrappers are not
  exercised by these tests, so keep tests on **pure logic only**.
- For pure logic, assert against golden values **derived** from the C++ now (do it while
  the C++ still runs), then the test pins future Rust regressions.
- Crate-level: consider a tiny `crates/mcpe-version/` or keep under `client` — under client
  is fine (matches `fmod_utils.rs`).

**Check:** `cargo test -p client` compiles and runs (may be empty initially).

---

## Phase 1 — `minecraft_version` (pure logic, test FIRST)

33 lines. Statics + `init` decode + `getString`. **No I/O, no linker, no JNI.**

- Port to `mcpe_core/minecraft_version.rs`: `static Mutex<Version>` (Android/iOS schema,
  950000000–990000000 and 1950000000–1990000000 branches from `minecraft_version.cpp:17-27`).
- C++ currently publishes the statics to mods via `getApi` (`mcpelauncher_package_version_*`);
  the Rust twin must keep those symbols available.
- Re-point `capi.rs mc_init_version` → Rust; drop `MinecraftVersion::init` from `capi.cpp`.
- Remove `minecraft_version.cpp` + header from build.

**Unit-testable: YES** (`#[test] decode("962112004") == "1.21.120.4"`, non-android passthrough).

> **Implementation note (2026-08-04):** landed in `crates/corelib/src/minecraft_version.rs`
> (`decode`, `get_string`, `#[no_mangle] mc_init_version` reusing the `mc_init_version` ABI).
> `capi.rs::init_version` and `main.rs` now go through the Rust twin; `capi.cpp`'s
> `mc_init_version` def + `MinecraftVersion` fwd-decl removed.
> **Deletion of `minecraft_version.cpp` + header is deferred to Phase 6**: its data statics
> (`major/minor/patch/revision/code/package`) are still linked by `minecraft_utils.cpp:526-531`
> (`mcpelauncher_package_*` → `getApi`). Removing it now breaks the link. Rust keeps the tested
> decode; the C++ file remains as a data-symbol stub until `getApi` is ported.

---

## Phase 2 — pure-logic halves of `patch_utils` and `hook` (mid-phase wins)

Do the *testable functions* of two files before the *stateful* halves, so you get two more
test seams cheaply.

`patch_utils.rs`:
- `patternSearch` (pure byte/`?` mask scan, `patch_utils.cpp:14-43`) — **unit-testable**
  (feed a byte buffer + pattern, assert match offset / null).
- `getVtableSize` (scan-for-null, `patch_utils.cpp:95-99`) — **unit-testable**.
- `patchCallInstruction` (arch x86/arm branch emit, `patch_utils.cpp:45-78`) — testable on a
  scratch `mmap` page on x86_64.

`hook.rs`:
- `translateConstructorName` (`hook.cpp:277-303`) — **unit-testable** (`_Z...N...C2...` → `C1`).

Ship these functions behind `#[no_mangle]` extern names now; leave the stateful
`HookManager` singleton and `VtableReplaceHelper` C++ for Phase 5/6.

**Check:** these three `#[test]`s pass; game still boots (you only *added* Rust twins).

---

## Phase 3 — `hybris_utils` (FFI service wrapper)

65 lines. `loadLibraryOS` = `dlopen` + `dlsym` loop + hand-off to
`linker_load_library_rust`; `stubSymbols` = register a stub with the Rust linker.

- Port to `mcpe_core/hybris_utils.rs`. Dependencies are already Rust (`dlopen`/`dlsym` in
  `libc-shim`, `linker_load_library_rust`).
- Consumers are still C++ (`minecraft_utils.cpp:101-160`) — keep symbol names identical so
  no `.cpp` changes.
- Not really unit-testable (OS dlopen); rely on game-boot as the check. Lower priority but
  tiny and unblocks Phase 6.

---

## Phase 4 — `mod_loader` (I/O separated; dependency parse is pure)

204 lines. Split port:

- **First** `getModDependencies` (`mod_loader.cpp:133-204`): pure ELF Ehdr/Phdr/Dyn/DT_NEEDED
  parse from a file path. **Unit-testable** if you give it a fixture `.so` (a copy of
  `libminecraftpe.so` or a synthesized minimal ELF). This is the cleanest remaining test seam.
- **Then** `loadMod`/`loadModMulti`/`loadModsFromDirectory`: orchestration over the already-
  Rust `linker_rust_dlopen_ext/dlsym/dlclose` + `MinecraftUtils::getApi` + `HookManager`.
  Fully dynamic — gate on game-boot, not unit test.
- No active external caller today (only self + header), so the whole file can be ported and
  deleted in one phase if preferred; the sub-split exists purely to add `getModDependencies`
  tests.
- The FakeJni `attachLibrary` call (`mod_loader.cpp:59`) depends on the JNI VM — see
  phase note in Phase 5 about `HookManager`/`jnivm`.

---

## Phase 5 — `hook` (HookManager) — the hard core

302 lines. ELF reloc patching (`applyHooks` rewrites GOT/JMP slots in memory), a
`LibInfo` per loaded lib, `dependents` graph, `HookedSymbol` chain (`createHook`/`deleteHook`).
Depends on `mcpelauncher_dispatch_*` + `linker_rust_*` + `mcpelauncher_linker_resolve_rust_handle`
(all already Rust), but needs C++-free data structures (no `std::shared_ptr`/`std::map`).

- `translateConstructorName` done in Phase 2.
- The singleton `HookManager::instance` state (`libs`, `dependents`, `hookedSymbols`) becomes a
  Rust `static Mutex<HookManager>` (or similar). Rewrite LibInfo parsing (dynamic → strtab/symtab/
  interp rel/relro) and `applyHooks` reloc-application in safe-ish unsafe Rust.
- **Not unit-testable on host** (needs a loaded, relocatable image). **Testable seam:**
  `run-and-reach-main-menu` + the existing CorePatches vtable patch (`rust_bridge.rs:249`) as a
  canary that hooking still works.
- Highest risk. Consider keeping the C++ `hook.cpp` compiled one extra phase while its Rust
  twin exists, and flip the entry only after a boot-canary passes (A/B aversion to the
  phase-3.4 style breakage).

---

## Phase 6 — `minecraft_utils` (funnel + getApi) — the last big funnel

654 lines, but a large portion is already ported (`loadMinecraftLib` → `minecraft_load.rs`,
`fmod` → `fmod_utils.rs`, path helpers → Rust). Remaining C++:

- `getLibCSymbols` → mostly forwards to Rust `get_shimmed_symbols`; thin.
- `loadLibM` / `loadFMod` / `setupHybris` / `stubFMod` → thin over `HybrisUtils` (Phase 3).
- `setupApi`/`getApi` → the 50+ mod-facing intrinsics (`mcpelauncher_hook`, `_hook2`,
  `_patch`, `_relocate`, `_load_library`, `_request_google_credentials`, `jnivm_register_method`
  + the `ModHandle`/`createModFunction` jvalue-dispatch templates `minecraft_utils.cpp:299-593`)
  — the big, symbol-dense, `jnivm::MethodHandle`-dependent chunk. Port last among this file.
- `preinitHooks` finalize logic → already half-portable via `mc_get_preinit_hooks` /
  `mc_finalize_load`.
- Depends on `jnivm` (the Rust JNI VM) for `jnivm_register_method` — coordinate with the JNI
  port (`libjnivm-sys`) so new method registrations use the **active** Rust VM, not FakeJni.

**Not unit-testable** (mod API tables over the live VM); gate on boot + a `getApi` symbol-
presence assertion (Rust test listing required `mcpelauncher_*` keys).

---

## Phase 7 — `hybris_android_log_hook` (tiny, pure level-map first)

53 lines. `__android_log_{print,vprint,write,assert}` → Rust logger. The
`convertAndroidLogLevel` priority map (`hybris_android_log_hook.cpp:16-28`) is
**unit-testable**. The `abort()` in `__android_log_assert` is fine as-is.
- `capi.cpp` takes these addresses for the `liblog.so` stub (`capi.cpp:233-236`) — keep names.

---

## Phase 8 — `fmod_utils` (finish existing Rust twin)

`fmod_utils.rs` already has `setup`/`init_hook`/`set_output_hook`. **Remaining C++ consumer:**
`fake_audio.cpp:218 FmodUtils::setSampleRate(...)`. 

- Add `sample_rate` to the Rust `FmodPointers`/static and expose `#[no_mangle] fn
  set_sample_rate`. Drop `setSampleRate` from `fmod_utils.cpp` (or from `fake_audio.cpp`).
- `initHook` env overrides (`FMOD_DSP_BUFFER_LENGTH` etc.) already covered.
- Remove `fmod_utils.cpp`.

---

## Phase 9 — `crash_handler` (dormant; delete or port last)

Has **no active caller** (verified by grep — only its own header/src). Two choices:
- **Simplest:** drop `crash_handler.cpp` + header entirely if `CrashHandler` is truly
  unreferenced and you accept losing the host stack dumper. It wasn't wired anywhere.
- If you want backtraces: port `registerCrashHandler` to a Rust signal handler that calls the
  Rust linker's `dladdr` for symbolization.

Do this last; it's independent of everything and non-blocking.

---

## Phase 10 — delete the `capi.cpp` bridge

Only after `capi.rs mc_*` functions are all Rust-backed:
- `mc_init_version` → Phase 1.
- `mc_get_libc_symbols` → Phase 6.
- `mc_load_core_libraries`, `mc_setup_android_hooks`, `mc_create_window_and_setup_graphics`,
  `mc_egl_swap_buffers`, `mc_dlsym` → already orchestrated from Rust (`minecraft_load.rs`,
  eglut, rust_bridge); strip remaining `MinecraftUtils::`/`HybrisUtils::` calls.
- Re-point `capi.rs` extern block to the Rust `mcpe_core` `#[no_mangle]`s, delete `capi.cpp`,
  drop `mcpelauncher-client-bridge` from `cpp-bridge-sys/build.rs:233`.

---

## Summary table

| Phase | File | Lines | Unit-testable ✅ | Risk | Depends on gone when |
|-------|------|-------|-----------------|------|----------------------|
| 0 | test harness | – | – | – | – |
| 1 | minecraft_version | 33 | ✅ decode/getString | lo | 6 (getApi) |
| 2 | patch_utils + hook (pure fns) | 100+302 | ✅ patternSearch/vtableSize/translateCtor | lo | 5/6 |
| 3 | hybris_utils | 65 | – | lo | 6 |
| 4 | mod_loader | 204 | ✅ getModDependencies | med | 5,6, jnivm |
| 5 | hook (HookManager) | 302 | – (boot canary) | **hi** | 6 |
| 6 | minecraft_utils | 654 | – (symbol-set test) | **hi** | jnivm |
| 7 | hybris_android_log_hook | 53 | ✅ convertAndroidLogLevel | lo | capi |
| 8 | fmod_utils (finish) | 39 | – | lo | fake_audio |
| 9 | crash_handler | 131 | – | lo | none (dormant) |
| 10 | capi.cpp bridge | 275 | – | med | all |

**Ordering rule of thumb:** every phase is allowed only by an earlier phase that has already
moved its dependency to Rust, and every phase leaves `cargo build -p client` green **and**
the game at main menu. Unit tests accumulate on the pure-logic functions (Phases 1, 2, 4, 7);
everything dynamic is gated on the boot canary (Phases 3, 5, 6, 8, 10). This is the same
hybrid keep-the-bridge strategy that already got `PATH_FILE_UTIL`, `loadMinecraftLib`, and the
CorePatches patching over cleanly.