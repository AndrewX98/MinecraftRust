/// Stub replacing jni_bridge.cpp for the Rust build.
/// Provides extern "C" wrappers for C++-dependent operations that are not
/// yet ported to Rust: android hooks, window creation/GL setup, JNI support
/// lifecycle, and MinecraftUtils::loadMinecraftLib.
///
/// Pure orchestration (mc_jni_create, mc_jni_start_game, etc.) lives in Rust
/// in rust_bridge.rs and calls through these extern "C" wrappers.

#include "jni/jni_support.h"
#include "fake_assetmanager.h"
#include "fake_looper.h"
#include "fake_inputqueue.h"
#include "fake_egl.h"
#include "fake_audio.h"
#include "xbox_live_helper.h"
#include <game_window.h>
#include <game_window_manager.h>
#include <window_callbacks.h>
#include <log.h>
#include <minecraft/imported/android_symbols.h>
#include <mcpelauncher/path_helper.h>
#include "core_patches.h"
#include "splitscreen_patch.h"
#include "shader_error_patch.h"
#include <cstdio>
#include <dlfcn.h>

extern "C" int eglutScreenWidth();
extern "C" int eglutScreenHeight();
#include <vector>
#include <string>
#include <unordered_map>

// Forward declare linker types/functions
struct mcpelauncher_hook_t {
    const char* name;
    void* value;
};
namespace linker {
    void* dlopen(const char* name, int flags);
    void* dlsym(void* handle, const char* symbol);
    int dlclose_unlocked(void* handle);
    void* load_library(const char* name, const std::unordered_map<std::string, void*>& symbols);
}

// Rust linker FFI bridge — mirror C++ state to Rust linker
extern "C" size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);
extern "C" void linker_add_symbols_to_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

static void mirror_rust_load(const char* name, const std::unordered_map<std::string, void*>& syms) {
    size_t n = syms.size();
    if (n == 0) {
        linker_load_library_rust(name, nullptr, nullptr, 0);
        return;
    }
    std::vector<const char*> keys(n);
    std::vector<void*> vals(n);
    size_t i = 0;
    for (auto& [k, v] : syms) {
        keys[i] = k.c_str();
        vals[i] = v;
        i++;
    }
    linker_load_library_rust(name, keys.data(), vals.data(), n);
}

static void mirror_rust_add_symbols(const char* name, const std::unordered_map<std::string, void*>& syms) {
    size_t n = syms.size();
    if (n == 0) return;
    std::vector<const char*> keys(n);
    std::vector<void*> vals(n);
    size_t i = 0;
    for (auto& [k, v] : syms) {
        keys[i] = k.c_str();
        vals[i] = v;
        i++;
    }
    linker_add_symbols_to_library_rust(name, keys.data(), vals.data(), n);
}

struct MinecraftUtils {
    static void* loadMinecraftLib(void* showMousePointerCallback,
                                  void* hideMousePointerCallback,
                                  void* fullscreenCallback,
                                  void* closeCallback,
                                  std::vector<mcpelauncher_hook_t> hooks);
};

// Rust bridge functions
extern "C" {
    int fake_anativewindow_getwidth(void*);
    int fake_anativewindow_getheight(void*);
    void fake_swappygl_fill_hooks(mcpelauncher_hook_t* hooks, size_t count);
    void fake_thread_mover_store_start_thread_id();
    void fake_thread_mover_execute_main_thread();
    void core_patches_show_mouse_pointer();
    void core_patches_hide_mouse_pointer();
    void core_patches_set_fullscreen(void*, int);
    void core_patches_install(void* handle);
    void mc_relocate_glesv2_symbols(void* (*resolver)(const char*));
}

extern "C" unsigned long eglutGetWindowHandle();

// ============================================================
// Android hooks setup (uses C++ unordered_map + hybris hooks)
// ============================================================

// Wrapper to add a single entry to the android sym map from Rust
extern "C" void mc_register_android_hook(void* map, const char* name, void* fn) {
    ((std::unordered_map<std::string, void*>*)map)->insert({name, fn});
}

// Rust registers FakeLooper hooks via mc_register_fake_looper_hooks
extern "C" void mc_register_fake_looper_hooks(void* map);

