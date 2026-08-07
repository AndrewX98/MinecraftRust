//! Rust port of the C++ `fake_audio.cpp`/`fake_audio.h` (AAudio stub backend).
//! FMOD's setOutput hook keeps AAudio; FMOD then dlopen's libaaudio.so and
//! calls the `AAudio*` API. The real libaaudio.so is never loaded — these
//! symbols are registered with the Rust linker via `mc_register_aaudio_stub`
//! (called from `crate::capi::setup_android_hooks`). The game treats
//! `AAudioStreamBuilder`/`AAudioStream` as opaque handles, so the struct
//! layout is launcher-internal; only pointer identity crosses the boundary.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

const AAUDIO_OK: i32 = 0;

const AAUDIO_FORMAT_INVALID: i32 = -1;
const AAUDIO_FORMAT_PCM_I16: i32 = 1;
const AAUDIO_FORMAT_PCM_FLOAT: i32 = 2;
const AAUDIO_FORMAT_PCM_I24_PACKED: i32 = 3;
const AAUDIO_FORMAT_PCM_I32: i32 = 4;

const AAUDIO_STREAM_STATE_STARTED: i32 = 5;
const AAUDIO_STREAM_STATE_STOPPED: i32 = 3;
const AAUDIO_STREAM_STATE_CLOSED: i32 = 8;

type AudioDataCallback = extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i32);
type AudioErrorCallback = extern "C" fn(*mut c_void, *mut c_void, i32);

#[repr(C)]
struct FakeAudioStreamBuilder {
    data_callback: Option<AudioDataCallback>,
    data_callback_user: *mut c_void,
    error_callback: Option<AudioErrorCallback>,
    error_callback_user: *mut c_void,
    buffer_cap: i32,
}

