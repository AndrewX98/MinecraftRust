# Port: logger

**Status:** DONE. C++ `log.cpp` deleted; the log sink lives in `crates/util/src/logger.rs`. All C++ `Log::*` calls route through Rust.

## What was done

- **Shim** `crates/client/src/logger_stub.cpp`: implements `Log::vlog(LogLevel, tag, text, va_list)` — the only non-inline symbol `log.cpp` provided (everything else in `log.h` is `static inline`). It does `vsnprintf` (varargs can't cross FFI), strips trailing `\r\n`, and forwards the formatted text to Rust.
- **Rust FFI** `mcpelauncher_log_vlog` (`rust_bridge.rs`): maps `int level` → `util::logger::LogLevel` (0=Trace..4=Error) and calls `util::logger::Log::vlog`.
- **`util/src/logger.rs`**: switched output to `stdout` + explicit flush to match C++ `printf` + `fflush(stdout)` exactly (it had no other consumers). Format verified identical: `%H:%M:%S %-5s [tag] text`.
- The `mcpelauncher-manifest-libs` cc::Build target (which held `log.cpp`) is gone; `logger_stub.cpp` is in the client-jni `stub_files`.
- `mcpelauncher_vlog`/`mcpelauncher_log` API symbols in `minecraft_utils.cpp` still point at the shim, so game-facing API behavior is preserved.

## Done when

- Same log output (`HH:MM:SS Info  [tag] text` on stdout); no `Log::vlog` implementation in the C++ static libs — only the thin shim, which forwards to Rust. Verified: `nm` shows shim `_ZN3Log4vlog…` + `mcpelauncher_log_vlog` + `util::logger::Log::vlog`, and no `libmcpelauncher-manifest-libs.a` / `log.o`.
