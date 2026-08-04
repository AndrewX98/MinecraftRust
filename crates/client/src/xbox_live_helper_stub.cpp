/// Minimal XboxLiveHelper for the Rust build.
/// Auth flows now run through Rust (crates/client/src/xbox_auth.rs) which
/// talks to the MSA daemon over the ported simple-ipc crate; the C++ launcher
/// no longer manages the daemon. Only getInstance()/setJvm() are required for
/// the C++ bridge (jni_support.cpp, jni_bridge_stub.cpp).

#include "manifest_headers/xbox_live_helper.h"

XboxLiveHelper XboxLiveHelper::instance;
