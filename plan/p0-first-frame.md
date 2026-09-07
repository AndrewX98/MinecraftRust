# P0 — first_frame 465ms → ~390ms (4 wins, zero-overhead prod)

Baseline (release --features perf, `MINECRAFT_GAME_DIR=... CLIENT_BIN=target/release/client`):
```
eglut_window 73 | goblin 25 | reloc 74 | linker_load_game 123 | game_start 109 | gap 156 | wall 465
```

## Scope
Only 4 changes. Prod `cargo build -p client --release` must stay `0 PERF lines`, binary size unchanged. All perf instrumentation behind `#[cfg(feature="perf")]`.

## P0-1 — eglut MapNotify busy-wait (35ms) — highest ROI
**File:** `crates/client/src/eglut/compat.rs:117-145`

Current: `for _ in 0..50 { XPending/XNextEvent; if mapped break; sleep 10ms }` waits 20-50ms for WM. History: avoids Mesa black frame until mapped.

Fix:
- `XMapWindow; XFlush;` fire-and-forget. Move `fake_window_set_size(width,height)` to *before* EGL create (already line 147 — move earlier + keep initial `INIT_WIN_W/H` as fallback).
- Delete loop, or keep at most `XSync(dpy, False)` 1 roundtrip or `sleep 1ms x3` gated by `DEBUG_EGLUT_SYNC`.
- `ConfigureNotify` already handled in `crates/client/src/eglut/event.rs:129`, will correct size when WM assigns.
- Guard: add `perf::span("eglut_MapNotifyWait")` before/after to prove saving.

Risk: Medium-Low — game may read initial size before ConfigureNotify; mitigated by eager `fake_window_set_size` + screen-size fallback `eglutScreenWidth/Height:48`.

Verification: `grep -c PERF` prod 0, `--features perf` 19; harness `eglut_window 73→~38` (expect 60% drop).

## P0-2 — goblin → manual ELF header parse (22ms)
**File:** `crates/linker/src/loader.rs:30-68`

Current: `Elf::parse(data)` (25ms) faults every 17.5k pages of 70MB file mmap + allocs `Vec<Phdr/Dyn/Shdr>` + walks SHT unused.

Fix:
- Keep `data: &[u8]` as file mmap slice (already whole-file `MAP_PRIVATE`).
- Read `Elf64_Ehdr` via `*(data.as_ptr() as *const Elf64_Ehdr)` (check `EI_MAG`, `e_type==ET_DYN`, `e_phnum/phoff`).
- `slice::from_raw_parts(data+e_phoff, e_phnum)` cast to `Elf64_Phdr` — no alloc, walk PT_LOAD/TLS/RELRO/DYNAMIC in place.
- Dynamic array walk until `DT_NULL`, only `DT_STRTAB/STRSZ/SYMTAB/GNU_HASH/HASH/JMPREL/PLTRELSZ/RELA/SZ/REL/SZ/INIT* /NEEDED/SONAME`.
- Reuse existing `phdr_flags_to_prot`, `load_elf_with_fd` MAP_FIXED paths unchanged. Keep `LoadError::Parse`.
- Keep `#[cfg(feature="perf")]` span `goblin_parse:<name>` → rename `elf_parse`.

Alternative if risky: `goblin::Elf::parse` with `goblin::elf::Elf::parse_header` lighter helper, but manual is ~50µs vs 25ms.

Risk: Low-Med — must handle `e_machine` x86_64/aarch64, endian `EI_DATA`, truncated file, overflow. Test vs `readelf -d`.

Depends on: none. Unblocks P0-3 hash calc.

Verification: `cargo build -p client --release --features perf` + harness `goblin_parse:libminecraftpe.so 25→~3`.

## P0-3 — reloc HashMap → Vec cache (12ms)
**File:** `crates/linker/src/reloc.rs:67,133`, `crates/linker/src/loader.rs:347`

Current: `HashMap<u32,Option<usize>>::with_capacity(4096)` + SipHash 1-3 per 350k symbol relocs (900k total, ~20k distinct). 2 resizes + `Entry` probe. `last_sym` fast path helps only consecutive duplicates.