extern "C" void mc_setup_android_hooks() {
    std::unordered_map<std::string, void*> android_syms;

    FakeAssetManager::initHybrisHooks(android_syms);
    mc_register_fake_looper_hooks(&android_syms);
    android_syms["ANativeWindow_getWidth"] = (void*)fake_anativewindow_getwidth;
    android_syms["ANativeWindow_getHeight"] = (void*)fake_anativewindow_getheight;
    FakeInputQueue::initHybrisHooks(android_syms);

    // APerformanceHint stubs (BIND_NOW requires non-null GOT entries)
    android_syms["APerformanceHint_getManager"] = (void*)+[]() -> void* { return nullptr; };
    android_syms["APerformanceHint_createSession"] = (void*)+[](void*, int, long) -> void* { return nullptr; };
    android_syms["APerformanceHint_closeSession"] = (void*)+[](void*) {};
    android_syms["APerformanceHint_reportActualWorkDuration"] = (void*)+[](void*, long) {};

    for (const char** p = android_symbols; *p != nullptr; p++) {
        android_syms.insert({*p, (void*)+[](void) -> int { return 0; }});
    }

    linker::load_library("libandroid.so", android_syms);
    mirror_rust_load("libandroid.so", android_syms);

    // FMOD setOutput is stubbed to keep AAudio; FMOD then dlopen's libaaudio.so
    // and calls AAudio_* symbols. Without this shim, do_dlopen fails or the
    // Streaming Pool thread SIGSEGVs on null AAudio function pointers.
    {
        std::unordered_map<std::string, void*> audio_syms;
        FakeAudio::initHybrisHooks(audio_syms);
        linker::load_library("libaaudio.so", audio_syms);
        mirror_rust_load("libaaudio.so", audio_syms);
    }
    {
        std::unordered_map<std::string, void*> audio_syms;
        FakeAudio::initHybrisHooks(audio_syms);
        linker::load_library("libaaudio.so.2", audio_syms);
        mirror_rust_load("libaaudio.so.2", audio_syms);
    }

    CorePatches::loadGameWindowLibrary();
}

// C++ FFI helpers for Rust prepare / pollAll / addFd / attachInputQueue
extern "C" void* fake_looper_prepare_begin() {
    if(FakeLooper::hasCurrent() && FakeLooper::isCurrentPrepared())
        throw std::runtime_error("Looper already prepared");
    if(!FakeLooper::hasCurrent())
        FakeLooper::createCurrent();
    FakeLooper::setCurrentPrepared();
    FakeLooper::getCurrent()->initializeWindow();
    FakeLooper::getJniSupport()->setLooperRunning(true);
    return (void*)FakeLooper::getCurrent();
}

// Forward declare Rust's window setter (used by fake_looper_notify_window_created)
extern "C" void jni_support_on_window_created(void *s, void *window, void *input_queue);

extern "C" void fake_looper_notify_window_created() {
    auto* l = FakeLooper::getCurrent();
    auto* win = l->getWindow();
    auto* queue = l->getInputQueue();
    FakeLooper::getJniSupport()->onWindowCreated(
        (ANativeWindow*)(void*)win, (AInputQueue*)(void*)queue);
    auto* rust = FakeLooper::getRustJniSupport();
    if (rust) {
        jni_support_on_window_created(rust, (void*)win, (void*)queue);
    }
}

extern "C" void fake_looper_create_window_callbacks() {
    auto* l = FakeLooper::getCurrent();
    auto cb = std::make_shared<WindowCallbacks>(
        *l->getWindow(), FakeLooper::getJniSupport(), FakeLooper::getRustJniSupport(), *l->getInputQueue());
    cb->registerCallbacks();
    l->setWindowCallbacks(std::move(cb));
}

extern "C" void fake_looper_register_core_patches() {
    auto* l = FakeLooper::getCurrent();
    CorePatches::setGameWindow(l->getWindowShared());
    CorePatches::setGameWindowCallbacks(l->getWindowCallbacksShared());
}

extern "C" void fake_looper_show_window() {
    auto* w = FakeLooper::getCurrent()->getWindow();
    if (w) w->show();
}

extern "C" void fake_looper_splitscreen_patch_gl_created() {
    SplitscreenPatch::onGLContextCreated();
}

extern "C" void fake_looper_shader_error_patch_gl_created() {
    ShaderErrorPatch::onGLContextCreated();
}

