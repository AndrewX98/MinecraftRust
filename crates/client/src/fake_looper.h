#pragma once

#include <android/looper.h>
#include <memory>
#include <game_window.h>
#include "fake_inputqueue.h"
#include <cstddef>
#include <string>
#include <unordered_map>

class JniSupport;
class WindowCallbacks;
struct GameActivity;

class FakeLooper {
private:
    static JniSupport *jniSupport;
    static void *rustJniSupport;
    static std::shared_ptr<GameWindow> pendingWindow;
    static thread_local std::unique_ptr<FakeLooper> currentLooper;
    bool prepared = false;
    bool textInput = false;
    int menuSize = 0;

    struct EventEntry {
        int fd, ident, events;
        void *data;

        EventEntry() : ident(-1) {}
        EventEntry(int fd, int ident, int events, void *data) : fd(fd), ident(ident), events(events), data(data) {}

        void fill(int *outFd, void **outData) const {
            if(outFd)
                *outFd = fd;
            if(outData)
                *outData = data;
        }

        operator bool const() {
            return ident != -1;
        }
    };
    EventEntry androidEvent;
    EventEntry inputEntry;
    FakeInputQueue fakeInputQueue;

    std::shared_ptr<GameWindow> associatedWindow;
    std::shared_ptr<WindowCallbacks> associatedWindowCallbacks;

public:
    void initializeWindow();
    static void setJniSupport(JniSupport *support) {
        jniSupport = support;
    }

    static void setRustJniSupport(void *s) {
        rustJniSupport = s;
    }

    ~FakeLooper();

    void prepare();

    int addFd(int fd, int ident, int events, ALooper_callbackFunc callback, void *data);

    void attachInputQueue(int ident, ALooper_callbackFunc callback, void *data);

    int pollAll(int timeoutMillis, int *outFd, int *outEvents, void **outData);

    static void initWindow();

    static void setWindow(std::shared_ptr<GameWindow> window);

    // Public accessors for Rust/C wrappers
    static FakeLooper* getCurrent() { return currentLooper.get(); }
    static bool hasCurrent() { return (bool)currentLooper; }
    static bool isCurrentPrepared() { return currentLooper && currentLooper->prepared; }
    static void setCurrentPrepared() { if (currentLooper) currentLooper->prepared = true; }
    static void createCurrent() { currentLooper = std::make_unique<FakeLooper>(); }
    static int pollCurrent(int timeoutMillis, int *outFd, int *outEvents, void **outData) {
        if (!currentLooper) return -1;
        return currentLooper->pollAll(timeoutMillis, outFd, outEvents, outData);
    }

    // Accessors for Rust pollAll / addFd / attachInputQueue
    GameWindow* getWindow() const { return associatedWindow.get(); }
    WindowCallbacks* getWindowCallbacks() const { return associatedWindowCallbacks.get(); }
    FakeInputQueue* getInputQueue() { return &fakeInputQueue; }
    static JniSupport* getJniSupport() { return jniSupport; }
    static void* getRustJniSupport() { return rustJniSupport; }

    // Accessors for Rust prepare (shared_ptr for CorePatches)
    void setWindowCallbacks(std::shared_ptr<WindowCallbacks> cb) { associatedWindowCallbacks = std::move(cb); }
    std::shared_ptr<GameWindow> getWindowShared() { return associatedWindow; }
    std::shared_ptr<WindowCallbacks> getWindowCallbacksShared() { return associatedWindowCallbacks; }

    static void onGameActivityClose(GameActivity *native);
};