#[repr(C)]
struct FakeAudioStream {
    data_callback: Option<AudioDataCallback>,
    data_callback_user: *mut c_void,
    error_callback: Option<AudioErrorCallback>,
    error_callback_user: *mut c_void,
    buffer_cap: i32,
    buffer_size: i32,
    sample_rate: i32,
    channel_count: i32,
    format: i32,
    audio_buffer: *mut c_void,
    audio_buffer_size: usize,
    playback_thread: Option<JoinHandle<()>>,
    started: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

fn get_bytes_per_sample(format: i32) -> i32 {
    match format {
        AAUDIO_FORMAT_INVALID => 0,
        AAUDIO_FORMAT_PCM_I16 => 2,
        AAUDIO_FORMAT_PCM_FLOAT | AAUDIO_FORMAT_PCM_I32 => 4,
        AAUDIO_FORMAT_PCM_I24_PACKED => 3,
        _ => 1,
    }
}

fn read_env_int(name: &str, def: i32) -> i32 {
    match std::env::var(name) {
        Ok(v) => v.parse::<i32>().unwrap_or(def),
        Err(_) => def,
    }
}

fn update_defaults() {
    let sample_rate = read_env_int("AUDIO_SAMPLE_RATE", 48000);
    let channels = read_env_int("AUDIO_CHANNEL_COUNT", 2);
    let buf = read_env_int("AUDIO_BUFFER_FRAMES", 512);
    DEFAULT_SAMPLE_RATE.store(sample_rate, Ordering::SeqCst);
    DEFAULT_NUM_CHANNELS.store(channels, Ordering::SeqCst);
    DEFAULT_BUF_SIZE.store(buf, Ordering::SeqCst);
    crate::fmod_utils::mc_fmod_set_sample_rate(sample_rate);
}

static DEFAULT_SAMPLE_RATE: AtomicI32 = AtomicI32::new(48000);
static DEFAULT_NUM_CHANNELS: AtomicI32 = AtomicI32::new(2);
static DEFAULT_BUF_SIZE: AtomicI32 = AtomicI32::new(512);

fn default_sample_rate() -> i32 {
    DEFAULT_SAMPLE_RATE.load(Ordering::SeqCst)
}
fn default_num_channels() -> i32 {
    DEFAULT_NUM_CHANNELS.load(Ordering::SeqCst)
}
fn default_buf_size() -> i32 {
    DEFAULT_BUF_SIZE.load(Ordering::SeqCst)
}

// ---- AAudio stub functions -------------------------------------------------

extern "C" fn aaudio_open_stream(
    builder: *mut FakeAudioStreamBuilder,
    stream: *mut *mut FakeAudioStream,
) -> i32 {
    eprintln!("=== FakeAudio: AAudioStreamBuilder_openStream called ===");
    if builder.is_null() || stream.is_null() {
        return AAUDIO_OK;
    }
    let builder = unsafe { &*builder };
    let s = Box::new(FakeAudioStream {
        data_callback: builder.data_callback,
        data_callback_user: builder.data_callback_user,
        error_callback: builder.error_callback,
        error_callback_user: builder.error_callback_user,
        buffer_cap: builder.buffer_cap,
        buffer_size: default_buf_size(),
        sample_rate: default_sample_rate(),
        channel_count: default_num_channels(),
        format: AAUDIO_FORMAT_PCM_I16,
        audio_buffer: std::ptr::null_mut(),
        audio_buffer_size: 0,
        playback_thread: None,
        started: Arc::new(AtomicBool::new(false)),
        running: Arc::new(AtomicBool::new(false)),
    });
    let s = Box::into_raw(s);
    unsafe {
        (*s).audio_buffer_size =
            (builder.buffer_cap * get_bytes_per_sample((*s).format) * (*s).channel_count)
                .max(0) as usize;
        (*s).audio_buffer = libc::malloc((*s).audio_buffer_size.max(1));
        *stream = s;
    }
    AAUDIO_OK
}

extern "C" fn aaudio_create_stream_builder(builder: *mut *mut FakeAudioStreamBuilder) -> i32 {
    eprintln!("=== FakeAudio: AAudio_createStreamBuilder called ===");
    update_defaults();
    if !builder.is_null() {
        let b = Box::new(FakeAudioStreamBuilder {
            data_callback: None,
            data_callback_user: std::ptr::null_mut(),
            error_callback: None,
            error_callback_user: std::ptr::null_mut(),
            buffer_cap: default_buf_size(),
        });
        unsafe { *builder = Box::into_raw(b) };
    }
    AAUDIO_OK
}

extern "C" fn aaudio_stream_builder_set_buffer_capacity_in_frames(
    builder: *mut FakeAudioStreamBuilder,
    new_cap: i32,
) {
    if !builder.is_null() {
        unsafe { (*builder).buffer_cap = new_cap };
    }
}

extern "C" fn aaudio_stream_builder_set_data_callback(
    builder: *mut FakeAudioStreamBuilder,
    callback: Option<AudioDataCallback>,
    user_data: *mut c_void,
) {
    if !builder.is_null() {
        unsafe {
            (*builder).data_callback = callback;
            (*builder).data_callback_user = user_data;
        }
    }
}

extern "C" fn aaudio_stream_get_xrun_count(_stream: *mut FakeAudioStream) -> i32 {
    0
}

extern "C" fn aaudio_stream_get_device_id(_stream: *mut FakeAudioStream) -> i32 {
    0
}

extern "C" fn aaudio_stream_builder_set_device_id(
    _builder: *mut FakeAudioStreamBuilder,
    _id: i32,
) {
}

extern "C" fn aaudio_stream_builder_set_sample_rate(
    _builder: *mut FakeAudioStreamBuilder,
    _rate: i32,
) {
}

extern "C" fn aaudio_stream_builder_set_channel_count(
    _builder: *mut FakeAudioStreamBuilder,
    _count: i32,
) {
}

extern "C" fn aaudio_stream_builder_set_format(
    _builder: *mut FakeAudioStreamBuilder,
    _format: i32,
) {
}

extern "C" fn aaudio_stream_builder_set_sharing_mode(
    _builder: *mut FakeAudioStreamBuilder,
    _mode: i32,
) {
}

extern "C" fn aaudio_stream_builder_set_error_callback(
    builder: *mut FakeAudioStreamBuilder,
    callback: Option<AudioErrorCallback>,
    user_data: *mut c_void,
) {
    if !builder.is_null() {
        unsafe {
            (*builder).error_callback = callback;
            (*builder).error_callback_user = user_data;
        }
    }
}

extern "C" fn aaudio_stream_get_buffer_size_in_frames(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        0
    } else {
        unsafe { (*stream).buffer_size }
    }
}

