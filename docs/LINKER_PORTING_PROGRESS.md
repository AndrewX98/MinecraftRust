# Linker Porting Progress — C++ Bionic → Rust

**Repo:** `crates/linker/` (Rust) vs `crates/client/src/mcpelauncher-linker/` + `crates/cpp-bridge-sys/` (C++)

The Rust linker handles initial symbol registration (libc, EGL) and basic `dlopen`/`dlsym`/`dlclose`/`dladdr`/`dlerror`. The C++ bionic linker still drives all game library loading (`libminecraftpe.so`, `libfmod.so`, mods, hooks).

## Legend
- [x] Ported to Rust — functionally complete
- [/] Partially ported — basic API covered, gaps remain
- [ ] Not started — C++ only
- (—) Not applicable / not needed

---

### 1. Core ELF Loading & Program Headers

- `loader.rs` — ELF parse via `goblin`, PT_LOAD mmap, .dynamic extraction, DT_NEEDED deps
- `loader.rs:31` — `.note.gnu.build-id` parsing, SONAME extraction
- C++ counterparts: `linker.cpp` (partial), `linker_phdr.cpp`

### 2. Relocation Processing

- `reloc.rs` — RELA/REL/PLTREL, x86_64 + AArch64
- Supported types: `R_X86_64_RELATIVE`, `R_X86_64_64`, `R_X86_64_GLOB_DAT`, `R_X86_64_JUMP_SLOT`, `R_X86_64_PC32` (and AArch64 equivalents)
- Deferred resolution via symbol resolver closure, `init_array`/`init` call
- C++ counterpart: `linker_relocate.cpp`

### 2b. Packed Relocation Iteration (Android packed relocations)

- [x] `reloc_iter.rs` — Rust port of `linker_sleb128.h` + `linker_reloc_iterators.h` (180 C++ lines → 372 Rust lines incl. tests)
- `Sleb128Decoder` struct — signed LEB128 variable-length integer decoder with `pop_front()`
- `Reloc` struct — `r_offset`, `r_info`, `r_addend` fields
- `for_all_packed_relocs()` — decodes the Android packed relocation format with grouped relocations (offset delta, info, addend grouping)
- 12 tests, all passing
- C++ counterparts: `linker_sleb128.h`, `linker_reloc_iterators.h`
- Note: `linker_relocs.h` (R_GENERIC_* constants) not ported — reloc.rs already uses ELF constants directly via goblin

### 3. Symbol Resolution

- `soinfo.rs` — `SoInfo` struct (name, base, dynamic/symtab/strtab/hash, relocations, TLS, deps, external_symbols, is_stub)
- `symbol.rs` — GNU hash (`find_symbol_gnu()`) + SysV hash (`find_symbol_sysv()`) walkers, name comparison against strtab
- C++ counterparts: `linker_soinfo.cpp`, (partial `linker.cpp`)

### 4. Linker API (dlopen/dlsym/dlclose/dladdr/dlerror)

- `lib.rs` — `LinkerState` with global symbol table + library map (by handle and name)
- Public API: `dlopen()`, `dlopen_ext()`, `dlsym()`, `dlsym_global()`, `dlclose()`, `dladdr()`, `dlerror()`, `load_library()`, `unload_library()`, `get_library_base()`, `get_library_code_region()`
- `dlopen_ext` — stubs for `android_dlextinfo` exist, namespace handling not implemented
- C++ counterpart: `dlfcn.cpp`, (partial `linker.cpp`, `linker_main.cpp`)

### 5. Linker State & Initialization

- `lib.rs:init()` — creates global `LinkerState`, initializes symbol table
- `lib.rs:add_symbols()` — bulk symbol registration for stub libs
- C++ counterparts: `linker_main.cpp`, `linker_globals.cpp`

### 6. Block Allocator

- `block_allocator.rs` — `LinkerBlockAllocator`, mmap-based (4096×100 = 409600B pages), free-list with alignment, purge
- 7 tests, all passing
- C++ counterpart: `linker_block_allocator.cpp`

### 7. Mapped File I/O

