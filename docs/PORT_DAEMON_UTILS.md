# Port: daemon-utils

**Status:** Rust crate (`crates/daemon-utils/`) is wired into `client` — `msa-daemon-client`'s `ServiceLauncher` (which implements `DaemonLauncher`) launches and connects to the daemon from `crates/client/src/xbox_auth.rs`. The C++ `daemon_launcher.cpp` (target `mcpelauncher-daemon-client-utils`) has been **removed** from `cpp-bridge-sys` and `client/build.rs`, and the `manifest_libs/daemon-utils` + `include/daemon-utils` sources deleted. **Verified against a real daemon** (the Flatpak `msa-daemon` binary): `DaemonLauncher::start` spawns it, waits for the socket, and a following RPC succeeds — see `crates/msa-daemon-client/tests/daemon_e2e.rs`. (`start` detaches the child's stdio so the daemon doesn't inherit the launcher's pipes.)

## Role

Launches and supervises the `mcpelauncher-msa-daemon` process, and provides the generic `LaunchableServiceClient`/`AutoShutdownService` scaffolding around the `simpleipc` transport. Only the client side matters for the game binary.

## C++ to remove (target `mcpelauncher-daemon-client-utils`, 1 file)

| File | Lines | Role |
|------|-------|------|
| `daemon-utils/client/src/daemon_launcher.cpp` | 197 | `daemon_launcher::start()` — fork+`setsid`+`chdir`+`execv` the daemon, then wait for readiness (inotify on the daemon pid/state file; uses `simpleipc/common/io_handler.h`) |

## Existing Rust

`crates/daemon-utils/` — `daemon_launcher.rs` (`DaemonLauncher`), `client.rs` (`LaunchableServiceClient`), `server.rs` (`AutoShutdownService`, `ShutdownPolicy`). Tokio-based (`process`, `net`, `fs`).

## Compatibility notes

- **Spawn semantics must match C++:** fork → `setsid()` (new session) → `chdir(cwd)` → `execv(argv[0])`; pass-through of CWD and args; parent waits for readiness before returning so the first RPC doesn't race the daemon's socket bind.
- **Readiness wait:** C++ uses inotify/`signalfd` on the daemon state file. The Rust `DaemonLauncher` must reproduce the same readiness contract the C++ daemon writes (daemon's own `AutoShutdownService` pings the process).
- **Error propagation:** C++ throws `std::runtime_error` on `fork` failure; the Rust port must map to `ClientError`/`DaemonError` and be catchable by the auth flow.

## Steps

1. **Audit `daemon_launcher.rs` against `daemon_launcher.cpp`.** ✅ DONE — `DaemonLauncher::start` (tokio `Command` spawn + 10s socket-exists wait) replaces C++ fork/`setsid`/inotify; `get_arguments` parity verified. C++ also passes `-d {data}`/`-x`; Rust `ServiceLauncher` matches and now uses the C++ socket path `{data}/service` (was `msa-daemon-ipc.sock` — fixed).
2. **Confirm launch succeeds against a real daemon** ✅ DONE — `daemon_e2e.rs::hello_and_get_accounts` spawns the daemon via `ServiceLauncher`/`DaemonLauncher` and completes `.hello` + `get_accounts`.
3. **Add `daemon-utils` (and its dep `simple-ipc`) to `client/Cargo.toml`.** ✅ DONE.
4. **Wire the launcher into the auth flow.** ✅ DONE — `crates/client/src/xbox_auth.rs` (`launch_and_connect`) tries a direct connect, else launches via `ServiceLauncher::start` and reconnects. `jni/xbox_live.rs` `invokeMSA`/`invokeAuthFlow` call it.
5. **Remove the C++ target.** ✅ DONE — `daemon_launcher.cpp` dropped from `cpp-bridge-sys/build.rs`; `mcpelauncher-daemon-client-utils` removed from `client/build.rs` STATIC_LIBS.
6. **Delete C++ sources/headers.** ✅ DONE — `manifest_libs/daemon-utils/` and `include/daemon-utils/` removed.

## Done when

- `nm` shows no `daemon_utils::` symbols. ✅
- The launcher spawns `mcpelauncher-msa-daemon`, waits for readiness, and a following `simpleipc` RPC succeeds. ✅
- `mcpelauncher-daemon-client-utils` gone from `client/build.rs`. ✅

## Depends on / used by

- Depends on **`simple-ipc`** (must port first).
- Used by **`msa-daemon-client`** (port next).