extern "C" fn aaudio_stream_close(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        return AAUDIO_OK;
    }
    let stream = unsafe { &mut *stream };
    stream.running.store(false, Ordering::SeqCst);
    if let Some(h) = stream.playback_thread.take() {
        let _ = h.join();
    }
    unsafe {
        if !stream.audio_buffer.is_null() {
            libc::free(stream.audio_buffer);
        }
    }
    stream.audio_buffer = std::ptr::null_mut();
    stream.audio_buffer_size = 0;
    AAUDIO_OK
}

extern "C" fn aaudio_stream_builder_set_direction(
    _builder: *mut FakeAudioStreamBuilder,
    _direction: i32,
) {
}

extern "C" fn aaudio_stream_set_buffer_size_in_frames(
    stream: *mut FakeAudioStream,
    new_size: i32,
) -> i32 {
    if stream.is_null() || new_size <= 0 {
        return -1;
    }
    let stream = unsafe { &mut *stream };
    stream.buffer_size = new_size;
    stream.audio_buffer_size =
        (stream.buffer_size * stream.channel_count * get_bytes_per_sample(stream.format)).max(0)
            as usize;
    stream.audio_buffer = unsafe { libc::realloc(stream.audio_buffer, stream.audio_buffer_size.max(1)) };
    stream.buffer_size
}

extern "C" fn aaudio_stream_get_channel_count(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        0
    } else {
        unsafe { (*stream).channel_count }
    }
}

extern "C" fn aaudio_stream_get_frames_per_burst(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        0
    } else {
        unsafe { (*stream).buffer_size }
    }
}

extern "C" fn aaudio_stream_builder_delete(builder: *mut FakeAudioStreamBuilder) {
    if !builder.is_null() {
        unsafe { drop(Box::from_raw(builder)) };
    }
}

extern "C" fn aaudio_stream_request_stop(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        return AAUDIO_OK;
    }
    let stream = unsafe { &mut *stream };
    stream.running.store(false, Ordering::SeqCst);
    if let Some(h) = stream.playback_thread.take() {
        let _ = h.join();
    }
    crate::jni::audio::rust_audio_stop();
    AAUDIO_OK
}

extern "C" fn aaudio_stream_get_buffer_capacity_in_frames(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        0
    } else {
        unsafe { (*stream).buffer_cap }
    }
}

extern "C" fn aaudio_stream_builder_set_input_preset(
    _builder: *mut FakeAudioStreamBuilder,
    _preset: i32,
) {
}

extern "C" fn aaudio_stream_get_sample_rate(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        0
    } else {
        unsafe { (*stream).sample_rate }
    }
}

extern "C" fn aaudio_stream_read(
    _stream: *mut FakeAudioStream,
    _buffer: *mut c_void,
    _num_frames: i32,
    _timeout_nanos: i64,
) -> i32 {
    0
}

extern "C" fn aaudio_stream_builder_set_performance_mode(
    _builder: *mut FakeAudioStreamBuilder,
    _mode: i32,
) {
}

extern "C" fn aaudio_stream_get_state(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() || !unsafe { (*stream).started.load(Ordering::SeqCst) } {
        return AAUDIO_STREAM_STATE_CLOSED;
    }
    if unsafe { (*stream).running.load(Ordering::SeqCst) } {
        AAUDIO_STREAM_STATE_STARTED
    } else {
        AAUDIO_STREAM_STATE_STOPPED
    }
}

extern "C" fn aaudio_stream_get_format(stream: *mut FakeAudioStream) -> i32 {
    if stream.is_null() {
        AAUDIO_FORMAT_INVALID
    } else {
        unsafe { (*stream).format }
    }
}

