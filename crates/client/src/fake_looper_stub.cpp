/// Stub replacing fake_looper.cpp for the Rust build.
/// Contains the full implementation identical to the manifest version.

#include "fake_looper.h"
#include "jni/jni_support.h"
#include "main.h"
#include "shader_error_patch.h"
#include "splitscreen_patch.h"
#include "gl_core_patch.h"
#include "fake_egl.h"

#include <sys/poll.h>
#include <thread>
#include <cstdio>

#include <game_window_manager.h>
#include <log.h>

// Forward declare Rust's window setter
extern "C" void jni_support_on_window_created(void *s, void *window, void *input_queue);

// Phase 2: CorePatches lives in Rust (core_patches.rs)
extern "C" void core_patches_set_game_window(void* window);
extern "C" void core_patches_set_game_window_callbacks(void* callbacks);

// Phase 3: WindowCallbacks lives in Rust (window_callbacks.rs)
extern "C" void window_callbacks_load_gamepad_mappings();
extern "C" void window_callbacks_destroy(void* callbacks);

JniSupport *FakeLooper::jniSupport;
void *FakeLooper::rustJniSupport = nullptr;
thread_local std::unique_ptr<FakeLooper> FakeLooper::currentLooper;
std::shared_ptr<GameWindow> FakeLooper::pendingWindow;

void FakeLooper::initWindow() {
    if(!currentLooper) {
        currentLooper = std::make_unique<FakeLooper>();
    }
    currentLooper->initializeWindow();
}

void FakeLooper::setWindow(std::shared_ptr<GameWindow> window) {
    pendingWindow = std::move(window);
}

void FakeLooper::onGameActivityClose(GameActivity *native) {
    FakeJni::JniEnvContext ctx(*(FakeJni::Jvm *)native->vm);
    auto activity = std::dynamic_pointer_cast<MainActivity>(ctx.getJniEnv().resolveReference(native->javaGameActivity));
    activity->quitCallback();
}

// C-linkage thunk so the Rust load path (minecraft_load.rs) can pass a C-safe
// function pointer for the GameActivity_finish hook.
extern "C" void fake_looper_on_game_activity_close(void* native) {
    FakeLooper::onGameActivityClose((GameActivity*)native);
}

void FakeLooper::initializeWindow() {
    if(associatedWindow) {
        return;
    }
    if(pendingWindow) {
        associatedWindow = std::move(pendingWindow);
        return;
    }
    Log::info("Launcher", "Loading gamepad mappings");
    window_callbacks_load_gamepad_mappings();
    Log::info("Launcher", "Creating window");
    associatedWindow = GameWindowManager::getManager()->createWindow("Minecraft",
                                                                     options.windowWidth, options.windowHeight, options.graphicsApi);
    FakeEGL::setupGLOverrides();
}

FakeLooper::~FakeLooper() {
    core_patches_set_game_window(nullptr);
    if(windowCallbacks) {
        core_patches_set_game_window_callbacks(nullptr);
        window_callbacks_destroy(windowCallbacks);
    }
    associatedWindow.reset();
}

int FakeLooper::addFd(int fd, int ident, int events, ALooper_callbackFunc callback, void *data) {
    if(androidEvent)
        return -1;
    if(callback != nullptr)
        throw std::runtime_error("callback is not supported");
    androidEvent = EventEntry(fd, ident, events, data);
    return 1;
}

void FakeLooper::attachInputQueue(int ident, ALooper_callbackFunc callback, void *data) {
    if(inputEntry)
        throw std::runtime_error("attachInputQueue already called on this looper");
    if(callback != nullptr)
        throw std::runtime_error("callback is not supported");
    inputEntry = EventEntry(-1, ident, 0, data);
}
