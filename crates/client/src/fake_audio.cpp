#include "fake_audio.h"
#include <game_window_manager.h>
#include <mcpelauncher/fmod_utils.h>
#include <cstdlib>
#include <cstring>
#include <string>

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
        SDL_Init(SDL_INIT_AUDIO);
        SDL_SetHint(SDL_HINT_AUDIO_DEVICE_APP_ICON_NAME, "mcpelauncher");
        SDL_SetHint(SDL_HINT_AUDIO_DEVICE_STREAM_NAME, "Minecraft");
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
    // Real signature: aaudio_result_t AAudioStream_getXRunCount(AAudioStream*, int32_t* outCount)
    syms["AAudioStream_getXRunCount"] = (void*)+[](FakeAudioStream*, int32_t* outCount) -> aaudio_result_t {
        if (outCount) *outCount = 0;
        return AAUDIO_OK;
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
    syms["AAudioStream_close"] = (void*)+[](FakeAudioStream* stream) {
        free(stream->audioBuffer);
        stream->audioBuffer = nullptr;
        stream->audioBufferSize = 0;
    };
    syms["AAudioStreamBuilder_setDirection"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_direction_t) {
    };
    // Real API returns the applied size (int32_t), not void.
    syms["AAudioStream_setBufferSizeInFrames"] = (void*)+[](FakeAudioStream* stream, int32_t newSize) -> int32_t {
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
    syms["AAudioStreamBuilder_delete"] = (void*)+[]() {
    };
    syms["AAudioStream_requestStop"] = (void*)+[](FakeAudioStream* stream) {
        SDL_AudioStream* s = stream->s;
        stream->s = nullptr;
        SDL_DestroyAudioStream(s);
    };
    syms["AAudioStream_getBufferCapacityInFrames"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->bufferCap;
    };
    syms["AAudioStreamBuilder_setInputPreset"] = (void*)+[]() {
    };
    syms["AAudioStream_getSampleRate"] = (void*)+[](FakeAudioStream* stream) -> int32_t {
        return stream->sampleRate;
    };
    syms["AAudioStream_read"] = (void*)+[]() {
    };
    syms["AAudioStreamBuilder_setPerformanceMode"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_performance_mode_t) -> void {
    };
    syms["AAudioStream_getState"] = (void*)+[](FakeAudioStream* stream) -> aaudio_stream_state_t {
        if (!stream->s) {
            return AAUDIO_STREAM_STATE_CLOSED;
        }
        SDL_AudioDeviceID devid = SDL_GetAudioStreamDevice(stream->s);
        if (!devid) {
            return AAUDIO_STREAM_STATE_CLOSED;
        }
        return SDL_AudioDevicePaused(devid) ? AAUDIO_STREAM_STATE_PAUSED : AAUDIO_STREAM_STATE_STARTED;
    };
    syms["AAudioStream_getFormat"] = (void*)+[](FakeAudioStream* stream) -> aaudio_format_t {
        return stream->format;
    };
    syms["AAudioStreamBuilder_setUsage"] = (void*)+[](FakeAudioStreamBuilder*, aaudio_usage_t) {
    };
    syms["AAudioStream_requestStart"] = (void*)+[](FakeAudioStream* stream) -> aaudio_result_t {
        fprintf(stderr, "=== FakeAudio: AAudioStream_requestStart called ===\n");
        SDL_AudioSpec spec;
        spec.channels = stream->channelCount;
        switch (stream->format) {
        case AAUDIO_FORMAT_PCM_I16:
            spec.format = SDL_AUDIO_S16LE;
            break;
        case AAUDIO_FORMAT_PCM_I32:
            spec.format = SDL_AUDIO_S32LE;
            break;
        default:
            spec.format = SDL_AUDIO_S16LE;
            break;
        }
        spec.freq = stream->sampleRate;
        fprintf(stderr, "=== FakeAudio: requestStart stream=%p rate=%d ch=%d fmt=%d bufSize=%d dataCb=%p user=%p ===\n",
                (void*)stream, stream->sampleRate, stream->channelCount, (int)stream->format,
                stream->bufferSize, (void*)stream->dataCallback, stream->dataCallbackUser);
        stream->s = SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, &spec,
            [](void* userdata, SDL_AudioStream* sdlStream, int additional_amount, int total_amount) {
                FakeAudioStream* stream = (FakeAudioStream*)userdata;
                static int cb_count = 0;
                if (stream->dataCallback == nullptr || stream->s == nullptr || stream->audioBuffer == nullptr) {
                    return;
                }
                if (additional_amount > stream->audioBufferSize) {
                    stream->audioBufferSize = additional_amount;
                    stream->audioBuffer = realloc(stream->audioBuffer, stream->audioBufferSize);
                }
                int frames = additional_amount / (stream->channelCount * stream->getBytesPerSample());
                if (frames <= 0) {
                    return;
                }
                // Zero buffer so underrun is silence if FMOD writes nothing.
                memset(stream->audioBuffer, 0, (size_t)additional_amount);
                if (cb_count < 3) {
                    fprintf(stderr, "=== FakeAudio: dataCallback #%d frames=%d amount=%d cb=%p user=%p ===\n",
                            cb_count, frames, additional_amount, (void*)stream->dataCallback, stream->dataCallbackUser);
                }
                cb_count++;
                stream->dataCallback((AAudioStream*)stream, stream->dataCallbackUser, stream->audioBuffer, frames);
                if (cb_count <= 3) {
                    fprintf(stderr, "=== FakeAudio: dataCallback #%d returned ===\n", cb_count - 1);
                }
                if (!SDL_PutAudioStreamData(stream->s, stream->audioBuffer, additional_amount)) {
                    if (stream->errorCallback != nullptr) {
                        stream->errorCallback((AAudioStream*)stream, stream->errorCallbackUser, AAUDIO_ERROR_DISCONNECTED);
                    }
                }
            }, stream);
        if (stream->s == nullptr) {
            auto errormsg = SDL_GetError();
            fprintf(stderr, "=== FakeAudio: SDL_OpenAudioDeviceStream FAILED: %s ===\n",
                    errormsg ? errormsg : "(null)");
            auto handler = GameWindowManager::getManager()->getErrorHandler();
            if (handler) {
                handler->onError("sdl3audio failed",
                    std::string("sdl3audio SDL_OpenAudioDeviceStream failed, audio will be unavailable: ") + (errormsg ? errormsg : "No message"));
            }
            return AAUDIO_OK;  // fmod retries on failure
        }
        fprintf(stderr, "=== FakeAudio: SDL stream opened s=%p, resuming ===\n", (void*)stream->s);
        SDL_ResumeAudioDevice(SDL_GetAudioStreamDevice(stream->s));
        fprintf(stderr, "=== FakeAudio: requestStart DONE ===\n");
        return AAUDIO_OK;
    };
}

void FakeAudio::updateDefaults() {
    SDL_AudioSpec spec;
    int sampleFrames;
    SDL_GetAudioDeviceFormat(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, &spec, &sampleFrames);

    defaultSampleRate = ReadEnvInt("AUDIO_SAMPLE_RATE", spec.freq);
    defaultNumChannels = spec.channels;
    defaultBufSize = sampleFrames;

    FmodUtils::setSampleRate(defaultSampleRate);
}
