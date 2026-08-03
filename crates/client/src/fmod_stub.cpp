#include "jni/fmod.h"

FMOD::FMOD() {}
FMOD::~FMOD() {}

FakeJni::JBoolean FMOD::checkInit() { return true; }

FakeJni::JBoolean FMOD::supportsLowLatency() { return true; }

FakeJni::JBoolean FMOD::supportsAAudio() {
    // Always true: FakeAudio registers a full AAudio shim as libaaudio.so.
    // Returning false makes FMOD fall back to OpenSL ES, which we only stub
    // as an empty library — FMOD's "Streaming Pool" thread then SIGSEGVs on
    // the first null SL* function pointer. Audio is routed through the Rust
    // cpal backend (crates/client/src/jni/audio.rs) via rust_audio_*.
    return true;
}

std::shared_ptr<AssetManager> FMOD::getAssetManager() {
    return std::make_shared<AssetManager>();
}
