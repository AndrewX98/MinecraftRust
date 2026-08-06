/// Stub replacing jni_bridge.cpp for the Rust build.
/// Provides extern "C" wrappers for C++-dependent operations that are not
/// yet ported to Rust: JNI support lifecycle and the FakeJni/Baron VM bridge.
///
/// Pure orchestration (mc_jni_create, mc_jni_start_game, etc.) lives in Rust
/// in rust_bridge.rs and calls through these extern "C" wrappers.
///
/// Ported to Rust (portable subset): mc_setup_android_hooks and mc_dlsym now
/// live in capi.rs; rust_load_stub/rust_add_symbols, mc_register_android_hook,
/// the unused Rust-bridge extern block, and dead jni_support_start_game_cpp /
/// jni_support_get_text_input_handler are gone. The process globals
/// (g_jni_support/g_rust_jni_support), looper routing
/// (mc_set_looper_running_cpp, mc_jni_support_on_window_created_cpp) and the
/// game-close hooks (fake_looper_finish, fake_looper_on_game_activity_close)
/// are now Rust (`crate::fake_looper`); the C++ JniSupport factory and the
/// FakeJni/Baron accessors remain until the ga->vm switch (PORT_JNI_SUPPORT.md).

#include "jni/jni_support.h"
#include "xbox_live_helper.h"

extern "C" void* mcpelauncher_dispatch_dlsym(void* handle, const char* name);
extern "C" void* mcpelauncher_dispatch_dlopen(const char* name, int flags);
extern "C" int mcpelauncher_dispatch_dlclose(void* handle);

// ============================================================
// C++ JniSupport factory (needed by looper/window internals)
// ============================================================

// (create/destroy ported to Rust — see jni_support.rs)

extern "C" void jni_support_register_minecraft_natives_cpp(void* s, void* game_handle) {
    auto* support = (JniSupport*)s;
    static void* handle = nullptr;
    handle = game_handle;
    // Register game native methods (nativeRegisterThis, etc.) with the C++ Baron JVM.
    // This MUST be called after libminecraftpe.so is loaded but before startGame().
    // The symResolver uses linker::dlsym on the loaded game library handle.
    support->registerMinecraftNatives(+[](const char* sym) -> void* {
        return mcpelauncher_dispatch_dlsym(handle, sym);
    });
}

// ============================================================
// Linker symbol resolver for Rust (ported to capi.rs — Phase 12)
// ============================================================

// (bridge function ported to Rust — see minecraft_load.rs)

// ============================================================
// C++ wrappers for Rust bridge (FakeJni, PathHelper, XboxLiveHelper)
// ============================================================

extern "C" void* jni_support_get_jvm(void* s) {
    return static_cast<FakeJni::Jvm*>(((JniSupport*)s)->getJavaVM());
}

extern "C" void fake_jni_jvm_attach_library(void* jvm, const char* path) {
    // Use handle-type-agnostic dispatch wrappers: libraries owned by the Rust
    // linker (libfmod.so, libminecraftpe.so) resolve to their Rust handles so
    // JNI_OnLoad comes from the same image the game uses. Routing through the
    // C++ bionic dlopen would re-load a Rust-owned library a second time.
    static_cast<FakeJni::Jvm*>(jvm)->attachLibrary(
        path, "", {mcpelauncher_dispatch_dlopen, mcpelauncher_dispatch_dlsym, mcpelauncher_dispatch_dlclose});
}

extern "C" void* fake_jni_local_frame_create(void* jvm) {
    return new FakeJni::LocalFrame(*static_cast<FakeJni::Jvm*>(jvm));
}

extern "C" void fake_jni_local_frame_destroy(void* frame) {
    delete static_cast<FakeJni::LocalFrame*>(frame);
}

extern "C" void* fake_jni_local_frame_get_env(void* frame) {
    return &static_cast<FakeJni::LocalFrame*>(frame)->getJniEnv();
}

extern "C" void xbox_live_helper_set_jvm(void* jvm) {
    XboxLiveHelper::getInstance().setJvm(static_cast<FakeJni::Jvm*>(jvm));
}

extern "C" void* jni_support_get_game_activity_callbacks_ptr(void* s) {
    return &((JniSupport*)s)->getGameActivityCallbacks();
}

extern "C" void* jni_support_get_java_vm_ptr(void* s) {
    return ((JniSupport*)s)->getJavaVM();
}

extern "C" void* jni_support_get_activity_ref(void* s) {
    return ((JniSupport*)s)->getActivityRef();
}

extern "C" void jni_support_set_game_activity_instance(void* s, void* instance) {
    ((JniSupport*)s)->getGameActivity()->instance = instance;
}

extern "C" void* jni_support_get_game_activity_ptr(void* s) {
    return ((JniSupport*)s)->getGameActivity();
}

extern "C" void* jni_support_new_cpp() {
    return new JniSupport();
}

extern "C" void jni_support_init_activity(void* s) {
    ((JniSupport*)s)->initActivity();
}

/// Set Baron FakeJni MainActivity::storageDirectory (used by getExternalStoragePath /
/// getFilesDir). The Rust path previously only updated jnivm_set_storage_dir, so
/// AppPlatform saw CurrentFileStoragePath = ''.
extern "C" void jni_support_set_activity_storage_dir(void* s, const char* dir) {
    if (!s || !dir) return;
    ((JniSupport*)s)->setActivityStorageDir(dir);
}

extern "C" void jni_support_delete(void* s) {
    delete (JniSupport*)s;
}

