# Port: base64

**Status:** DONE. C++ `base64.cpp` deleted; all JNI base64 dispatch goes through the Rust `util` crate.

## What was done

- `jbase64_decode_rust` (`rust_bridge.rs`) now delegates to `util::base64::decode` (skips `\r\n`, errors map to empty result) — this also fixed a padding parity bug where the old inline decoder mapped `=` to 0 and emitted a trailing zero byte.
- `base64_encode_rust` already used `util::base64::encode`.
- Consumers `jbase64_stub.cpp` / `jni_descriptors.cpp` already called the Rust extern functions — no C++ `Base64::` usage remained.
- Deleted `manifest_libs/base64/base64.cpp` and the `mcpelauncher-base64` target from `cpp-bridge-sys/build.rs`; removed the link directive from `client/build.rs`.

## Done when

- JNI base64 round-trips match C++ behavior; no `base64::` symbols in `nm`.
