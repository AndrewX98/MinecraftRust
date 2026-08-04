# Port: msa-daemon-client

**Status:** Rust crate (`crates/msa-daemon-client/`) is wired into `client` and drives auth. The C++ `msa-daemon-client` (2 files, target `mcpelauncher-msa-daemon-client`) has been **removed** from `cpp-bridge-sys` and `client/build.rs`; the `manifest_libs/msa-daemon-client` + `include/msa-daemon-client` sources are deleted. Auth flow: `jni/xbox_live.rs` (`invokeMSA` silent token request, `invokeAuthFlow` → `pick_account`) → `crates/client/src/xbox_auth.rs` → Rust `ServiceClient` over ported `simple-ipc`. `MSA_CLIENT_ID`/`MSA_COBRAND_ID` constants preserved (`android-app://com.mojang.minecraftpe.H62DKCBHJP6WXXIV7RBFOGOL4NAK4E6Y`, `90023`). **Round-trips against the real C++ daemon verified** (`crates/msa-daemon-client/tests/daemon_e2e.rs`): `.hello`, `get_accounts` → `[]`, `request_token` unknown cid → `-100 No such account`. Full token acquisition still needs a real signed-in account.

## Role

RPC client for Microsoft auth against the `mcpelauncher-msa-daemon` process: requests Xbox Live tokens (XBL/CLX) by `cid`, plus deserializes `msa::client::Token`/`LegacyToken`/`CompactToken` JSON payloads.

## C++ to remove (target `mcpelauncher-msa-daemon-client`, 2 files)

| File | Lines | Role |
|------|-------|------|
| `msa-daemon-client/src/service_client.cpp` | 59 | `ServiceClient` RPC methods to the daemon (token requests), hello gating |
| `msa-daemon-client/src/token.cpp` | 24 | `Token::fromJson` dispatch by `"type"` → `LegacyToken`/`CompactToken` |

## Existing Rust

`crates/msa-daemon-client/` — `types.rs` (`BaseAccountInfo`, `SecurityScope`, `Token`, `LegacyToken`, `CompactToken`, `TokenType`), `client.rs` (`ServiceClient`), `error.rs` (`ErrorCodes`), `launcher.rs` (`ServiceLauncher`). Depends on `simple-ipc` + `daemon-utils` + `util`.

## Compatibility notes

- **RPC method names + payload shapes must match the daemon's C++ server handlers** exactly (these are cross-process strings/JSON, not Rust-visible). The daemon is not in this repo; capture its `.hello` + method names from the mcpelauncher-manifest source or a live daemon before changing anything.
- **Token JSON:** `token.cpp` parses `{"type": "urn:passport:legacy"|"urn:passport:compact", "scope": {address, policy_ref}, "created", "expires"}`. `types.rs` must deserialize the identical fields; unknown `"type"` must error like the C++ `throw`.
- **Error codes:** `rpc_error_code` (`error_code.cpp`) values used for auth failures (`connection_closed`, `no_hello_reply`, …) must map through `error.rs::ErrorCodes`.

## Steps

1. **Audit `client.rs`/`types.rs` against the C++ sources.** ✅ DONE — method names (`msa/get_accounts`, `msa/add_account`, `msa/remove_account`, `msa/pick_account`, `msa/request_token`) and request JSON match `service_client.cpp`; token fields (`type`, `scope.{address,policy_ref}`, `created`, `expires`, `xml_data`, `binary_secret`, `binary_token`) match `token.h`/`legacy_token.h`/`compact_token.h`.
2. **Cross-check against the daemon** server handlers. ✅ DONE — verified against the real daemon (`daemon_e2e.rs`): method names, request JSON, and RPC error codes (`-100 No such account`) match `service_client.cpp`/daemon behavior.
3. **Add `msa-daemon-client` to `client/Cargo.toml`.** ✅ DONE.
4. **Wire the auth flow** replacing the stub behavior. ✅ DONE — `crates/client/src/xbox_auth.rs` holds the Rust `ServiceClient`; `jni/xbox_live.rs` calls `request_xbl_token` (silent, error `-102` → `TICKET_UI_INTERACTION_REQUIRED`) and `pick_account` (→ `auth_flow_callback`). The C++ `XboxLiveHelper` was slimmed to `getInstance()`/`setJvm()` (no more msa/simpleipc/cll deps).
5. **Remove the C++ target.** ✅ DONE — 2 files dropped from `cpp-bridge-sys/build.rs`; `mcpelauncher-msa-daemon-client` removed from `client/build.rs` STATIC_LIBS.
6. **Delete C++ sources/headers.** ✅ DONE — `manifest_libs/msa-daemon-client/` and `include/msa-daemon-client/` removed.

## Done when

- `nm` shows no `msa::client::` symbols. ✅
- Auth RPCs round-trip against a live daemon. ✅ (`get_accounts`, `request_token` error path; full token needs a real signed-in account)
- `mcpelauncher-msa-daemon-client` gone from `client/build.rs`. ✅

## Depends on / used by

- Depends on **`simple-ipc`** and **`daemon-utils`** (port both first).
- Unblocks functional online auth / telemetry once wired.