- `mapped_file_fragment.rs` — `MappedFileFragment`, page-aligned mmap with offset, Drop unmaps
- 4 tests, all passing
- C++ counterparts: `linker_mapped_file_fragment.cpp`, `mapped_file.cpp`

### 8. Diagnostics & Logging

- `debug.rs` — `linker_log!` macro wrapping `log::debug!`, 1 test
- `dlwarning.rs` — thread-local warning buffer, `add_dlwarning()` / `get_dlwarning()` + C callback, 4 tests
- C++ counterparts: `linker_debug.cpp`, `linker_logger.cpp`, `linker_dlwarning.cpp`

### 9. SDK Version Tracking

- `sdk_versions.rs` — atomic `TARGET_SDK_VERSION` (default 35), get/set, 3 tests
- C++ counterpart: `linker_sdk_versions.cpp`

### 10. Utilities

- `utils.rs` — `format_string()`, `dirname()`, `normalize_path()`, `file_is_in_dir()`, `file_is_under_dir()`, `parse_zip_path()`, `page_start()`/`page_offset()`, `safe_add()`, `split_path()`, `resolve_paths()`/`resolve_path()`, `is_first_stage_init()`
- 12 tests, all passing
- C++ counterpart: `linker_utils.cpp`

### 11. `src/linker.cpp` Wrapper / Orchestrator

- [ ] C++ only — `linker::init()`, `linker::load_library()`, `linker::relocate()` orchestration
- Rust has equivalent primitives in `lib.rs` but they aren't wired as the primary path
- Game startup (`capi.cpp`) calls C++ `linker::init()` → `linker::load_library()` for each core lib → `MinecraftUtils::loadMinecraftLib()`

### 12. Namespace Isolation

- [ ] C++ only — `linker_namespaces.cpp`
- `dlopen_ext` in `lib.rs` stubs namespace params but does nothing with them

### 13. TLS (Thread-Local Storage)

- [x] `tls.rs` — Rust port of `linker_tls.cpp` (152 C++ lines → 245 Rust lines incl. tests)
- `TlsSegment` struct (size, alignment, init_ptr, init_size) added to `soinfo.rs`
- `TlsModule` struct with segment, static_offset, first_generation, soinfo_base pointer
- Global module table (`G_TLS_MODULES: Mutex<Vec<TlsModule>>`), generation counter (`NEXT_GENERATION`)
- Public API: `register_soinfo_tls()`, `unregister_soinfo_tls()`, `get_tls_module()`, `linker_setup_exe_static_tls()`, `linker_finalize_static_tls()`, `static_tls_finished()`
- Tracked on `SoInfo` via `tls_segment: Option<TlsSegment>` + `tls_module_id: usize`
- 12 tests, all passing
- Note: static TLS layout (alignment/reservation) not ported — not needed on Linux; `linker_memory.cpp` (BionicAllocator malloc/free overrides) not ported — Linux uses system allocator

### 14. Control Flow Integrity (CFI)

- [x] `cfi.rs` — Rust port of `linker_cfi.cpp` (294 C++ lines → 370 Rust lines incl. tests)
- Provides `CFIShadowWriter` struct with:
  - Shadow math constants and helper functions (`mem_to_shadow_offset`, `align_up`, `K_SHADOW_SIZE`)
  - `ShadowWrite` RAII helper (prepares data on Vec, copies back on drop — vs C++ no-op destructor)
  - `add_constant`, `add_unchecked`, `add_invalid`, `add` — shadow value computation
  - `map_shadow` — mmaps 2GB sparse shadow region
  - `add_library`, `after_load`, `before_unload`, `initial_link_done` — lifecycle methods
  - `cfi_fail` — resolves `__cfi_check` for a faulting address
- 14 tests, all passing
- Note: `after_load`/`before_unload`/`initial_link_done` use `&[&SoInfo]` slices instead of C++ `soinfo*` linked list; not yet wired into `lib.rs`'s `dlopen`/`dlclose` path

### 15a. ELF Program Headers (phdr)