extern "C" void fake_looper_window_make_current(int v) {
    auto* w = FakeLooper::getCurrent()->getWindow();
    if (w) w->makeCurrent((bool)v);
}

// C++ FFI helpers for Rust pollAll / addFd / attachInputQueue
extern "C" void* fake_looper_get_window() {
    auto* l = FakeLooper::getCurrent();
    return l ? l->getWindow() : nullptr;
}
extern "C" void* fake_looper_get_callbacks() {
    auto* l = FakeLooper::getCurrent();
    return l ? l->getWindowCallbacks() : nullptr;
}
extern "C" void* fake_looper_get_input_queue() {
    auto* l = FakeLooper::getCurrent();
    return l ? l->getInputQueue() : nullptr;
}
extern "C" bool fake_looper_get_text_input_enabled() {
    return FakeLooper::getJniSupport()->getTextInputHandler().isEnabled();
}
extern "C" void fake_looper_callbacks_start_send_events(void* cb) {
    ((WindowCallbacks*)cb)->startSendEvents();
}
extern "C" void fake_looper_callbacks_mark_requeue_gamepad(void* cb) {
    ((WindowCallbacks*)cb)->markRequeueGamepadInput();
}
extern "C" void fake_looper_window_poll_events(void* w) {
    ((GameWindow*)w)->pollEvents();
}
extern "C" void fake_looper_window_start_text_input(void* w) {
    ((GameWindow*)w)->startTextInput();
}
extern "C" void fake_looper_window_stop_text_input(void* w) {
    ((GameWindow*)w)->stopTextInput();
}
// Upstream FakeEGL path: surface handle IS the GameWindow* (eglCreateWindowSurface
// returns the native_window pointer). makeCurrent/swapBuffers go through GameWindow.
extern "C" void game_window_make_current(void* w, int active) {
    if (!w) return;
    ((GameWindow*)w)->makeCurrent(active != 0);
}
extern "C" void game_window_swap_buffers(void* w) {
    if (!w) return;
    ((GameWindow*)w)->swapBuffers();
}
extern "C" void game_window_get_size(void* w, int* out_w, int* out_h) {
    if (!w) return;
    int ww = 0, hh = 0;
    ((GameWindow*)w)->getWindowSize(ww, hh);
    if (out_w) *out_w = ww;
    if (out_h) *out_h = hh;
}
extern "C" bool fake_input_queue_has_events(void* q) {
    return ((FakeInputQueue*)q)->hasEvents();
}

extern "C" void fake_looper_finish(void* native) {
    ANativeActivity* an = (ANativeActivity*)native;
    FakeJni::JniEnvContext ctx(*(FakeJni::Jvm *)an->vm);
    auto activity = std::dynamic_pointer_cast<MainActivity>(ctx.getJniEnv().resolveReference(an->clazz));
    activity->quitCallback();
}

// ============================================================
// Window creation + GL setup (uses GameWindowManager)
// ============================================================

extern "C" void mc_create_window_and_setup_graphics() {
    typedef int (*XInitThreadsFn)(void);
    XInitThreadsFn xinit = (XInitThreadsFn)dlsym(RTLD_DEFAULT, "XInitThreads");
    if (xinit) {
        xinit();
        Log::info("LAUNCHER", "XInitThreads() called successfully");
    } else {
        Log::warn("LAUNCHER", "XInitThreads not available");
    }

    Log::info("LAUNCHER", "Creating window via GameWindowManager...");
    auto windowManager = GameWindowManager::getManager();
    Log::info("LAUNCHER", "GameWindowManager created, creating window...");
    int win_w = eglutScreenWidth();
    int win_h = eglutScreenHeight();
    Log::info("LAUNCHER", "Using screen size: %dx%d", win_w, win_h);
    auto window = windowManager->createWindow("Minecraft", win_w, win_h, GraphicsApi::OPENGL_ES2);
    Log::info("LAUNCHER", "Window created successfully");
    FakeLooper::setWindow(window);

    auto procAddr = windowManager->getProcAddrFunc();
    FakeEGL::setProcAddrFunction(reinterpret_cast<void* (*)(const char*)>(procAddr));
    FakeEGL::installLibrary();
    FakeEGL::setupGLOverrides();
    FakeEGL::saveCurrentWindowHandle();
    FakeEGL::saveNativeWindow(eglutGetWindowHandle());
    FakeEGL::releaseContext();
    Log::info("LAUNCHER", "FakeEGL installed");

    mc_relocate_glesv2_symbols(fake_egl::eglGetProcAddress);
    Log::info("LAUNCHER", "Graphics setup complete");
}

