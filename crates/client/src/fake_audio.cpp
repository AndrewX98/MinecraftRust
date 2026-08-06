#include "fake_audio.h"
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

extern "C" {
int32_t rust_audio_start(int32_t channels, int32_t sampleRate);
void rust_audio_push_i16(const int16_t* samples, int32_t count);
void rust_audio_stop();
void mc_fmod_set_sample_rate(int32_t sampleRate);
size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);
}

// Rust calls this from capi::setup_android_hooks to register the AAudio stubs
// with the Rust linker. Replaces the `rust_load_stub("libaaudio.so", …)` blocks
// that used to live in jni_bridge_stub.cpp; FakeAudio::initHybrisHooks stays C++.
extern "C" void mc_register_aaudio_stub(const char* name) {
    std::unordered_map<std::string, void*> audio_syms;
    FakeAudio::initHybrisHooks(audio_syms);
    size_t n = audio_syms.size();
    if (n == 0) {
        linker_load_library_rust(name, nullptr, nullptr, 0);
        return;
    }
    std::vector<const char*> keys(n);
    std::vector<void*> vals(n);
    size_t i = 0;
    for (auto& [k, v] : audio_syms) {
        keys[i] = k.c_str();
        vals[i] = v;
        i++;
    }
    linker_load_library_rust(name, keys.data(), vals.data(), n);
}

int32_t FakeAudio::defaultSampleRate = 48000;
int32_t FakeAudio::defaultNumChannels = 2;
int32_t FakeAudio::defaultBufSize = 512;

static int ReadEnvInt(const char* name, int def = 0) {
    auto val = getenv(name);
    if (!val) return def;
    return std::stoi(val);
}

