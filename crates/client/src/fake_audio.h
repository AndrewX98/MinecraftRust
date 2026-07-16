#pragma once

#include <unordered_map>
#include <string>
#include <cstdint>
#include <cstddef>

#include <SDL3/SDL.h>

// AAudio types self-contained — the Android NDK audio.h uses _Nonnull/_Nullable
// annotations that GCC rejects. We only need the types that FMOD's AAudio backend
// touches: enums, opaque handles, and callback typedefs.
typedef int32_t aaudio_result_t;
typedef int32_t aaudio_stream_state_t;
typedef int32_t aaudio_format_t;
typedef int32_t aaudio_direction_t;
typedef int32_t aaudio_performance_mode_t;
typedef int32_t aaudio_usage_t;
typedef int32_t aaudio_sharing_mode_t;
typedef int32_t aaudio_content_type_t;
typedef int32_t aaudio_input_preset_t;
typedef int32_t aaudio_session_id_t;
typedef int32_t aaudio_channel_mask_t;

#define AAUDIO_OK 0
#define AAUDIO_ERROR_DISCONNECTED -3

// Enum values FMOD actually uses
enum {
    AAUDIO_DIRECTION_OUTPUT = 0,
    AAUDIO_DIRECTION_INPUT = 1,
};
enum {
    AAUDIO_FORMAT_INVALID = -1,
    AAUDIO_FORMAT_UNSPECIFIED = 0,
    AAUDIO_FORMAT_PCM_I16 = 1,
    AAUDIO_FORMAT_PCM_FLOAT = 2,
    AAUDIO_FORMAT_PCM_I24_PACKED = 3,
    AAUDIO_FORMAT_PCM_I32 = 4,
};
enum {
    AAUDIO_STREAM_STATE_UNINITIALIZED = 0,
    AAUDIO_STREAM_STATE_STOPPING = 2,
    AAUDIO_STREAM_STATE_STOPPED = 3,
    AAUDIO_STREAM_STATE_STARTING = 4,
    AAUDIO_STREAM_STATE_STARTED = 5,
    AAUDIO_STREAM_STATE_FLUSHING = 6,
    AAUDIO_STREAM_STATE_FLUSHED = 7,
    AAUDIO_STREAM_STATE_CLOSED = 8,
};
// State aliases for paused
#define AAUDIO_STREAM_STATE_PAUSED 1

// Opaque handles
struct AAudioStreamBuilder;
struct AAudioStream;

// Callback types
typedef void (*AAudioStream_dataCallback)(AAudioStream* stream, void* userData, void* audioData, int32_t numFrames);
typedef void (*AAudioStream_errorCallback)(AAudioStream* stream, void* userData, aaudio_result_t error);

class FakeAudio {
private:
    static int32_t defaultSampleRate;
    static int32_t defaultNumChannels;
    static int32_t defaultBufSize;

    struct FakeAudioStreamBuilder {
        AAudioStream_dataCallback dataCallback = nullptr;
        void *dataCallbackUser = nullptr;

        AAudioStream_errorCallback errorCallback = nullptr;
        void *errorCallbackUser = nullptr;

        int32_t bufferCap = defaultBufSize;
    };

    struct FakeAudioStream {
        AAudioStream_dataCallback dataCallback;
        void *dataCallbackUser;

        AAudioStream_errorCallback errorCallback;
        void *errorCallbackUser;

        int32_t bufferCap;
        int32_t bufferSize = defaultBufSize;
        int32_t sampleRate = defaultSampleRate;
        int32_t channelCount = defaultNumChannels;

        aaudio_format_t format = AAUDIO_FORMAT_PCM_I16;

        void *audioBuffer;
        int audioBufferSize = 0;

        SDL_AudioStream *s = nullptr;

        int32_t getBytesPerSample() {
            switch (format) {
            case AAUDIO_FORMAT_INVALID:
                return 0;
            case AAUDIO_FORMAT_PCM_I16:
                return 2;
            case AAUDIO_FORMAT_PCM_FLOAT:
            case AAUDIO_FORMAT_PCM_I32:
                return 4;
            case AAUDIO_FORMAT_PCM_I24_PACKED:
                return 3;
            default:
                return 1;
            }
        }
    };

public:
    static void initHybrisHooks(std::unordered_map<std::string, void *> &syms);
    static void updateDefaults();
};
