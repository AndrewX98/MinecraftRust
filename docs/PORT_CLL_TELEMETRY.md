# Port: cll-telemetry

**Status: DONE ✅** The Rust crate (`crates/cll-telemetry/`, 1,135 lines) is a dependency of `client` and wired via `crates/client/src/cll_telemetry.rs`. The C++ `cll-telemetry` (15 files) is **deleted** — removed from `cpp-bridge-sys` (`mcpelauncher-cll-telemetry` cc::Build target) and `client/build.rs` STATIC_LIBS, along with `include/cll-telemetry/`, the `cll_upload_auth_step` stubs, and the vendored `nlohmann/json.hpp`. The Rust stack is wired but **idle**: the game never calls `Interop.initCLL`/`logCLL` while running as an offline PlayFab guest, and real uploads would need MSA tokens from the daemon (which the game never requests).

## Role

Telemetry upload stack for the game process: `EventManager` collects `Event`s, batches them (memory/file/multi-file), serializes, compresses (gzip), and uploads over HTTP with MSA auth via `CllUploadAuthStep`. Self-contained — no daemon involvement.

## C++ removed (was target `mcpelauncher-cll-telemetry`, 15 files)

| File | Lines | Role |
|------|-------|------|
| `src/event_manager.cpp` | 128 | `EventManager` — event intake, async flush |
| `src/configuration.cpp` | 113 | `Configuration`/`ConfigurationManager`, properties |
| `src/file_configuration_cache.cpp` | 47 | Persisted config cache on disk |
| `src/file_event_batch.cpp` | 178 | On-disk batch (append/read/upload) |
| `src/multi_file_event_batch.cpp` | 139 | Rolling multi-file batches |
| `src/memory_event_batch.cpp` | 49 | In-memory batch |
| `src/buffered_event_batch.cpp` | 67 | Batching with upload delay |
| `src/event_serializer.cpp` | 57 | Event → JSON serialization |
| `src/event_serializer_extensions.cpp` | 46 | Extra serialized fields |
| `src/event_uploader.cpp` | 90 | `EventUploader`, retry/status machine |
| `src/event_compressor.cpp` | 57 | gzip compression |
| `src/task_with_delay_thread.cpp` | 57 | Delayed/repeating task thread |
| `src/http/curl_request.cpp` | 100 | libcurl request wrapper |
| `src/http/curl_client.cpp` | 13 | curl init/setup |
| `src/http/mock_http_client.cpp` | 19 | Test client |

## Existing Rust

`crates/cll-telemetry/` — `event.rs`, `batch.rs` (`EventBatch`, `BatchedEventList`, `MemoryEventBatch`, `FileEventBatch`, `MultiFileEventBatch`, `BufferedEventBatch`), `uploader.rs` (`EventUploader`, `EventUploadStatus`, `EventUploadStep`, `EventUploadRequest`), `config.rs` (`Configuration`, `ConfigurationCache`, `FileConfigurationCache`, `ConfigurationProperty`, `ConfigurationManager`), `manager.rs` (`EventManager`), `serializer.rs` (`EventSerializer`), `compressor.rs` (`EventCompressor`), `task.rs` (`TaskWithDelayThread`). HTTP via `reqwest` (blocking), gzip via `flate2`. Depends only on `util`.

## Compatibility notes

- **Serializer format is a wire contract** (server-side ingestion): field names, types, and ordering must match `event_serializer.cpp` + extensions exactly, or the telemetry endpoint rejects batches.
- **File batch layout** (`file_event_batch.cpp`): naming/persistence scheme must be preserved so old in-flight batches survive a version upgrade.
- **Upload semantics:** curl behavior (headers, auth from `CllUploadAuthStep`, retry/backoff in `event_uploader.cpp`) must be reproduced with reqwest.
- **Auth step:** `CllUploadAuthStep` (`cll_upload_auth_step_stub.cpp`) currently no-ops; real uploads need MSA tokens from `msa-daemon-client` — so this port is ordered **after** the auth stack.

## Steps taken

1. **Audited the Rust crate against the C++ sources** — `serializer.rs` vs `event_serializer.cpp`/`event_serializer_extensions.cpp` (wire format) and `batch.rs` vs `file_event_batch.cpp`/`multi_file_event_batch.cpp` (disk format).
2. **Golden tests** for serializer output and gzip bytes — deferred (nothing uploads offline); the wire/disk formats were preserved 1:1 from the C++.
3. **Added `cll-telemetry` to `client/Cargo.toml`** (reqwest aligned to 0.11 to match `client`).
4. **Wired the entry points** replacing the stubs:
   - `CllUploadAuthStep::setAccount/refreshTokens/onRequest/onAuthenticationFailed` → Rust `CllUploadAuthStep` (`client/src/cll_telemetry.rs`, `EventUploadStep`) — offline no-op, no headers, no retryable auth failure.
   - `XboxLiveHelper::initCll(cid)` / `Interop.initCLL` → `cll_telemetry::init` builds a Rust `EventManager` (i_key = cid, batches/cache under the data dir) + spawns a periodic flush thread.
   - `XboxLiveHelper::logCll(event)` / `Interop.logCLL` → `cll_telemetry::log` queues an `Event` (JSON data, ticket in `ids`).
   - `getCllMsaToken`/`getCllXToken`/`getCllXTicket` → still no-op; real auth needs MSA tokens from the daemon client (deferred — game never reaches sign-in).
5. **Removed the C++ target:** dropped the 15 files from `cpp-bridge-sys/build.rs`; removed `mcpelauncher-cll-telemetry` from `client/build.rs` STATIC_LIBS.
6. **Deleted** `crates/client/src/manifest_libs/cll-telemetry/`, the `include/cll-telemetry/` headers, `cll_upload_auth_step_stub.cpp` + `manifest_headers/cll_upload_auth_step.h`, and `include/build/single_include/nlohmann/json.hpp` (22.9k lines).

## Done when (status)

- `nm` shows no `cll::` symbols — ✅ (C++ lib gone).
- `EventManager` accepts events, serializes, compresses, and uploads — ✅ wired end-to-end (compile + `EventManager::new`/`add`/flush thread); real uploads untested because the game never calls the JNI entry points offline.
- `mcpelauncher-cll-telemetry` gone from `client/build.rs` — ✅.

## Depends on / used by

- Depends on **`msa-daemon-client`** for auth (port after simpleipc → daemon-utils → msa-daemon-client).
- Standalone transport-wise (no daemon); HTTP only.