- [x] `phdr.rs` — Rust port of `linker_phdr.cpp` + `linker_phdr.h` (1291+137 C++ lines → 800 Rust lines incl. 16 tests)
- `ElfReader` struct with `Read()`/`Load()` workflow — reads ELF from file descriptor, loads segments into memory
- Raw `repr(C)` ELF64 types (`Ehdr`, `Phdr`, `Shdr`, `Dyn`) for direct mmap access via `MappedFileFragment`
- Full header validation: magic, class (64-bit), endianness (LE), type (DYN), machine, version, section header size/strndx
- `phdr_table_get_load_size()` — computes load size range from PT_LOAD segments, detects W+X transitions
- `ReserveAligned()` — mmaps PROT_NONE region with kLibraryAlignment (256KB), non-4096 page size fallback
- `LoadSegments()` — file-backed `mmap64(MAP_FIXED)` for each PT_LOAD, BSS zero-fill for `p_memsz > p_filesz`, partial-page clearing for writable segments
- `FindPhdr()` / `CheckPhdr()` — locates loaded program header via PT_PHDR or first PT_LOAD with `p_offset == 0`
- `phdr_table_get_dynamic_section()` / `phdr_table_get_interpreter_name()` — helpers for finding PT_DYNAMIC/PT_INTERP in loaded memory
- `phdr_table_protect_segments()` / `phdr_table_unprotect_segments()` / `phdr_table_protect_gnu_relro()` — no-ops (matching C++ `#if 0` behavior)
- `phdr_table_serialize_gnu_relro()` / `phdr_table_map_gnu_relro()` — functional RELRO serialize/compare-and-remap
- Note: macOS m1 JIT code not ported; ARM exception index not ported (no `__arm__`)
- C++ counterpart: `linker_phdr.cpp`, `linker_phdr.h`

### 15. GDB / JDB Support

- [x] `gdb_support.rs` — Rust port of `linker_gdb_support.cpp` + `rt.cpp` + `linker_logger.cpp` (270 C++ lines → 280 Rust lines incl. tests)
- Provides: `LinkMap`, `RDebug` structures, `rtld_db_dlactivity()` stub, `insert_link_map_into_debug_map`, `remove_link_map_from_debug_map`, `notify_gdb_of_load`, `notify_gdb_of_unload`, `notify_gdb_of_libraries`
- Also provides `LinkerLogger` class (`ResetState`/`Log`/`IsEnabled`), `G_LINKER_LOGGER` global, `G_GREYLIST_DISABLED`, `parse_property`
- Note: `rdebug.r_map` and `_r_debug` stored behind `Mutex` instead of raw `static mut`; `R_DEBUG_TAIL` stored as `usize` (not `AtomicPtr`) to avoid `!Send` pointer issues
- 5 tests, all passing
- C++ counterparts: `linker_gdb_support.cpp`, `rt.cpp`, `linker_logger.cpp`
- Still C++ only: `linker_globals.cpp` (67 lines)

### 16. Linker Stubs & Globals

- [x] `linker_stubs.rs` — Rust port of `linker_globals.cpp` + `linker_libc_support.c` + `linker_libcxx_support.cpp` + `linker_debuggerd_stub.cpp` (309 C++ lines → 220 Rust lines incl. tests)
- Error buffer (`LINKER_ERR_BUF: Mutex<String>`), `linker_get_error_buffer()`, `linker_get_error_buffer_size()`, `linker_set_error()`
- `dl_err!()` / `dl_warn!()` macros, `DlErrorRestorer` RAII struct
- `g_argc`/`g_argv`/`g_envp` → `linker_set_args()`, `linker_argc()`, `linker_argv()`, `linker_envp()` via `OnceLock`
- `linker_debuggerd_init()` — no-op on Linux
- `dl_warn_documented_change!()` — SDK-version-aware deprecation warning
- Note: `atexit`, `__find_icu_symbol`, `__cxa_type_match`, `posix_memalign` stubs intentionally omitted — they would conflict with glibc when linked into the main binary; on Linux the real glibc versions are used instead
- 12 tests, all passing
- C++ counterparts: `linker_globals.cpp/h`, `linker_libc_support.c`, `linker_libcxx_support.cpp`, `linker_debuggerd_stub.cpp/h`

### 17. Linker Configuration

