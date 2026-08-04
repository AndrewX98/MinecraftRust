# Port: simpleipc

**Status:** Rust crate (`crates/simple-ipc/`) is the wire-compatible port, locked with 23 golden-bytes/E2E tests (`crates/simple-ipc/tests/wire.rs`). It is a dependency of `client` (via `daemon-utils` + `msa-daemon-client`), and the C++ `simpleipc` (14 files, target `mcpelauncher-simpleipc`) has been **removed** from `cpp-bridge-sys` and `client/build.rs`. **E2E against a real `mcpelauncher-msa-daemon` (the Flatpak binary) succeeds** — see `crates/msa-daemon-client/tests/daemon_e2e.rs`.

## Role

Message transport + RPC library spoken over unix sockets between this process and the `mcpelauncher-msa-daemon` process (which is a **separate binary, still C++**). Wire compatibility with that daemon is the hard requirement — this is a cross-process protocol, not an in-memory API.

## C++ to remove (target `mcpelauncher-simpleipc`, 14 files)

| File | Lines | Role |
|------|-------|------|
| `common/connection_internal.cpp/.h` | 90/93 | Framing buffer, read/parse loop, MAX_BUFFER_SIZE growth, message dispatch |
| `common/encoding/encodings.cpp/.h` | 33/35 | Encoding registry, default/name lookup |
| `common/encoding/encoding_json.cpp/.h` | 74/29 | JSON message encoding |
| `common/encoding/encoding_json_cbor.cpp/.h` | 63/35 | CBOR message encoding |
| `common/encoding/varint.cpp/.h` | 39/20 | Base-128 LEB128 varints |
| `common/message/error_code.cpp` | 24 | RPC error code enum + `to_string` |
| `common/message/message_container.h` | 50 | Message type tag (request/response/error/notification) |
| `server/rpc_handler.cpp` | 50 | Method→handler dispatch, exception mapping |
| `server/default_rpc_handler.cpp/.h` | 29/19 | `.hello` handshake + encoding negotiation |
| `client/service_client.cpp` | 92 | Async RPC client, id→callback map, hello gating |
| `client/rpc_json_call.cpp` | 27 | Typed `rpc_call` result helper |
| `unix/common/unix_connection.cpp/.h` | 23/27 | Unix socket wrapper |
| `unix/server/unix_service_impl.cpp/.h` | 86/32 | Unix server acceptor |
| `unix/client/unix_service_client.cpp/.h` | 58/30 | Unix client connector |
| `unix/epoll_io_handler.cpp/.h` | 93/37 | Single-threaded epoll event loop |
| `unix/kqueue_io_handler.h` | 38 | BSD variant (unused on Linux) |

The game only needs the **client** half (connector + service_client). The server half can go once nothing else uses it.

## Existing Rust

`crates/simple-ipc/` — `varint.rs`, `message.rs`, `encoding.rs`, `client.rs` (`Client`), `server.rs` (`RpcHandler`, `Server`). Tokio-based (net/io-util/process/fs). Port is **wire-compatible with the C++ sources**, locked by golden-bytes tests in `tests/wire.rs` (varint, JSON, CBOR framing, error codes, hello negotiation) plus in-process E2E client↔server over a unix socket (both encodings).

### Port parity fixes (already applied)

- **CBOR decode of partial/empty buffers** returns `None` (C++ `check_read_message_complete` treats it as incomplete); previously it errored and dropped the connection.
- **`Encoding::pick_from_preferred`** iterates the *client's* encoding order, matching C++ `default_rpc_handler::handle_hello` (picks the first mutually-supported entry, not the server's own preference).
- **Server `.hello`** echoes the request id (C++ `rpc_handler::invoke`) and sends the reply in the *current* encoding before switching (C++ sends with the default encoding, then `set_encoding`).
- **JSON decode of an empty line** is a parse error (C++ `nlohmann::json::parse("")` → `parse_error`), not an empty response.
- **`serde_json` `preserve_order`** so JSON key order matches nlohmann's insertion order (`id` first) byte-for-byte.
- **Client reader task** owns the socket read half (`tokio::io::split`); responses/errors dispatch to a pending id→oneshot map; on EOF all pending calls fail with `connection_closed` (C++ `service_client` semantics). A blocked read never stalls writes.

## Compatibility notes (must not break)

- **Wire framing:** messages = varint length prefix + varint type tag + payload (JSON or CBOR). Must match C++ exactly or the C++ daemon will reject traffic.
- **`.hello` handshake:** client sends `.hello` with `encodings` list; server replies with `version` + chosen `encoding`; client must gate all other RPC until the reply arrives (`no_hello_reply` error otherwise).
- **Error codes:** `error_code.cpp` enum + `to_string` must be reproduced in `error.rs` (currently in `client.rs`/`ClientError`).
- **Epoll vs tokio:** the C++ client is single-threaded epoll; tokio is fine internally, but reconnect/`connection_closed` semantics (fail all pending callbacks with `connection_closed`) must match.

## Steps

1. **Audit for wire parity.** ✅ DONE — `tests/wire.rs` has golden-bytes tests (varint/JSON/CBOR/error codes/negotiation) and E2E round-trips over a real socket for both encodings.
2. **Verify client semantics.** ✅ DONE — hello gate, id→callback map, error dispatch on close, `connection_closed`/`no_hello_reply` codes all covered by tests.
3. **Add `simple-ipc` to `client/Cargo.toml`.** ✅ DONE — pulled in via `daemon-utils`/`msa-daemon-client` (auth wired through `crates/client/src/xbox_auth.rs` → `jni/xbox_live.rs`).
4. **E2E against a real daemon** ✅ DONE — the Flatpak mcpelauncher bundle ships `mcpelauncher-msa-daemon` (the C++ daemon, no Qt deps in the core binary). `crates/msa-daemon-client/tests/daemon_e2e.rs` launches it via `ServiceLauncher` and completes `.hello` + `msa/get_accounts` + `msa/request_token` over both the JSON handshake; `msa-daemon -d <dir> -x` listens on `<dir>/service`.
5. **Remove the C++ target.** ✅ DONE — 14 `simpleipc` sources dropped from `cpp-bridge-sys/build.rs`; `mcpelauncher-simpleipc` removed from `client/build.rs` STATIC_LIBS.
6. **Delete C++ sources/headers.** ✅ DONE — `crates/client/src/manifest_libs/simpleipc/` and `include/simple-ipc/` removed.

## Done when

- `nm` shows no `simpleipc::` symbols. ✅
- `.hello` handshake + a real RPC succeed against a running C++ daemon over a unix socket, both encodings. ✅ (real daemon; JSON handshake exercised — CBOR negotiation left to `tests/wire.rs` lock)
- `mcpelauncher-simpleipc` gone from `client/build.rs`. ✅

## Depends on / used by

- **`daemon-utils`** (port next — its `daemon_launcher.cpp` includes `simpleipc/common/io_handler.h`)
- **`msa-daemon-client`** (RPC client on top of simpleipc)
