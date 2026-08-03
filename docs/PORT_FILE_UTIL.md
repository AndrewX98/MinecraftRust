# Port: file-util

**Status:** Done. C++ `FileUtil`/`EnvPathUtil` classes deleted; logic lives in Rust
(`crates/util/src/file_util.rs`), exposed to remaining C++ via `file_util_*` /
`env_path_util_*` extern "C" FFI in `crates/client/src/rust_bridge.rs`.

## C++ to remove

| File | Lines | Role |
|------|-------|------|
| ~~`manifest_libs/file-util/FileUtil.cpp`~~ | ~~92~~ | ~~`exists`, `isDirectory`, `getParent`, `mkdirRecursive`, `readFile`~~ — deleted |
| ~~`manifest_libs/file-util/EnvPathUtil.cpp`~~ | ~~119~~ | ~~`getAppDir`, `getWorkingDir`, `getHomeDir`, `getDataHome`, `findInPath`~~ — deleted |

Headers: ~~`crates/client/include/file-util/FileUtil.h`~~ and ~~`EnvPathUtil.h`~~ — deleted.

Consumers: `minecraft_utils.cpp`, `daemon_launcher.cpp`, `xbox_live_stub.cpp`, `service_launcher.h`.

## Existing Rust (`crates/util/src/file_util.rs`)

`FileUtil::{get_parent, exists, is_directory, mkdir_recursive, read_file, read_file_bytes}`
and `EnvPathUtil::{get_app_dir, get_working_dir, get_home_dir, get_data_home, get_cache_home,
find_in_path, find_in_path_with}`.

Brought to C++ parity during the port:
- `get_parent` now mirrors the C++ trailing-slash recursion and consecutive-slash skipping.
- `find_in_path`/`find_in_path_with` mirror the C++ `access(X_OK)` executable check, cwd
  prefixing for empty/relative segments, and the empty-segment→`"."` rule (via `libc::access`).

## FFI (`crates/client/src/rust_bridge.rs`)

`file_util_read_file_rust` (pre-existing), plus `file_util_get_parent`,
`file_util_mkdir_recursive`, `env_path_util_get_app_dir`, `env_path_util_get_data_home`,
`env_path_util_find_in_path`. String returns use the per-thread `cstr_static` cache; the
existing `Box::into_raw` + C-side `free()` convention is used for `file_util_read_file_rust`.

## Steps

1. ✅ Diff Rust `file_util.rs` against both C++ files; match `get_parent`/`findInPath` semantics, add `read_file_bytes`/`libc` dep.
2. ✅ Swap C++ consumers (`minecraft_utils.cpp`, `daemon_launcher.cpp`, `xbox_live_stub.cpp`, `service_launcher.h`) to the Rust FFI.
3. ✅ Delete `mcpelauncher-manifest-libs` target from `cpp-bridge-sys/build.rs` (last user was file-util) + drop it from `client/build.rs`; delete the 2 `.cpp` + 2 headers.

## Done when

- ✅ No `FileUtil::`/`EnvPathUtil::` references remain in C++; `nm` clean of `_ZN8FileUtil`/`_ZN11EnvPathUtil`.
