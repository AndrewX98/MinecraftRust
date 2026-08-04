# Port: cll-telemetry

**Status:** Rust crate exists (`crates/cll-telemetry/`, 1,135 lines) but is **not a dependency of `client`**. The C++ `cll-telemetry` (15 files) is still compiled by `cpp-bridge-sys` as `mcpelauncher-cll-telemetry`. All telemetry entry points (`CllUploadAuthStep`, `XboxLiveHelper::initCll`/`logCll`) are no-op stubs, so nothing is uploaded and the C++ lib is dead weight.

## Role

Telemetry upload stack for the game process: `EventManager` collects `Event`s, batches them (memory/file/multi-file), serializes, compresses (gzip), and uploads over HTTP with MSA auth via `CllUploadAuthStep`. Self-contained — no daemon involvement.

## C++ to remove (target `mcpelauncher-cll-telemetry`, 15 files)

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

## Steps

1. **Audit the Rust crate against the C++ sources**, prioritizing `serializer.rs` vs `event_serializer.cpp`/`event_serializer_extensions.cpp` (wire format) and `batch.rs` vs `file_event_batch.cpp`/`multi_file_event_batch.cpp` (disk format).
2. **Add golden tests** for serializer output and gzip bytes against captured C++ output.
3. **Add `cll-telemetry` to `client/Cargo.toml`.**
4. **Wire the entry points** replacing the stubs:
   - `CllUploadAuthStep::setAccount/refreshTokens/onRequest/onAuthenticationFailed` → drive Rust `EventUploader`
   - `XboxLiveHelper::initCll(cid)` → build/start a Rust `EventManager` + `Configuration`
   - `XboxLiveHelper::logCll(event)` → `EventManager` event intake
   - `getCllMsaToken`/`getCllXToken`/`getCllXTicket` → MSA token plumbing from the daemon client
5. **Remove the C++ target:** drop the 15 files from `cpp-bridge-sys/build.rs`; remove `mcpelauncher-cll-telemetry` from `client/build.rs` STATIC_LIBS.
6. Delete `crates/client/src/manifest_libs/cll-telemetry/` and the `include/cll-telemetry/` headers.

## Done when

- `nm` shows no `cll::` symbols.
- `EventManager` accepts events, serializes to the exact C++-compatible format, compresses, and uploads (or at least constructs + flushes a batch end-to-end against a mock server).
- `mcpelauncher-cll-telemetry` gone from `client/build.rs`.

## Depends on / used by

- Depends on **`msa-daemon-client`** for auth (port after simpleipc → daemon-utils → msa-daemon-client).
- Standalone transport-wise (no daemon); HTTP only.
