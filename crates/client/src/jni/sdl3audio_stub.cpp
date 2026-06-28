// Stub replacing sdl3audio.cpp for the Rust build.
// The actual audio implementation is handled by the Rust audio module (audio.rs).

#include "sdl3audio.h"

AudioDevice::AudioDevice() {
    s = nullptr;
}

AudioDevice::~AudioDevice() {
}

FakeJni::JBoolean AudioDevice::init(FakeJni::JInt channels, FakeJni::JInt samplerate, FakeJni::JInt c, FakeJni::JInt d) {
    // Implemented in Rust audio.rs
    return false;
}

void AudioDevice::write(std::shared_ptr<FakeJni::JByteArray> data, FakeJni::JInt length) {
    // Implemented in Rust audio.rs
}

void AudioDevice::write2(std::shared_ptr<FakeJni::JShortArray> data, FakeJni::JInt length) {
    // Implemented in Rust audio.rs
}

void AudioDevice::close() {
    // Implemented in Rust audio.rs
}
