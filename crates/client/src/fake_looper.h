#pragma once

#include <android/looper.h>
#include <memory>
#include <game_window.h>
#include "fake_inputqueue.h"
#include <cstddef>
#include <string>
#include <unordered_map>

class JniSupport;
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
    void* windowCallbacks = nullptr;

public:
    void initializeWindow();
    static void setJniSupport(JniSupport *support) {
        jniSupport = support;
    }

    static void setRustJniSupport(void *s) {
        rustJniSupport = s;
    }

    ~FakeLooper();

    int addFd(int fd, int ident, int events, ALooper_callbackFunc callback, void *data);

    void attachInputQueue(int ident, ALooper_callbackFunc callback, void *data);

    static void initWindow();

    static void setWindow(std::shared_ptr<GameWindow> window);

    // Public accessors for Rust/C wrappers
    static FakeLooper* getCurrent() { return currentLooper.get(); }
    static bool hasCurrent() { return (bool)currentLooper; }
    static bool isCurrentPrepared() { return currentLooper && currentLooper->prepared; }
    static void setCurrentPrepared() { if (currentLooper) currentLooper->prepared = true; }
    static void createCurrent() { currentLooper = std::make_unique<FakeLooper>(); }

    // Accessors for Rust pollAll / addFd / attachInputQueue
    GameWindow* getWindow() const { return associatedWindow.get(); }
    void* getWindowCallbacks() const { return windowCallbacks; }
    FakeInputQueue* getInputQueue() { return &fakeInputQueue; }
    static JniSupport* getJniSupport() { return jniSupport; }
    static void* getRustJniSupport() { return rustJniSupport; }

    void setWindowCallbacks(void* cb) { windowCallbacks = cb; }
    std::shared_ptr<GameWindow> getWindowShared() { return associatedWindow; }

    static void onGameActivityClose(GameActivity *native);
};
