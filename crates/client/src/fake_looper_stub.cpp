/// Stub replacing fake_looper.cpp for the Rust build.
/// Contains the full implementation identical to the manifest version.

#include "fake_looper.h"
#include "jni/jni_support.h"
#include "main.h"
#include "shader_error_patch.h"
#include "splitscreen_patch.h"
#include "gl_core_patch.h"
#include "core_patches.h"
#include "fake_egl.h"

#include <sys/poll.h>
#include <thread>
#include <cstdio>

#include <game_window_manager.h>
#include <log.h>

// Forward declare Rust's window setter
extern "C" void jni_support_on_window_created(void *s, void *window, void *input_queue);

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

void FakeLooper::initializeWindow() {
    if(associatedWindow) {
        return;
    }
    if(pendingWindow) {
        associatedWindow = std::move(pendingWindow);
        return;
    }
    Log::info("Launcher", "Loading gamepad mappings");
    WindowCallbacks::loadGamepadMappings();
    Log::info("Launcher", "Creating window");
    associatedWindow = GameWindowManager::getManager()->createWindow("Minecraft",
                                                                     options.windowWidth, options.windowHeight, options.graphicsApi);
    FakeEGL::setupGLOverrides();
}

void FakeLooper::prepare() {
    fprintf(stderr, "=== FakeLooper::prepare: tid=%lu ===\n",
            (unsigned long)std::hash<std::thread::id>{}(std::this_thread::get_id()));
    jniSupport->setLooperRunning(true);
    fprintf(stderr, "=== FakeLooper::prepare: initializeWindow ===\n");
    initializeWindow();
    fprintf(stderr, "=== FakeLooper::prepare: onWindowCreated window=%p ===\n",
            (void*)associatedWindow.get());
    jniSupport->onWindowCreated((ANativeWindow *)(void *)associatedWindow.get(),
                                (AInputQueue *)(void *)&fakeInputQueue);
    // Also forward the window to the Rust JniSupport so its lifecycle
    // callbacks receive a valid window pointer.
    if (rustJniSupport) {
        jni_support_on_window_created(rustJniSupport,
                                      (void *)associatedWindow.get(),
                                      (void *)&fakeInputQueue);
    }
    fprintf(stderr, "=== FakeLooper::prepare: creating WindowCallbacks ===\n");
    associatedWindowCallbacks = std::make_shared<WindowCallbacks>(*associatedWindow, (void*)jniSupport, rustJniSupport, fakeInputQueue);
    associatedWindowCallbacks->registerCallbacks();

    CorePatches::setGameWindow(associatedWindow);
    CorePatches::setGameWindowCallbacks(associatedWindowCallbacks);

    associatedWindow->show();
    SplitscreenPatch::onGLContextCreated();
    ShaderErrorPatch::onGLContextCreated();
    fprintf(stderr, "=== FakeLooper::prepare: makeCurrent(false) ===\n");
    associatedWindow->makeCurrent(false);
    fprintf(stderr, "=== FakeLooper::prepare: done ===\n");
}

FakeLooper::~FakeLooper() {
    CorePatches::setGameWindow(nullptr);
    associatedWindow.reset();
    associatedWindowCallbacks.reset();
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

int FakeLooper::pollAll(int timeoutMillis, int *outFd, int *outEvents, void **outData) {
    static int pollCount = 0;
    pollCount++;
    if (pollCount <= 5 || pollCount % 1000 == 0) {
        fprintf(stderr, "=== FakeLooper::pollAll call #%d tid=%lu ===\n", pollCount,
                (unsigned long)std::hash<std::thread::id>{}(std::this_thread::get_id()));
    }
    associatedWindowCallbacks->startSendEvents();
    if(textInput != jniSupport->getTextInputHandler().isEnabled()) {
        textInput = jniSupport->getTextInputHandler().isEnabled();
        if(textInput) {
            associatedWindow->startTextInput();
        } else {
            associatedWindow->stopTextInput();
        }
    }

    if(androidEvent) {
        pollfd f;
        f.fd = androidEvent.fd;
        f.events = androidEvent.events;
        if(poll(&f, 1, 0) > 0) {
            androidEvent.fill(outFd, outData);
            if(outEvents)
                *outEvents = f.revents;
            return androidEvent.ident;
        }
    }

    if(inputEntry && fakeInputQueue.hasEvents()) {
        inputEntry.fill(outFd, outData);
        return inputEntry.ident;
    }

    associatedWindow->pollEvents();
    associatedWindowCallbacks->markRequeueGamepadInput();
    return ALOOPER_POLL_TIMEOUT;
}