Fix:
- Pass `dynsym_count` (already computed `loader.rs:347-360` as `symoffset + chains_len`) into `apply_rela/apply_rel` signature.
- `let mut cache: Vec<Option<Option<usize>>> = vec![None; dynsym_count];` (30k → 240KB). Fallback to HashMap if `r_sym >= len` (corrupt).
- Loop: `if r_sym==last_sym { last_val } else { match &mut cache[r_sym] { Some(v)=>*v, None=>{ let r=resolve_sym; cache[r_sym]=Some(r); r }}}`
- Optional: `rustc-hash FxHasher` if Vec not trusted (saves ~6ms alone).

Risk: Low — `dynsym_count` already derived from `strtab - gnu_hash` adjacency. Guard with `r_sym as usize < cache.len()`.

Verification: `reloc:libminecraftpe.so 74→~60` (14ms). Combined linker `123→~85`.

## P0-4 — AAsset read → mmap + dedup cache (20-35 cold)
**File:** `crates/client/src/fake_assetmanager.rs:46-70`

Current: `std::fs::read(&full_path)` per `AAssetManager_open` — copies whole file into `Vec<u8>`, alloc per open. Game opens hundreds assets at boot (shaders, `*.brarchive`, `subdirs.txt`). No caching; duplicate opens re-read.

Fix (incremental, 3 steps without changing `AAsset` ABI):
1. `mmap` for `>1MB` (or `>64KB`): `open` + `fstat` → `mmap(PROT_READ,MAP_PRIVATE)` vs `read()`. Lazy demand-paged, zero copy. Store `*const u8+len+fd` in `AAsset`, `munmap` on `AAsset_close:95`. Small files keep `read`.
2. `Mutex<LruCache<String, Arc<[u8]>>>` (32MB cap) keyed by `name` for duplicate opens (shader reopened 2×). Insert on first `read`/`mmap` copy, hit returns `Arc::clone`.
3. Optional follow-up (not P0): preload `assets/resource_packs/vanilla/*` in `main.rs:283 asset_manager` span via `rayon` (gated).

Lifetime: mmap lives until `AAsset_close`; `AAsset_getBuffer:168` returns `as_ptr()` unchanged.

Risk: Med — munmap on close must not double-free cached Arc; cache holds Arc, AAsset holds Arc clone.

Verification: cold FS `first_frame gap 156→~125` (instrument `fake_assetmanager.rs:58` with `perf::span` or `eprintln! if dt>1ms`).

## Order & estimates

```
P0-1 (eglut sleep)      -35ms  73→38   1 file, 10 lines deleted   L-M
P0-3 (reloc Vec)         -12ms  74→60  2 fns, + dynsym_count param Low
P0-2 (goblin manual)     -22ms  25→3   loader.rs header rewrite    L-M
P0-4 (asset mmap)        -20ms gap      assetmanager.rs           M
-------------------------------------------------------------
Total P0: wall 465 → ~390 (reloc+goblin 38ms + eglut 35ms overlap = 73ms linker+window, gap 20ms)
P1 follow-ups deliver additional ~25ms to 365 (see prior synthesis).
```

Suggested sequence: P0-1 → P0-3 → P0-2 → P0-4. P0-1+3 are trivial review, land first.

## Measurement protocol

```sh
cargo build -p client --release --features perf
MINECRAFT_GAME_DIR=/home/andrew/.local/MinecraftLauncher/extracted/1.26.20 \
  CLIENT_BIN=target/release/client \
  cargo test -p client --test first_frame -- --ignored --nocapture 2>&1 | grep -E "first_frame|phase|goblin|reloc|eglut"
# prod zero-cost:
cargo build -p client --release
MINECRAFT_GAME_DIR=... RUST_LOG=error timeout 6 target/release/client -dg ... 2>&1 | grep -c PERF  # expect 0
```

Add sub-spans before P0: `eglut_MapNotifyWait`, split `game_start` into `call_init/jni_onload/onCreate` to confirm.

## Out of scope (P1+)
XInternAtoms batch, dedup dlopen libEGL, double register_global_exports, FakeLooper poll timeout, shader binary cache — tracked separately.
