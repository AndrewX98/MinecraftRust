# Port Plan — mcpelauncher-core (9 C++ files → Rust)

**Status:** Linking-only plan. The launcher already runs hybrid: some core logic is in
Rust (`minecraft_load.rs`, `fmod_utils.rs`, `path_helper.rs`, `rust_bridge.rs`
CorePatches), most is still C++ driven through `capi.cpp`. This plan finishes the job
file-by-file while keeping the game reachable at main menu after **every** phase.

## The hybrid mechanism (use in every phase)

Same trick as `PORT_FILE_UTIL.md` / the phase-4 `loadMinecraftLib` port:

1. Create `crates/corelib/src/<name>.rs` exposing **`#[no_mangle] pub extern "C" fn`**
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

- Add a `#[cfg(test)] mod tests` inside each `corelib/*.rs` module. Rust integration
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

- Port to `corelib/minecraft_version.rs`: `static Mutex<Version>` (Android/iOS schema,
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
> **Deletion of `minecraft_version.cpp` + header landed in Phase 6** (with `getApi`):
> the version statics are now `static AtomicI32` mirrors in `minecraft_version.rs` and
> `mc_workaround_locale_bug` is a Rust twin; the C++ file is gone.

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

> **Implementation note (2026-08-04):** additive-only. Pure logic lives in
> `crates/corelib/src/patch_utils.rs` (`parse_pattern`, `scan_pattern`,
> `get_vtable_size`, `patch_call_instruction_bytes`, all unit-tested) and
> `crates/corelib/src/hook.rs` (`translate_constructor_name`, unit-tested). Each
> is also exported as a **clean-named** `#[no_mangle]` extern twin
> (`patternSearch`, `getVtableSize`, `patchCallInstruction`,
> `translateConstructorName`) — no name collision with the still-compiled C++
> mangled symbols (`_ZN10PatchUtils…`, `_ZN11HookManager…`). No caller is
> re-pointed and nothing is deleted here; callers get switched to these twins in
> Phases 5/6 (CorePatches vtable) and 6 (translateConstructorName in
> `minecraft_utils.cpp:459`). `patchCallInstruction`/`patternSearch` twins call the
> Rust linker dispatch (`mcpelauncher_dispatch_get_library_code_region`) and
> `libc::mprotect`. ARM branch intentionally omitted (target is x86_64).

---

## Phase 3 — `hybris_utils` (FFI service wrapper)

65 lines. `loadLibraryOS` = `dlopen` + `dlsym` loop + hand-off to
`linker_load_library_rust`; `stubSymbols` = register a stub with the Rust linker.

- Port to `corelib/hybris_utils.rs`. Dependencies are already Rust (`dlopen`/`dlsym` in
  `libc-shim`, `linker_load_library_rust`).
- Consumers are still C++ (`minecraft_utils.cpp:101-160`) — keep symbol names identical so
  no `.cpp` changes.
- Not really unit-testable (OS dlopen); rely on game-boot as the check. Lower priority but
  tiny and unblocks Phase 6.

> **Implementation note (2026-08-04):** additive-only, same shape as Phase 2. Logic lives in
> `crates/corelib/src/hybris_utils.rs` with pure `collect_symbol_names` (unit-tested: order,
> termination, null list) plus clean-named `#[no_mangle]` twins `mc_hybris_load_library_os`
> (`dlopen` + `dlsym` loop + optional `HybrisSym` overrides → `linker_load_library_rust`) and
> `mc_hybris_stub_symbols`. **Deletion of `hybris_utils.cpp` deferred to Phase 6**: its callers
> (`loadLibM`/`loadFMod`/`setupHybris`/`stubFMod` in `minecraft_utils.cpp`) still link against
> the **mangled** C++ methods `_ZN11HybrisUtils14loadLibraryOSE…`/`_ZN11HybrisUtils11stubSymbolsE…`
> which take C++-only types (`std::string const&`, by-value `std::unordered_map`). Rust cannot
> cleanly emit those mangled names or receive those ABIs, so the twins are re-pointed by the
> Phase-6 port of `minecraft_utils.cpp`. Tests avoid `dlopen`; boot is the check.

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

> **Implementation note (2026-08-04):** scope confirmed by grep — `ModLoader` has **zero
> external callers** (only self/header references), so only `getModDependencies` is ported now;
> `mod_loader.cpp` stays compiled and whole-file deletion + the orchestration twins
> (`loadMod`/`loadModMulti`/`loadModsFromDirectory`) defer to Phase 6, when `getApi`
> (`minecraft_utils`) and `HookManager` (`hook`) are Rust. `crates/corelib/src/mod_loader.rs`
> ships `get_mod_dependencies(path: &Path) -> Result<Vec<String>, String>` using **goblin**
> (already a workspace dep via `linker`) — semantics mirror the C++ `fread` walk: bad
> read/header → error, no `PT_DYNAMIC` → error, otherwise `DT_NEEDED` in file order.
> Unit-tested against a synthesized in-memory ELF64 fixture (Ehdr + PT_DYNAMIC Phdr + 5 Dyn
> entries + strtab) plus error cases (missing file, bad magic, no PT_DYNAMIC, truncated).
> `#[no_mangle] mc_get_mod_dependencies` twin returns a null-terminated array; the C++ mangled
> `_ZN9ModLoader18getModDependenciesE…` stays linked (no collision). The new code is NOT wired
> into the boot path — boot is a regression gate only.

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

> **Implementation note (2026-08-04):** additive-only, same shape as Phases 2–4. The Rust
> singleton `HookManager` lives in `crates/corelib/src/hook_manager.rs` behind
> `OnceLock<Mutex<HookManager>>` (`unsafe impl Send/Sync`: all access through the mutex, raw
> pointers into loaded-lib memory). Faithful port of the data model (arena-backed
> `HookedSymbol` referenced by index from both the manager map and each `LibInfo::hooked_symbols`;
> `HookInstance` is a leaked `Box`, freed by `deleteHook`). **RELA semantics on x86_64** — the
> real `libminecraftpe.so` is `DT_PLTREL=RELA` with 24-byte entries; the C++ default
> (`USE_RELA` unset, 16-byte `Elf64_Rel` stride) mis-walks a 24-byte table (harmless only
> because `applyHooks` is off the boot path). This port reads correct 24-byte entries for both
> `DT_RELA`/`DT_RELASZ` and `DT_JMPREL`/`DT_PLTRELSZ`. Reads the `dynamic` table and `PT_GNU_RELRO`
> via raw offsets; strtab/symtab strings read directly from in-image memory (testable without the
> linker). Clean-named `#[no_mangle]` twins (`hook_manager_add_library`/`remove_library`/
> `create_hook`/`delete_hook`/`apply_hooks`/`find_symbol_index`) coexist with the still-linked
> `_ZN11HookManager*` (verified via `nm`); the extern linker symbols are declared locally and
> resolved at client final link (`#[cfg(test)]` stubs satisfy unit-build linking). Unit tests
> (pure, in-memory): dynamic-offset parse, symbol-name resolution from a synthesized ELF,
> RELA `applyHooks` GOT rewrite (mutation captured into `orig`), unknown-reloc skip, and the
> create/delete hook chain. **`hook.cpp` stayed compiled through Phase 5** — its C++
> callers (`minecraft_utils.cpp`, `mod_loader.cpp`) called mangled `HookManager::instance.*`;
> the flip (and `hook.cpp` deletion) landed in Phase 6 with the `minecraft_utils` port. Boot is
> a regression gate only (CorePatches vtable canary uses `VtableReplaceHelper`, not HookManager).

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

> **Implementation note (2026-08-04):** landed in `crates/corelib/src/minecraft_utils.rs`
> (~430 lines + 5 test fns). `minecraft_utils.cpp` (654), `minecraft_version.cpp` (33),
> `mod_loader.cpp` (204) and `hook.cpp` (302) are **deleted** from the build. Remaining:
>
> - `getLibCSymbols` → Rust `get_libc_symbols` (static `SHIMMED` table from `C_API_CPP`
>   data), `loadLibM` → `load_lib_m`, `setupHybris` → `setup_hybris` (loads libz + registers
>   the 50+ mod API intrinsics). `setupApi`/`getApi` registered via `HybrisUtils::stubSymbols`
>   → Rust `linker::stub_symbols_rust`, keyed into `LIB_HOOK`/`LIB_GLOBAL` hashmaps.
> - `loadMinecraftLib` (the master game loader) → already `minecraft_load.rs` (Phase 4).
> - **`jnivm_register_method` was kept in C++** (user decision): a small `jnivm_mod_api.cpp`
>   shim (option A) so it can build `jnivm::MethodHandle` objects against the real C++
>   `libjnivm` headers. It also carries `mc_mod_log`/`mc_mod_vlog`
>   (`va_list` → `Log::vlog`) and `mc_mod_request_google_credentials` (fork/exec helper).
> - `mc_find_data_file`/`mc_get_preinit_hooks`/`mc_finalize_load` moved to Rust
>   (`PreinitTable` wrapper around the loaded `preinit_hooks` function list).
> - `capi.cpp` re-pointed: `mc_get_libc_symbols` → `core_minecraft_utils_get_libc_symbols`;
>   `load_core_libraries` calls `core_minecraft_utils_register_libc_stub` + `load_lib_m` +
>   `setup_hybris`; `libDir` uses `path_helper_get_abi_dir()`.
> - **Boot bug fixed en route:** passing a `Box<Vec<*const c_char>>` as `const char**`
>   crashed (Vec struct field misread as an array entry); now passes the Vec's contiguous
>   buffer directly. Without the fix, boot segfaulted in `collect_symbol_names`.
> - **Phase-6 note:** duplicate `libc.so` registration is avoided — main.rs no longer
>   calls `linker::load_library` for libc; registration happens once inside
>   `mc_load_core_libraries` via `core_minecraft_utils_register_libc_stub`. **Verified:**
>   boot reaches main menu, `nm` shows zero `_ZN14MinecraftUtils/_ZN9ModLoader/
>   _ZN14MinecraftVersion` symbols, 43 corelib tests pass.

---

## Phase 7 — `hybris_android_log_hook` (tiny, pure level-map first)

53 lines. `__android_log_{print,vprint,write,assert}` → Rust logger. The
`convertAndroidLogLevel` priority map (`hybris_android_log_hook.cpp:16-28`) is
**unit-testable**. The `abort()` in `__android_log_assert` is fine as-is.
- `capi.cpp` takes these addresses for the `liblog.so` stub (`capi.cpp:233-236`) — keep names.

> **Implementation note (2026-08-05):** `hybris_android_log_hook.cpp` **deleted**.
> The priority→`LogLevel` map is single-sourced in Rust (`corelib/android_log_hook.rs`:
> `convert_android_log_level`, `#[no_mangle] mc_android_convert_log_level`) with unit tests.
> Stable Rust cannot define `...`/va_list extern "C" fns, so the three varargs entry points
> moved to a tiny C++ shim (`mcpelauncher-core/src/android_log_varargs.cpp`) that calls the
> Rust level converter; `__android_log_write` (non-varargs) is Rust-owned in
> `client/android_log_hook.rs` via `util::Log`. `capi.cpp` unchanged — the four
> `__android_log_*` externs still resolve at final link (Rust + shim). Final print path was
> already Rust (`Log::vlog` → `mcpelauncher_log_vlog` in `rust_bridge.rs`). 45 corelib tests.

---

## Phase 8 — `fmod_utils` (finish existing Rust twin)

`fmod_utils.rs` already has `setup`/`init_hook`/`set_output_hook`. **Remaining C++ consumer:**
`fake_audio.cpp:218 FmodUtils::setSampleRate(...)`. 

- Add `sample_rate` to the Rust `FmodPointers`/static and expose `#[no_mangle] fn
  set_sample_rate`. Drop `setSampleRate` from `fmod_utils.cpp` (or from `fake_audio.cpp`).
- `initHook` env overrides (`FMOD_DSP_BUFFER_LENGTH` etc.) already covered.
- Remove `fmod_utils.cpp`.

> **Implementation note (2026-08-05):** `fmod_utils.cpp` + both `fmod_utils.h` copies
> **deleted**. The Rust twin (`client/fmod_utils.rs`) now owns the settable `SAMPLE_RATE`
> as an `AtomicI32` (default 48000), read by `init_hook`'s `setSoftwareFormat` call, and
> exposes `#[no_mangle] mc_fmod_set_sample_rate`. The lone C++ consumer,
> `fake_audio.cpp:218`, drops the `fmod_utils.h` include and calls
> `mc_fmod_set_sample_rate(defaultSampleRate)` directly (declared in its own `extern "C"`
> block). `AUDIO_SAMPLE_RATE` env behavior unchanged. `minecraft_load.rs` hook wiring
> untouched. 45 corelib tests.

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
- Re-point `capi.rs` extern block to the Rust `corelib` `#[no_mangle]`s, delete `capi.cpp`,
  drop `mcpelauncher-client-bridge` from `cpp-bridge-sys/build.rs:233`.

---

## Summary table

| Phase | File | Lines | Unit-testable ✅ | Risk | Depends on gone when |
|-------|------|-------|-----------------|------|----------------------|
| 0 | test harness | – | – | – | – |
| 1 | minecraft_version | 33 | ✅ decode/getString | lo | done (Phase 6) |
| 2 | patch_utils + hook (pure fns) | 100+302 | ✅ patternSearch/vtableSize/translateCtor | lo | done |
| 3 | hybris_utils | 65 | – | lo | done (Phase 6) |
| 4 | mod_loader | 204 | ✅ getModDependencies | med | done (Phase 6) |
| 5 | hook (HookManager) | 302 | ✅ dynamic-parse/reloc-rewrite (in-mem) | **hi** | done (Phase 6) |
| 6 | minecraft_utils | 654 | – (symbol-set test) | **hi** | done (Phase 6) |
| 7 | hybris_android_log_hook | 53 | ✅ convertAndroidLogLevel | lo | done |
| 8 | fmod_utils (finish) | 39 | – | lo | done || 9 | crash_handler | 131 | – | lo | none (dormant) |
| 10 | capi.cpp bridge | 275 | – | med | all |

**Ordering rule of thumb:** every phase is allowed only by an earlier phase that has already
moved its dependency to Rust, and every phase leaves `cargo build -p client` green **and**
the game at main menu. Unit tests accumulate on the pure-logic functions (Phases 1, 2, 4, 5,
7);
everything dynamic is gated on the boot canary (Phases 3, 5, 6, 8, 10). This is the same
hybrid keep-the-bridge strategy that already got `PATH_FILE_UTIL`, `loadMinecraftLib`, and the
CorePatches patching over cleanly.