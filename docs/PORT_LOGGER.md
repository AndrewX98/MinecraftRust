# Port: logger

**Status:** DONE. C++ `log.cpp` and `logger_stub.cpp` deleted; the log sink lives in `crates/util/src/logger.rs`. All C++ `Log::*` calls route through Rust.

## What was done

- **Rust export** `crates/client/src/logger.rs`: exports the C++ mangled `Log::vlog` symbol (`_ZN3Log4vlogE8LogLevelPKcS2_P13__va_list_tag`) directly — ABI-identical `(c_int, *const c_char, *const c_char, *mut c_void)` (va_list = `__va_list_tag*`). Body mirrors the old C++ shim exactly: `vsnprintf` into a 4096 buffer, clamp, strip trailing `\r\n`, then forwards to the Rust FFI. This replaced the `logger_stub.cpp` shim.
- **Rust FFI** `mcpelauncher_log_vlog` (`rust_bridge.rs`): maps `int level` → `util::logger::LogLevel` (0=Trace..4=Error) and calls `util::logger::Log::vlog`.
- **`util/src/logger.rs`**: switched output to `stdout` + explicit flush to match C++ `printf` + `fflush(stdout)` exactly (it had no other consumers). Format verified identical: `%H:%M:%S %-5s [tag] text`.
- The `mcpelauncher-manifest-libs` cc::Build target (which held `log.cpp`) is gone; `logger_stub.cpp` is out of the client-jni `stub_files`.
- `mcpelauncher_vlog`/`mcpelauncher_log` API symbols in `minecraft_utils.cpp` still point at the Rust export, so game-facing API behavior is preserved.

## Done when

- Same log output (`HH:MM:SS Info  [tag] text` on stdout); no `Log::vlog` implementation in the C++ static libs — only the Rust export, which forwards to Rust. Verified: `nm` shows `_ZN3Log4vlog…` (Rust `t`), no `logger_stub.o`, no `libmcpelauncher-manifest-libs.a` / `log.o`.