extern "C" fn aaudio_stream_builder_set_usage(
    _builder: *mut FakeAudioStreamBuilder,
    _usage: i32,
) {
}

extern "C" fn aaudio_stream_request_start(stream: *mut FakeAudioStream) -> i32 {
    eprintln!("=== FakeAudio: AAudioStream_requestStart called ===");
    if stream.is_null() {
        return AAUDIO_OK;
    }
    let stream = unsafe { &mut *stream };
    eprintln!(
        "=== FakeAudio: requestStart stream={:p} rate={} ch={} fmt={} bufSize={} dataCb={:?} user={:p} ===",
        stream,
        stream.sample_rate,
        stream.channel_count,
        stream.format,
        stream.buffer_size,
        stream.data_callback,
        stream.data_callback_user,
    );
    stream.started.store(true, Ordering::SeqCst);
    stream.running.store(true, Ordering::SeqCst);
    crate::jni::audio::rust_audio_start(stream.channel_count, stream.sample_rate);
    let data_callback = stream.data_callback;
    if data_callback.is_none() {
        return AAUDIO_OK;
    }
    let chunk_frames = if stream.buffer_size > 0 { stream.buffer_size } else { 512 };
    let sample_rate = if stream.sample_rate > 0 { stream.sample_rate } else { 48000 };
    let channel_count = stream.channel_count;
    let format = stream.format;
    let data_callback_user = stream.data_callback_user as usize;
    let stream_ptr = stream as *mut FakeAudioStream as usize;
    let running = stream.running.clone();
    let handle = std::thread::spawn(move || {
        let stream_ptr = stream_ptr as *mut FakeAudioStream;
        let data_callback_user = data_callback_user as *mut c_void;
        let mut scratch: Vec<i16> = Vec::new();
        while running.load(Ordering::SeqCst) {
            let stream_ref = unsafe { &mut *stream_ptr };
            let bytes_per_sample = get_bytes_per_sample(format);
            let amount = chunk_frames * channel_count * bytes_per_sample;
            if (amount as usize) > stream_ref.audio_buffer_size {
                stream_ref.audio_buffer_size = amount.max(1) as usize;
                stream_ref.audio_buffer =
                    unsafe { libc::realloc(stream_ref.audio_buffer, stream_ref.audio_buffer_size) };
            }
            unsafe { libc::memset(stream_ref.audio_buffer, 0, amount as usize) };
            let cb = data_callback.expect("data callback checked before spawn");
            unsafe {
                cb(
                    stream_ptr as *mut c_void,
                    data_callback_user,
                    stream_ref.audio_buffer,
                    chunk_frames,
                )
            };
            let sample_count = chunk_frames * channel_count;
            match format {
                AAUDIO_FORMAT_PCM_I16 => {
                    unsafe {
                        crate::jni::audio::rust_audio_push_i16(
                            stream_ref.audio_buffer as *const i16,
                            sample_count,
                        )
                    };
                }
                AAUDIO_FORMAT_PCM_I32 => {
                    scratch.resize(sample_count.max(0) as usize, 0);
                    let src = stream_ref.audio_buffer as *const i32;
                    for i in 0..sample_count as usize {
                        scratch[i] = (unsafe { *src.add(i) } >> 16) as i16;
                    }
                    unsafe {
                        crate::jni::audio::rust_audio_push_i16(scratch.as_ptr(), sample_count)
                    };
                }
                AAUDIO_FORMAT_PCM_FLOAT => {
                    scratch.resize(sample_count.max(0) as usize, 0);
                    let src = stream_ref.audio_buffer as *const f32;
                    for i in 0..sample_count as usize {
                        let v = unsafe { *src.add(i) };
                        let v = if v > 1.0f32 { 1.0 } else if v < -1.0f32 { -1.0 } else { v };
                        scratch[i] = (v * 32767.0f32) as i16;
                    }
                    unsafe {
                        crate::jni::audio::rust_audio_push_i16(scratch.as_ptr(), sample_count)
                    };
                }
                _ => {}
            }
            let chunk_us = chunk_frames as i64 * 1000000 / sample_rate as i64;
            std::thread::sleep(Duration::from_micros(chunk_us.max(0) as u64));
        }
    });
    stream.playback_thread = Some(handle);
    eprintln!("=== FakeAudio: requestStart DONE ===");
    AAUDIO_OK
}