- [x] `linker_config.rs` — Rust port of `linker_config.cpp` + `linker_config.h` (620+188 C++ lines → 540 Rust lines incl. 16 tests)
- `ConfigParser` — line-by-line INI-like parser handling comments, sections `[name]`, property `=` and `+=`
- `parse_config_file()` — finds section matching binary path via `dir.<section>` properties, parses namespaces, links, paths, whitelist
- `Properties` — typed accessor: `get_strings()`, `get_bool()`, `get_string()`, `get_paths()` with `${LIB}`/`${SDK_VER}` expansion and optional `resolve_path()`
- `NamespaceLinkConfig`, `NamespaceConfig`, `Config` — data types matching `linker_config.h`
- `Config::read_binary_config()` — reads config file, parses `.version` file for target SDK, builds namespace configs with links/isolation/visibility/paths/whitelist
- 16 tests, all passing
- Note: returns `Result<Config, String>` (no global singleton) to avoid parallel-test interference; `resolve_path` requires paths to exist on disk
- C++ counterpart: `linker_config.cpp`, `linker_config.h`, `linker_config_test.cpp`

### 18. Zip Archive Reading

- [ ] C++ only — `zip_archive.cpp`, `zip_archive_stream_entry.cc`
- `utils.rs:parse_zip_path()` handles path parsing but no actual zip I/O

### 19. libdl Stubs & `__loader_*` Exports

- `libdl.rs` — Rust port of `libdl.cpp` (178 C++ lines → 319 Rust lines incl. tests)
- Provides `get_dl_symbols()` returning function pointers for:
  - `dlopen`, `dlsym`, `dlclose`, `dladdr`, `dlerror` — wrap the Rust linker's API
  - `dl_iterate_phdr` — iterates loaded libraries, calls callback with `dl_phdr_info`
  - `android_dlopen_ext` — wraps `crate::dlopen_ext()` with hook support
  - `android_get/set_application_target_sdk_version` — delegates to `sdk_versions`
- `init()` now registers these Rust implementations instead of forwarding to `libc::dlopen` etc.
- 20 tests, all passing
- C++ counterpart: `libdl.cpp`

### 20. Android / libc Infrastructure

- [x] `base_strings.rs` — Rust port of `strings.cpp` + `stringprintf.cpp` + `parsebool.cpp` (258 C++ lines → 220 Rust lines incl. tests)
- Provides: `split`, `trim`, `starts_with`, `ends_with`, `starts_with_ignore_case`, `ends_with_ignore_case`, `equals_ignore_case`, `string_replace`, `join`, `join_str`, `parse_bool`
- `StringPrintf`/`StringAppendF` not ported — use Rust's `format!()` directly
- 12 tests, all passing
- C++ counterparts: `strings.cpp`, `stringprintf.cpp`, `parsebool.cpp`
- Still C++ only:
  - `liblog_symbols.cpp` — runtime liblog symbol resolution
  - `properties.cpp` — system property access (ported, `properties.rs`, 22 tests)
  - `async_safe_log.cpp`, `logger_write.cpp` — async-safe logging
  - `threads.cpp` — thread management
  - `file.cpp`, `logging.cpp` — misc helpers

### 21. Runtime Support

- `rtld_db_dlactivity()` stub ported in `gdb_support.rs`
- Still C++ only: IFUNC resolver dispatch (`bionic_call_ifunc_resolver.cpp`)
- Rust `loader.rs` calls `init_array`/`init` but doesn't handle IFUNC or `fini_array`

### 22. C Support (strlcpy/strlcat)

- (—) Not needed — Rust standard library covers string operations

### 23. `dl_iterate_phdr`

- `libdl.rs::dl_iterate_phdr_impl()` — iterates the Rust linker's loaded libraries
- Uses `libc::dl_phdr_info` struct for FFI compat
- Supports early-stop callback return values
- C++ counterpart: `dlfcn.cpp` / `linker.cpp`

### 24. FFI Integration (Rust ↔ C++ bridge)

- `lib.rs` — 3 `#[no_mangle]` extern "C" exports:
  - `linker_init_rust` — called from Rust startup
  - `linker_load_library_rust` — called from C++ bridge (`rust_bridge.rs`, `capi.cpp`) to register symbols with Rust linker
  - `linker_show_state_rust` — debug dump
