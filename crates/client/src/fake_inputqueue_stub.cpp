/// Stub replacing fake_inputqueue.cpp for the Rust build.
/// Phase 1 of PORT_FAKE_LOOPER.md: the storage + libandroid.so input hooks
/// moved to Rust (fake_inputqueue.rs). This class is now a thin forwarding
/// wrapper so C++ WindowCallbacks/FakeLooper keep working with the C++ event
/// struct types while Rust owns the queues.

#include "fake_inputqueue.h"
#include <stdexcept>

extern "C" {
    void* mc_fake_input_queue_create();
    void mc_fake_input_queue_destroy(void* queue);
    int mc_fake_input_queue_get_event(void* queue, void** outEvent);
    int mc_fake_input_queue_finish_event(void* queue, void* event);
    void mc_fake_input_queue_add_key_event(void* queue, const void* event);
    void mc_fake_input_queue_add_motion_event(void* queue, const void* event);
    bool mc_fake_input_queue_has_events(void* queue);
}

// Accessor used by the Rust libandroid.so hooks to reach the Rust-owned queue
// behind this C++ wrapper.
extern "C" void* fake_input_queue_get_rust(void* queue) {
    if(!queue) return nullptr;
    return ((FakeInputQueue*)queue)->rustQueuePtr();
}

FakeInputQueue::FakeInputQueue() : rustQueue(mc_fake_input_queue_create()) {
    if(!rustQueue)
        throw std::runtime_error("mc_fake_input_queue_create returned null");
}

FakeInputQueue::~FakeInputQueue() {
    if(rustQueue)
        mc_fake_input_queue_destroy(rustQueue);
}

bool FakeInputQueue::hasEvents() const {
    return mc_fake_input_queue_has_events(rustQueue);
}

int FakeInputQueue::getEvent(FakeInputEvent **event) {
    return mc_fake_input_queue_get_event(rustQueue, (void**)event);
}

void FakeInputQueue::finishEvent(FakeInputEvent *event) {
    if(mc_fake_input_queue_finish_event(rustQueue, event) != 0)
        throw std::runtime_error("finishEvent: the event is not the event on the front of queue");
}

void FakeInputQueue::addEvent(FakeKeyEvent event) {
    mc_fake_input_queue_add_key_event(rustQueue, &event);
}

void FakeInputQueue::addEvent(FakeMotionEvent event) {
    mc_fake_input_queue_add_motion_event(rustQueue, &event);
}