void FakeAudio::initHybrisHooks(std::unordered_map<std::string, void*>& syms) {
    syms["AAudioStreamBuilder_openStream"] = (void*)+[](FakeAudioStreamBuilder* builder, FakeAudioStream** stream) -> aaudio_result_t {
        fprintf(stderr, "=== FakeAudio: AAudioStreamBuilder_openStream called ===\n");
        *stream = new FakeAudioStream{
            .dataCallback = builder->dataCallback,
            .dataCallbackUser = builder->dataCallbackUser,
            .errorCallback = builder->errorCallback,
            .errorCallbackUser = builder->errorCallbackUser,
            .bufferCap = builder->bufferCap
        };
        (*stream)->audioBufferSize = builder->bufferCap * (*stream)->getBytesPerSample() * (*stream)->channelCount;
        (*stream)->audioBuffer = malloc((*stream)->audioBufferSize);
        return AAUDIO_OK;
    };
    syms["AAudio_createStreamBuilder"] = (void*)+[](FakeAudioStreamBuilder** builder) -> aaudio_result_t {
        fprintf(stderr, "=== FakeAudio: AAudio_createStreamBuilder called ===\n");
        FakeAudio::updateDefaults();
        *builder = new FakeAudioStreamBuilder{};
        return AAUDIO_OK;
    };
    syms["AAudioStreamBuilder_setBufferCapacityInFrames"] = (void*)+[](FakeAudioStreamBuilder* builder, int32_t newCap) -> void {
        builder->bufferCap = newCap;
    };
    syms["AAudioStreamBuilder_setDataCallback"] = (void*)+[](FakeAudioStreamBuilder* builder, AAudioStream_dataCallback callback, void* userData) {
        builder->dataCallback = callback;
        builder->dataCallbackUser = userData;
    };
    // Real AAudio API (API 26+): int32_t AAudioStream_getXRunCount(AAudioStream*)
    // Returns the underrun/overrun count — NOT an out-pointer write. The previous
    // (stream, int32_t* outCount) stub treated the next register as a pointer
    // (often garbage like 0x81) and SIGSEGV'd on FMOD's main-thread poll.
    syms["AAudioStream_getXRunCount"] = (void*)+[](FakeAudioStream*) -> int32_t {
        return 0;
    };
    // Missing on older FakeAudio ports — FMOD 1.26+ may dlsym these.
    syms["AAudioStream_getDeviceId"] = (void*)+[](FakeAudioStream*) -> int32_t {
        return 0;
    };
    syms["AAudioStreamBuilder_setDeviceId"] = (void*)+[](FakeAudioStreamBuilder*, int32_t) {
    };
    syms["AAudioStreamBuilder_setSampleRate"] = (void*)+[](FakeAudioStreamBuilder* builder, int32_t rate) {
        // Stored on stream at open time via defaults; track on builder for openStream.
        (void)builder;
        (void)rate;
    };
    syms["AAudioStreamBuilder_setChannelCount"] = (void*)+[](FakeAudioStreamBuilder*, int32_t) {
    };
    syms["AAudioStreamBuilder_setFormat"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_format_t) {
    };
    syms["AAudioStreamBuilder_setSharingMode"] = (void*)+[](FakeAudioStreamBuilder*, int32_t) {
    };
    syms["AAudioStreamBuilder_setErrorCallback"] = (void*)+[](FakeAudioStreamBuilder* builder, AAudioStream_errorCallback callback, void* userData) {
        builder->errorCallback = callback;
        builder->errorCallbackUser = userData;
    };
    syms["AAudioStream_getBufferSizeInFrames"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->bufferSize;
    };
    // Real: aaudio_result_t AAudioStream_close(AAudioStream*)
    syms["AAudioStream_close"] = (void*)+[](FakeAudioStream* stream) -> aaudio_result_t {
        if (!stream) return AAUDIO_OK;
        stream->running = false;
        if (stream->playbackThread.joinable()) {
            stream->playbackThread.join();
        }
        free(stream->audioBuffer);
        stream->audioBuffer = nullptr;
        stream->audioBufferSize = 0;
        return AAUDIO_OK;
    };
    syms["AAudioStreamBuilder_setDirection"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_direction_t) {
    };
    // Real: returns actual buffer size in frames, or a negative error (aaudio_result_t).
    syms["AAudioStream_setBufferSizeInFrames"] = (void*)+[](FakeAudioStream* stream, int32_t newSize) -> aaudio_result_t {
        if (!stream || newSize <= 0) return -1;
        stream->bufferSize = newSize;
        stream->audioBufferSize = stream->bufferSize * stream->channelCount * stream->getBytesPerSample();
        stream->audioBuffer = realloc(stream->audioBuffer, stream->audioBufferSize);
        return stream->bufferSize;
    };
    syms["AAudioStream_getChannelCount"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->channelCount;
    };
    syms["AAudioStream_getFramesPerBurst"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->bufferSize;
    };
    // Real: void AAudioStreamBuilder_delete(AAudioStreamBuilder*)
    syms["AAudioStreamBuilder_delete"] = (void*)+[](FakeAudioStreamBuilder* builder) {
        delete builder;
    };
    // Real: aaudio_result_t AAudioStream_requestStop(AAudioStream*)
    syms["AAudioStream_requestStop"] = (void*)+[](FakeAudioStream* stream) -> aaudio_result_t {
        if (!stream) return AAUDIO_OK;
        stream->running = false;
        if (stream->playbackThread.joinable()) {
            stream->playbackThread.join();
        }
        rust_audio_stop();
        return AAUDIO_OK;
    };
    syms["AAudioStream_getBufferCapacityInFrames"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->bufferCap;
    };
    syms["AAudioStreamBuilder_setInputPreset"] = (void*)+[]() {
    };
    syms["AAudioStream_getSampleRate"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->sampleRate;
    };
    // Real: aaudio_result_t AAudioStream_read(stream, buffer, numFrames, timeoutNanos)
    // Callback-driven output streams don't use read; return 0 frames.
    syms["AAudioStream_read"] = (void*)+[](FakeAudioStream*, void*, int32_t, int64_t) -> aaudio_result_t {
        return 0;
    };
    syms["AAudioStreamBuilder_setPerformanceMode"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_performance_mode_t) -> void {
    };
    syms["AAudioStream_getState"] = (void*)+[](FakeAudioStream* stream) -> aaudio_stream_state_t {
        if (!stream || !stream->started) {
            return AAUDIO_STREAM_STATE_CLOSED;
        }
        return stream->running ? AAUDIO_STREAM_STATE_STARTED : AAUDIO_STREAM_STATE_STOPPED;
    };
    syms["AAudioStream_getFormat"] = (void*)+[](FakeAudioStream* stream) -> aaudio_format_t {
        return stream->format;
    };
    syms["AAudioStreamBuilder_setUsage"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_usage_t) {
    };
    syms["AAudioStream_requestStart"] = (void*)+[](FakeAudioStream* stream) -> aaudio_result_t {
        fprintf(stderr, "=== FakeAudio: AAudioStream_requestStart called ===\n");
        fprintf(stderr, "=== FakeAudio: requestStart stream=%p rate=%d ch=%d fmt=%d bufSize=%d dataCb=%p user=%p ===\n",
                (void*)stream, stream->sampleRate, stream->channelCount, (int)stream->format,
                stream->bufferSize, (void*)stream->dataCallback, stream->dataCallbackUser);
        stream->started = true;
        stream->running = true;
        rust_audio_start(stream->channelCount, stream->sampleRate);
        if (stream->dataCallback == nullptr) {
            return AAUDIO_OK;
        }
        int chunkFrames = stream->bufferSize > 0 ? stream->bufferSize : 512;
        int sampleRate = stream->sampleRate > 0 ? stream->sampleRate : 48000;
        stream->playbackThread = std::thread([stream, chunkFrames, sampleRate]() {
            std::vector<int16_t> scratch;
            while (stream->running.load()) {
                int bytesPerSample = stream->getBytesPerSample();
                int amount = chunkFrames * stream->channelCount * bytesPerSample;
                if (amount > stream->audioBufferSize) {
                    stream->audioBufferSize = amount;
                    stream->audioBuffer = realloc(stream->audioBuffer, stream->audioBufferSize);
                }
                // Zero buffer so underrun is silence if FMOD writes nothing.
                memset(stream->audioBuffer, 0, (size_t)amount);
                stream->dataCallback((AAudioStream*)stream, stream->dataCallbackUser, stream->audioBuffer, chunkFrames);
                int sampleCount = chunkFrames * stream->channelCount;
                switch (stream->format) {
                case AAUDIO_FORMAT_PCM_I16:
                    rust_audio_push_i16((const int16_t*)stream->audioBuffer, sampleCount);
                    break;
                case AAUDIO_FORMAT_PCM_I32: {
                    scratch.resize(sampleCount);
                    const int32_t* src = (const int32_t*)stream->audioBuffer;
                    for (int i = 0; i < sampleCount; i++) {
                        scratch[i] = (int16_t)(src[i] >> 16);
                    }
                    rust_audio_push_i16(scratch.data(), sampleCount);
                    break;
                }
                case AAUDIO_FORMAT_PCM_FLOAT: {
                    scratch.resize(sampleCount);
                    const float* src = (const float*)stream->audioBuffer;
                    for (int i = 0; i < sampleCount; i++) {
                        float v = src[i];
                        if (v > 1.0f) v = 1.0f;
                        else if (v < -1.0f) v = -1.0f;
                        scratch[i] = (int16_t)(v * 32767.0f);
                    }
                    rust_audio_push_i16(scratch.data(), sampleCount);
                    break;
                }
                default:
                    break;
                }
                int64_t chunkUs = (int64_t)chunkFrames * 1000000 / sampleRate;
                std::this_thread::sleep_for(std::chrono::microseconds(chunkUs));
            }
        });
        fprintf(stderr, "=== FakeAudio: requestStart DONE ===\n");
        return AAUDIO_OK;
    };
}

void FakeAudio::updateDefaults() {
    defaultSampleRate = ReadEnvInt("AUDIO_SAMPLE_RATE", 48000);
    defaultNumChannels = ReadEnvInt("AUDIO_CHANNEL_COUNT", 2);
    defaultBufSize = ReadEnvInt("AUDIO_BUFFER_FRAMES", 512);

    mc_fmod_set_sample_rate(defaultSampleRate);
}