- C++ side calls these via `extern "C"` declarations in `capi.cpp` / `rust_bridge.cpp`
- Rust linker acts as secondary symbol supplier; C++ linker remains primary loader

### 25. linker_main (Solist, Globals, Path Parsing)

- [x] `linker_main.rs` — Rust port of `linker_main.cpp` active code paths (130 of 765 C++ lines, 26 Rust lines incl. 25 tests)
- Solist functions: `solist_init()`, `solist_add_soinfo()`, `solist_remove_soinfo()`, `solist_get_head()`, `solist_get_somain()`, `solist_get_vdso()`, `solist_set_somain()`, `solist_set_vdso()` — maintain ordered `Vec<Handle>` behind `Mutex` (16 tests)
- Global flags: `g_is_ldd` → `set_is_ldd()`/`is_ldd()`, `g_ld_debug_verbosity` → `set_ld_debug_verbosity()`/`ld_debug_verbosity()` (4 tests)
- LD_PRELOAD: `g_ld_preload_names` → `parse_ld_preload()`, `g_ld_preloads` → `set_ld_preloads()`/`ld_preloads()` (5 tests)
- LD_LIBRARY_PATH: `parse_ld_library_path()` stores in `G_LD_LIBRARY_PATH: Mutex<Vec<String>>` (1 test)
- Path parsing: `split_path()`, `resolve_paths()`, `parse_path()` — standalone helpers replacing `linker_utils.h` deps (5 tests)
- `call_ifunc_resolvers()` — no-op on Linux (no `__rela_iplt_start`/`__rela_iplt_end` symbols in Rust linker)
- `#if 0` functions (`linker_main()`, `__linker_init()`, `__linker_init_post_relocation()`, `ProtectedDataGuard`, `get_executable_info()`, `load_executable()`, `set_bss_vma_name()`, `get_elf_base_from_phdr()`, `get_elf_exec_load_bias()`, `add_vdso()`, `init_link_map_head()`) — not ported (disabled in C++ too)
- Note: functions use `Mutex` statics independent of the main `STATE` — avoids symbol conflicts with C++ linker's global `solist`/`sonext`/`somain`/`solinker`/`vdso` variables; `ProtectedDataGuard` not needed (no mprotect-based data protection in Rust linker)
- C++ counterpart: `linker_main.cpp` (765 lines, 130 active), `linker_main.h` (73 lines)

### 26. Relocation Engine (Full)

