# Roadmap to Full Rust

Everything between the current state (Phase 5 done — gamewindow ported; `jni_support.cpp` port pending per `docs/PORT_JNI_SUPPORT.md`) and a build with **zero C++/C compilation**.

## Milestone 0 — Current state

- Game boots to main menu; `mcpelauncher-client-jni` is the only C++ static lib.
- Remaining C++ ≈ 6,600 lines (FakeJni/Baron chain ≈ 5,500 + HTTP/WebSocket 514 + live shims ≈ 500 + `variadic.c`).
- Two JNI VMs coexist (Rust `libjnivm-sys` + C++ FakeJni/Baron) — collapse to one via `docs/PORT_JNI_SUPPORT.md` (Phases 1–5).

## Milestone 1 — `jni_support.cpp` port (unlocks ~5,500 lines)

Follow `docs/PORT_JNI_SUPPORT.md`. Deletes: `jni_support.cpp` (677), `main_activity.cpp` (547), `jni_descriptors.cpp` (305), `jnivm_class_wrappers.cpp` (721), `pulseaudio_stub.cpp`/`uuid_stub.cpp` (59), the FakeJni/LocalFrame wrappers in `jni_bridge_stub.cpp` (shrinks to ~50 lines), `text_input_handler_stub.cpp` (233, "exists only because C++ JniSupport has a TextInputHandler member"), and the libjnivm C++ tree (2,990: `jnivm/*.cpp` + `fake-jni/*` + `baron/jvm.cpp`).

**Gate:** build, tests, `nm -C` zero `FakeJni`/`Baron`/`JniSupport::`/`MainActivity::` symbols, boot to main menu.

## Milestone 2 — HTTP/WebSocket (514 lines, independent — can run in parallel with M1)

Wire the Rust response callbacks in `jni/http_client.rs` + `jni/websocket.rs` (currently "Partial — callbacks not wired"), then delete `lib_http_client.cpp` (290) + `lib_http_client_websocket.cpp` (224) and `http_client_stubs.cpp`.

**Gate:** boot; no network regressions (game is offline anyway); `nm` zero `HttpClientRequest`/`HttpClientWebSocket` C++ symbols.

## Milestone 3 — Dead-stub sweep (bulk delete)

The `*_stub.cpp` files that exist **only** to satisfy FakeJni registration/method linker deps die with Milestone 1:

- `store_stub.cpp`, `jbase64_stub.cpp`, `arrays_stub.cpp`, `asset_manager_stub.cpp`, `package_source_stub.cpp`, `securerandom_stub.cpp`, `signature_stub.cpp`, `accounts_stub.cpp`, `locale_stub.cpp`, `playfab_stub.cpp`, `fmod_stub.cpp`, `webview_stub.cpp`, `shahasher_stub.cpp`, `file_picker_stub.cpp`, `settings_stub.cpp`, `xbox_live_stub.cpp`, `xbox_live_helper_stub.cpp`, `xal_webview_factory_stub.cpp`

The Rust `jni/*.rs` + `jni_support.rs` already provide these natives on the Rust VM. Verify each with a boot-gated deletion (remove a few per commit).

**Gate:** `nm -C` zero symbols for each deleted stub's classes; boot.

## Milestone 4 — Port the live shims (real implementations ≈ 450 lines)

| File | Lines | Action |
|------|-------|--------|
| `fake_assetmanager_stub.cpp` | 214 | Port FakeAssetManager (asset file I/O from game dir) to Rust; `main.rs:185` currently calls the C++ `fake_assetmanager_create_and_set_global` |
| `fake_egl_stub.cpp` | 161 | Verify libEGL.so stub symbols register purely in Rust (`rust_bridge.rs` FakeEGL), then delete the C++ forwarding shim |
| `logger_stub.cpp` | ~40 | `Log::vlog` → Rust `util::logger` |
| `main_stubs.cpp` | 8 | `LauncherOptions` global → Rust (`main.rs` already parses CLI args) |
| `jni_bridge_stub.cpp` | ~50 (after M1) | Process globals + hybris hook registration + `JNI_OnLoad`-era glue → Rust module |

**Gate:** boot with assets loading (main menu textures), EGL/FakeEGL swaps, logging, per-milestone `nm` checks.

## Milestone 5 — Delete `variadic.c`

Replace `crates/libc-shim/src/variadic.c` with the nightly `c_variadic` already used for `android_log_varargs` (`client/android_log_hook.rs`). libc-shim becomes 100% Rust.

**Gate:** boot; no missing variadic libc symbols.

## Milestone 6 — Drop the C++ compiler (true 100% Rust)

- Zero `.cpp`/`.c` in the workspace → delete the `cpp-bridge-sys` crate entirely.
- `client/build.rs`: remove `-lstdc++`, `-lmcpelauncher-client-jni`, C++ include dirs, `cargo:rustc-link-lib=stdc++`; drop `cc` as a build-dependency everywhere.
- Remove `libstdc++-dev` from system deps (`AGENTS.md`).
- `cargo clean` + fresh `cargo build` from scratch to prove no hidden C++ object is linked.

**Gate:** `find crates -name '*.cpp' -o -name '*.c'` returns nothing; `nm -C target/debug/client` shows no C++-mangled symbols (`t _Z…`); clean-tree build + boot.

## Rough totals

| Milestone | Lines removed |
|-----------|---------------|
| 1. jni_support port | ~5,500 |
| 2. HTTP/WebSocket | ~514 |
| 3. Dead-stub sweep | ~600 |
| 4. Live shims | ~450 |
| 5. variadic.c | ~50 |
| 6. cc/static-lib removal | build-system only |
| **Total** | **~7,100 + build system** |

## Doc updates along the way

- `docs/PORTING_PROGRESS.md` — per-file status table, static-libs table, overall-estimate table.
- `docs/CXX_BRIDGE.md` — shrink the bridge-file table + notable-ports as each milestone lands.
- `docs/ARCHITECTURE.md` — drop the libjnivm C++ row; single-VM note.
- `docs/STATIC_LIBS.md` — `mcpelauncher-client-jni` DELETED at M6.
- `AGENTS.md` — system deps, crate table, remaining-C++ status.
