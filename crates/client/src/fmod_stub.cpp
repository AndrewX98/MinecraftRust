#include "jni/fmod.h"

FMOD::FMOD() {}
FMOD::~FMOD() {}

FakeJni::JBoolean FMOD::checkInit() { return true; }

FakeJni::JBoolean FMOD::supportsLowLatency() { return true; }

FakeJni::JBoolean FMOD::supportsAAudio() {
    // Always true: FakeAudio registers a full AAudio shim (SDL3 backend) as
    // libaaudio.so. Returning false makes FMOD fall back to OpenSL ES, which
    // we only stub as an empty library — FMOD's "Streaming Pool" thread then
    // SIGSEGVs on the first null SL* function pointer.
    //
    // Upstream gates this on HAVE_SDL3AUDIO; we always compile fake_audio.cpp
    // and link SDL3, so AAudio is always available.
    return true;
}

std::shared_ptr<AssetManager> FMOD::getAssetManager() {
    return std::make_shared<AssetManager>();
}
