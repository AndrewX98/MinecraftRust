# Port: mcpelauncher-common (path_helper)

**Status:** Done. C++ `PathHelper` class deleted; state and resolution live in Rust
(`crates/client/src/path_helper.rs`), exposed to remaining C++ via a `path_helper_*`
extern "C" FFI surface. `-dg`/`-dd`/`-dc` + XDG defaults + cwd-special-case +
`DEV_EXTRA_PATHS` (runtime dirs) + `share/mcpelauncher` fallback all match the C++
`PathInfo` semantics.

## C++ to remove

| File | Lines | Role |
|------|-------|------|
| ~~`manifest_libs/mcpelauncher-common/path_helper.cpp`~~ | ~~195~~ | ~~`PathInfo::getPath(PathType)` → data/cache/game dir resolution, ABI dir~~ — deleted |

Header: ~~`crates/client/include/mcpelauncher-common/mcpelauncher/path_helper.h`~~ — deleted.

Consumers: `jni_support.cpp`, `hybris_utils.cpp`, `minecraft_utils.cpp`, `window_callbacks_stub.cpp`, `xbox_live_stub.cpp`, `jni_bridge_stub.cpp`, `capi.cpp`.

## Existing Rust (`crates/client/src/path_helper.rs`)

`get_app_dir`, `get_primary_data_directory`, `get_cache_directory`, `get_game_dir`, `set_game_dir`, `set_data_dir`, `set_cache_dir`, `file_exists`, `get_parent_dir`, `get_working_dir`, `find_data_file`, `find_game_file`, `find_all_data_files`, `get_abi_dir` — plus `path_helper_*` extern "C" exports (`get/set/find_*`) consumed by the C++ stubs.

## Steps

1. ✅ Verify the Rust `get_abi_dir`/dir-resolution matches `PathInfo` enum semantics (`-dd`/`-dc`/`-dg` CLI overrides + XDG defaults).
2. ✅ Swap the 7 C++ consumers to Rust (`path_helper.rs`), threading the `PathInfo` struct through the FFI surface.
3. ✅ Delete `mcpelauncher-common` target from `cpp-bridge-sys/build.rs` + `path_helper.cpp`.

## Done when

- ✅ `-dg`/`-dd`/`-dc` behave identically; no `PathInfo`/`path_helper` symbols in `nm`.