- [x] `relocate.rs` — Rust port of `linker_relocate.cpp` (681 C++ lines → ~650 Rust lines incl. 33 tests)
- Provides `relocate()` function — processes `.rela`, `.rel`, and `.plt` (both REL and RELA format) relocation sections
- `ProcessRelocArgs` struct — equivalent of C++ `Relocator` class state + `process_relocation_impl<>`
- `SymCache` — 1-entry symbol lookup cache (matching C++ `Relocator::cache_sym_val`/`cache_sym`/`cache_si`)
- Full x86_64 relocation type support:
  - `R_X86_64_RELATIVE`, `R_X86_64_64` (ABSOLUTE), `R_X86_64_GLOB_DAT`, `R_X86_64_JUMP_SLOT`
  - `R_X86_64_PC32`, `R_X86_64_32`, `R_X86_64_IRELATIVE` (IFUNC resolver calling)
  - `R_X86_64_COPY` — returns `Err(CopyRelocNotSupported)` (Bionic doesn't support it either for PIE)
  - TLS: `R_X86_64_DTPMOD64`, `R_X86_64_DTPOFF64`, `R_X86_64_TPOFF64` — works with `tls::get_all_tls_modules()` for module ID lookup
  - `R_X86_64_TLSDESC` — returns `Err(UnsupportedType)` (only on aarch64 in C++ bionic)
- `RelocationKind` enum + `count_relocation()`/`print_linker_stats()` — optional counting
- `call_ifunc_resolver()` — calls function pointer returned by IFUNC resolver
- Note: excludes Android packed relocations (`DT_ANDROID_REL[A]`) and RELR (`DT_RELR`) — these are rare in non-Android-platform libraries; `version_tracker` not ported (symbol lookup uses simple name resolution instead)
- C++ counterpart: `linker_relocate.cpp` (681 lines), `linker_relocate.h` (69 lines)

### 27. libdl API (dlopen/dlsym/…)

- [x] `libdl.rs` — Rust port of `dlfcn.cpp` (348 C++ lines → ~530 Rust lines incl. 33 tests)
- Core functions: `dlopen`, `dlsym`, `dlvsym`, `dlclose`, `dladdr`, `dlerror`, `dl_iterate_phdr`
- Android extensions: `android_dlopen_ext`, `android_dlwarning`, `android_get/set_application_target_sdk_version`, `android_get/update_LD_LIBRARY_PATH`
- Namespace stubs: `android_init_anonymous_namespace`, `android_create_namespace`, `android_link_namespaces`, `android_link_namespaces_all_libs`, `android_get_exported_namespace` — stub (return null/false) until full namespace wiring is ported
- Other stubs: `cfi_fail`, `add_thread_local_dtor`, `remove_thread_local_dtor` — no-ops for now
- `get_dl_symbols()` — returns a HashMap of all 22 function pointers, registered at linker init
- Thread-local error handling via `DL_ERROR` thread-local (matching bionic's `__bionic_set_dlerror`/`__bionic_format_dlerror`)
- Note: `get_libdl_info()` not ported — creates a synthetic `soinfo` for the linker itself; Rust linker registers libdl symbols directly via `get_dl_symbols()`
- C++ counterpart: `dlfcn.cpp` (348 lines) — all `__loader_*` extern "C" exports

### 28. soinfo Lifecycle Methods

- [x] `soinfo.rs` — Rust port of `linker_soinfo.cpp` (982 C++ lines → ~390 Rust lines incl. 24 tests)
- Flag constants + bitfield: `FLAG_LINKED`, `FLAG_EXE`, `FLAG_LINKER`, `FLAG_GNU_HASH`, `FLAG_MAPPED_BY_CALLER`, `FLAG_IMAGE_LINKED`, `FLAG_NEW_SOINFO`, `FLAG_PRELINKED`, `FLAG_RESERVED`
- Flag helpers: `is_linked()`/`set_linked()`, `is_image_linked()`/`set_image_linked()`, `is_gnu_hash()`, `is_main_executable()`/`set_main_executable()`, `is_linker()`/`set_linker_flag()`, `is_mapped_by_caller()`/`set_mapped_by_caller()`
- State: `constructors_called`, `call_pre_init_constructors()`, `call_init_functions()`, `call_fini_functions()` — unsafe wrappers for DT_INIT/DT_INIT_ARRAY/DT_FINI/DT_FINI_ARRAY/DT_PREINIT_ARRAY
- Ref counting: `get_ref_count()`, `increment_ref_count()`, `decrement_ref_count()` — saturating, no underflow
- Dependencies: `children: Vec<Handle>`, `parents: Vec<Handle>`, `add_child()`, `remove_all_links()`
- DT flags: `set_dt_flags_1()` — converts `DF_1_GLOBAL` → `RTLD_GLOBAL`, `DF_1_NODELETE` → `RTLD_NODELETE`; `set_nodelete()`, `can_unload()`
- Handle: `get_handle()` — stored inline, matching `LinkerState`-assigned values
- Version: `has_min_version()` support via inline `version` field; st_dev/st_ino/file_offset accessors
- New struct fields: `load_bias`, `flags: u32`, `constructors_called`, `children`, `handle`, `local_group_root`, `ref_count`, `st_dev`, `st_ino`, `file_offset`, `init_func`, `fini_func`
- Note: `generate_handle()` not ported (Rust linker assigns handles sequentially via `LinkerState::next_handle`); `SymbolLookupList`/`SymbolLookupLib`/`soinfo_do_lookup` not ported (symbol resolution lives in `symbol.rs`)
- C++ counterpart: `linker_soinfo.cpp` (982 lines), `linker_soinfo.h`

### 29. Integration: Rust Linker as Primary

- [x] `linker_init_rust()` now initializes BOTH Rust and C++ linker states — calls C++ `mcpelauncher_linker_cpp_init()` (wraps `linker::init()`) then Rust `init()`
- [x] `capi.cpp` `mc_load_core_libraries()` calls `linker_init_rust()` instead of `linker::init()` — Rust linker is the primary entry point
- [x] C++ bionic linker state still initialized as a side effect, keeping game library loading (`do_dlopen`, `__loader_*`) functional
- [x] `mcpelauncher_linker_cpp_init()` added to `src/linker.cpp` as an `extern "C"` wrapper for Rust FFI call
- [ ] All `linker::load_library()` calls in `capi.cpp` still use C++ (handles between Rust and C++ are incompatible — Rust uses simple integers, C++ uses soinfo pointers)
- [ ] `linker::relocate()` in `capi.cpp` still uses C++ (operates on C++ soinfo handles)
- [ ] Future: make `linker_load_library_rust()` and `linker_relocate_rust()` the primary calls once handle representation is unified

## Summary

| Area | Status | Rust files | C++ files |
|------|--------|-----------|-----------|
| ELF loading | [x] | `loader.rs`, `phdr.rs` | `linker.cpp`, `linker_phdr.cpp` |
| Relocations | [x] | `reloc.rs`, `relocate.rs` | `linker_relocate.cpp` |
| Packed reloc iter |  | `reloc_iter.rs` | `linker_sleb128.h`, `linker_reloc_iterators.h` |
| Symbols | [x] | `symbol.rs`, `soinfo.rs` | `linker_soinfo.cpp` |
| API (dlopen/dlsym/…) | [x] | `libdl.rs`, `lib.rs` | `dlfcn.cpp` |
| State & init |  | `lib.rs`, `linker_stubs.rs` | `linker_globals.cpp` |
| Stubs & globals | [x] | `linker_stubs.rs` | `linker_globals.cpp/h`, `linker_libc_support.c`, `linker_libcxx_support.cpp`, `linker_debuggerd_stub.cpp/h` |
| Block allocator |  | `block_allocator.rs` | `linker_block_allocator.cpp` |
| Mapped file I/O |  | `mapped_file_fragment.rs` | `linker_mapped_file_fragment.cpp`, `mapped_file.cpp` |
| Diagnostics |  | `debug.rs`, `dlwarning.rs` | `linker_debug.cpp`, `linker_logger.cpp`, `linker_dlwarning.cpp` |
| SDK versions |  | `sdk_versions.rs` | `linker_sdk_versions.cpp` |
| Utilities |  | `utils.rs` | `linker_utils.cpp` |
| Orchestrator wrapper |  | — | `src/linker.cpp` |
| Namespaces |  | `namespaces.rs` | `linker_namespaces.cpp` |
| TLS |  | `tls.rs`, `soinfo.rs` | `linker_tls.cpp`, `linker_memory.cpp` |
| CFI |  | `cfi.rs` | `linker_cfi.cpp` |
| GDB support |  | `gdb_support.rs` | `linker_gdb_support.cpp`, `rt.cpp`, `linker_logger.cpp` |
| Config | [x] | `linker_config.rs` | `linker_config.cpp`, `linker_config.h`, `linker_config_test.cpp` |
| Zip archive |  | — | `zip_archive.cpp`, `zip_archive_stream_entry.cc` |
| libdl stubs |  | `libdl.rs` | `libdl.cpp` |
| Android infra |  | `properties.rs`, `base_strings.rs` | liblog, properties, strings, threads, … |
| Runtime support |  | `gdb_support.rs` (rtld_db_dlactivity) | `bionic_call_ifunc_resolver.cpp` |
| dl_iterate_phdr |  | `libdl.rs` | `linker.cpp`, `dlfcn.cpp` |
| FFI bridge |  | `lib.rs` (3 exports) | `capi.cpp`, `rust_bridge.cpp` |
| Solist / Path parsing / Globals | [x] | `linker_main.rs` | `linker_main.cpp` (130 active lines) |
| Primary linker | [x] | `lib.rs` (`linker_init_rust`) | C++ linker drives game loading |