// ============================================================
// C++ JniSupport factory (needed by FakeLooper internals)
// ============================================================

// (create/destroy ported to Rust — see jni_support.rs)

extern "C" void jni_support_start_game_cpp(void* s, void* game_on_create, void* stbi_load, void* stbi_image_free) {
    auto* support = (JniSupport*)s;
    // Use the C++ startGame which properly sets up the JNI environment
    support->startGame(nullptr, (GameActivity_createFunc*)game_on_create, stbi_load, stbi_image_free);
}

extern "C" void jni_support_register_minecraft_natives_cpp(void* s, void* game_handle) {
    auto* support = (JniSupport*)s;
    static void* handle = nullptr;
    handle = game_handle;
    // Register game native methods (nativeRegisterThis, etc.) with the C++ Baron JVM.
    // This MUST be called after libminecraftpe.so is loaded but before startGame().
    // The symResolver uses linker::dlsym on the loaded game library handle.
    support->registerMinecraftNatives(+[](const char* sym) -> void* {
        return linker::dlsym(handle, sym);
    });
}

extern "C" void fake_looper_set_jni_support(void* support) {
    FakeLooper::setJniSupport((JniSupport*)support);
}

extern "C" void fake_looper_set_rust_jni_support(void* support) {
    FakeLooper::setRustJniSupport(support);
}

// ============================================================
// Linker symbol resolver for Rust
// ============================================================

extern "C" void* mc_dlsym(void* handle, const char* symbol) {
    return linker::dlsym(handle, symbol);
}

// (bridge function ported to Rust — see jni_support.rs)

// ============================================================
// Minecraft library loading (uses MinecraftUtils + linker)
// ============================================================

extern "C" void* mc_load_minecraft() {
    std::vector<mcpelauncher_hook_t> hooks(15);
    fake_swappygl_fill_hooks(hooks.data(), hooks.size());

    void* handle = MinecraftUtils::loadMinecraftLib(
        reinterpret_cast<void*>(&core_patches_show_mouse_pointer),
        reinterpret_cast<void*>(&core_patches_hide_mouse_pointer),
        reinterpret_cast<void*>(&core_patches_set_fullscreen),
        reinterpret_cast<void*>(&FakeLooper::onGameActivityClose),
        hooks);
    if (handle) {
        core_patches_install(handle);
    }
    return handle;
}

// ============================================================
// C-linkage wrapper for eglSwapBuffers (called from Rust)
// ============================================================

extern "C" int mc_egl_swap_buffers(void* display, void* surface) {
    return fake_egl::eglSwapBuffers((EGLDisplay)display, (EGLSurface)surface);
}

// ============================================================
// C++ wrappers for Rust bridge (FakeJni, PathHelper, XboxLiveHelper)
// ============================================================

extern "C" void* jni_support_get_jvm(void* s) {
    return static_cast<FakeJni::Jvm*>(((JniSupport*)s)->getJavaVM());
}

extern "C" void fake_jni_jvm_attach_library(void* jvm, const char* path) {
    static_cast<FakeJni::Jvm*>(jvm)->attachLibrary(
        path, "", {linker::dlopen, linker::dlsym, linker::dlclose_unlocked});
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

extern "C" const char* path_helper_get_primary_data_directory() {
    static std::string dir;
    if (dir.empty()) dir = PathHelper::getPrimaryDataDirectory();
    return dir.c_str();
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

extern "C" void* jni_support_get_window_ptr(void* s) {
    return ((JniSupport*)s)->getWindow();
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

// ============================================================
// JniSupport bridge functions (void* → JniSupport*)
// Called from window_callbacks_stub.cpp
// ============================================================

extern "C" void* jni_support_get_text_input_handler(void*) {
    // Return the Rust global TextInputHandler instead of C++ member
    extern void* jnivm_get_text_input_handler();
    return jnivm_get_text_input_handler();
}