// ---- Symbol table + linker registration -----------------------------------

fn insert_sym(syms: &mut HashMap<String, *mut c_void>, name: &str, f: *mut c_void) {
    syms.insert(name.to_string(), f);
}

pub fn init_hybris_hooks(syms: &mut HashMap<String, *mut c_void>) {
    insert_sym(syms, "AAudioStreamBuilder_openStream", aaudio_open_stream as *mut c_void);
    insert_sym(syms, "AAudio_createStreamBuilder", aaudio_create_stream_builder as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setBufferCapacityInFrames", aaudio_stream_builder_set_buffer_capacity_in_frames as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setDataCallback", aaudio_stream_builder_set_data_callback as *mut c_void);
    insert_sym(syms, "AAudioStream_getXRunCount", aaudio_stream_get_xrun_count as *mut c_void);
    insert_sym(syms, "AAudioStream_getDeviceId", aaudio_stream_get_device_id as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setDeviceId", aaudio_stream_builder_set_device_id as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setSampleRate", aaudio_stream_builder_set_sample_rate as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setChannelCount", aaudio_stream_builder_set_channel_count as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setFormat", aaudio_stream_builder_set_format as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setSharingMode", aaudio_stream_builder_set_sharing_mode as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setErrorCallback", aaudio_stream_builder_set_error_callback as *mut c_void);
    insert_sym(syms, "AAudioStream_getBufferSizeInFrames", aaudio_stream_get_buffer_size_in_frames as *mut c_void);
    insert_sym(syms, "AAudioStream_close", aaudio_stream_close as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setDirection", aaudio_stream_builder_set_direction as *mut c_void);
    insert_sym(syms, "AAudioStream_setBufferSizeInFrames", aaudio_stream_set_buffer_size_in_frames as *mut c_void);
    insert_sym(syms, "AAudioStream_getChannelCount", aaudio_stream_get_channel_count as *mut c_void);
    insert_sym(syms, "AAudioStream_getFramesPerBurst", aaudio_stream_get_frames_per_burst as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_delete", aaudio_stream_builder_delete as *mut c_void);
    insert_sym(syms, "AAudioStream_requestStop", aaudio_stream_request_stop as *mut c_void);
    insert_sym(syms, "AAudioStream_getBufferCapacityInFrames", aaudio_stream_get_buffer_capacity_in_frames as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setInputPreset", aaudio_stream_builder_set_input_preset as *mut c_void);
    insert_sym(syms, "AAudioStream_getSampleRate", aaudio_stream_get_sample_rate as *mut c_void);
    insert_sym(syms, "AAudioStream_read", aaudio_stream_read as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setPerformanceMode", aaudio_stream_builder_set_performance_mode as *mut c_void);
    insert_sym(syms, "AAudioStream_getState", aaudio_stream_get_state as *mut c_void);
    insert_sym(syms, "AAudioStream_getFormat", aaudio_stream_get_format as *mut c_void);
    insert_sym(syms, "AAudioStreamBuilder_setUsage", aaudio_stream_builder_set_usage as *mut c_void);
    insert_sym(syms, "AAudioStream_requestStart", aaudio_stream_request_start as *mut c_void);
}

/// Rust replacement for the C++ `mc_register_aaudio_stub` (fake_audio.cpp):
/// registers the AAudio stub table with the Rust linker under the given
/// soname. Called from `crate::capi::setup_android_hooks` for both
/// `libaaudio.so` and `libaaudio.so.2`.
pub fn mc_register_aaudio_stub(name: *const c_char) {
    let name_str = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    let mut syms: HashMap<String, *mut c_void> = HashMap::new();
    init_hybris_hooks(&mut syms);
    linker::register_stub(&name_str, &syms);
}
