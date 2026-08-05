#pragma once

#include <android/input.h>
#include <functional>
#include <string>
#include <unordered_map>

struct FakeInputEvent {
    int32_t source, type;
    int32_t deviceId = 0;

    FakeInputEvent(int32_t source, int32_t type, int32_t deviceId = 0) : source(source), type(type), deviceId(deviceId) {}
};

struct FakeKeyEvent : FakeInputEvent {
    int32_t action, keyCode, metaState;

    FakeKeyEvent(int32_t action, int32_t keyCode, int32_t metaState) : FakeInputEvent(AINPUT_SOURCE_KEYBOARD, AINPUT_EVENT_TYPE_KEY), action(action), keyCode(keyCode), metaState(metaState) {}
    FakeKeyEvent(int32_t source, int32_t deviceId, int32_t action, int32_t keyCode) : FakeInputEvent(source, AINPUT_EVENT_TYPE_KEY, deviceId), action(action), keyCode(keyCode), metaState(0) {}
    FakeKeyEvent() : FakeKeyEvent(0, 0, 0) {}
};

struct FakeMotionEvent : FakeInputEvent {
    int32_t action;
    int32_t pointerId;
    float x, y;
    std::function<float(int32_t axis)> axisFunction;
    int32_t btn = 0, dy = 0;

    FakeMotionEvent(int32_t source, int32_t action, int32_t pointerId, float x, float y) : FakeInputEvent(source, AINPUT_EVENT_TYPE_MOTION), action(action), pointerId(pointerId), x(x), y(y) {}

    FakeMotionEvent(int32_t source, int32_t action, int32_t pointerId, float x, float y, int32_t btn, int32_t dy) : FakeInputEvent(source, AINPUT_EVENT_TYPE_MOTION), action(action), pointerId(pointerId), x(x), y(y), btn(btn), dy(dy) {}

    FakeMotionEvent(int32_t source, int32_t deviceId, int32_t action, int32_t pointerId, float x, float y, std::function<float(int32_t axis)> axisFunction) : FakeInputEvent(source, AINPUT_EVENT_TYPE_MOTION, deviceId), action(action), pointerId(pointerId), x(x), y(y), axisFunction(std::move(axisFunction)) {}

    FakeMotionEvent() : FakeMotionEvent(0, 0, 0, 0, 0) {}
};

// Thin forwarding wrapper (Phase 1 of PORT_FAKE_LOOPER.md): storage + the
// libandroid.so input hooks now live in Rust (fake_inputqueue.rs). Each
// method forwards to the Rust FakeInputQueue via the mc_fake_input_queue_*
// FFI. The event structs above are kept so C++ WindowCallbacks/FakeLooper can
// keep constructing events with these types; the Rust hooks read the opaque
// axisFunction slot and the Rust layout is pinned by unit tests.
class FakeInputQueue {
private:
    void* rustQueue;

public:
    FakeInputQueue();
    ~FakeInputQueue();

    // Returns the Rust FakeInputQueue* this wrapper forwards to.
    void* rustQueuePtr() const { return rustQueue; }

    bool hasEvents() const;

    int getEvent(FakeInputEvent **event);

    void finishEvent(FakeInputEvent *event);

    void addEvent(FakeKeyEvent event);

    void addEvent(FakeMotionEvent event);
};
