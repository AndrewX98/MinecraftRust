#pragma once

#include <memory>
#include <fake-jni/jvm.h>

/// Minimal XboxLiveHelper kept for the C++ bridge.
/// MSA auth now runs through the Rust IPC stack (crates/client/src/xbox_auth.rs):
///   - invokeMSA/invokeAuthFlow natives live in Rust (jni/xbox_live.rs)
///   - the C++ ServiceLauncher/ServiceClient/simpleipc chain was removed (Phase: IPC port)
/// Only getInstance()/setJvm() remain, called by jni_support.cpp and jni_bridge_stub.cpp.
class XboxLiveHelper {
private:
    static XboxLiveHelper instance;

    FakeJni::Jvm* vm;

public:
    static XboxLiveHelper& getInstance() {
        return instance;
    }

    void setJvm(FakeJni::Jvm* jvm) {
        vm = jvm;
    }

    void shutdown() {}
};
