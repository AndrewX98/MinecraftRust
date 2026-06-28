use std::collections::VecDeque;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;

use libjnivm_sys::*;

// cpal::Stream is not Send, so we can't store it in a global Mutex directly.
// Instead, we store the audio data buffer in a global and create the stream
// on a dedicated audio thread.

struct AudioBuffer {
    samples: VecDeque<i16>,
    max_samples: usize,
}

impl AudioBuffer {
    fn new() -> Self {
        AudioBuffer {
            samples: VecDeque::new(),
            max_samples: 48000 * 2, // 1 second at 48kHz stereo
        }
    }

    fn push_samples(&mut self, data: &[i16]) {
        self.samples.extend(data.iter());
        // Limit buffer size
        if self.samples.len() > self.max_samples {
            let excess = self.samples.len() - self.max_samples;
            self.samples.drain(..excess);
        }
    }

    fn pop_samples(&mut self, buf: &mut [f32]) {
        for sample in buf.iter_mut() {
            if let Some(s) = self.samples.pop_front() {
                *sample = s as f32 / i16::MAX as f32;
            } else {
                *sample = 0.0;
            }
        }
    }
}

static AUDIO_BUFFER: OnceLock<Mutex<AudioBuffer>> = OnceLock::new();

fn audio_buffer() -> &'static Mutex<AudioBuffer> {
    AUDIO_BUFFER.get_or_init(|| Mutex::new(AudioBuffer::new()))
}

// Helper to get JNI vtable
fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

// Helper to read byte array
fn get_byte_array_elements(env: *mut JNIEnv, arr: jbyteArray) -> Option<*const u8> {
    let iface = get_iface(env);
    if iface.is_null() {
        return None;
    }
    let get_bytes = unsafe { (*iface).GetByteArrayElements }?;
    let ptr = unsafe { get_bytes(env, arr, std::ptr::null_mut()) };
    if ptr.is_null() {
        None
    } else {
        Some(ptr as *const u8)
    }
}

// Helper to read short array
fn get_short_array_elements(env: *mut JNIEnv, arr: jshortArray) -> Option<*const i16> {
    let iface = get_iface(env);
    if iface.is_null() {
        return None;
    }
    let get_elements = unsafe { (*iface).GetShortArrayElements }?;
    let ptr = unsafe { get_elements(env, arr, std::ptr::null_mut()) };
    if ptr.is_null() {
        None
    } else {
        Some(ptr as *const i16)
    }
}

// Helper to release byte array
fn release_byte_array_elements(env: *mut JNIEnv, arr: jbyteArray, ptr: *const u8) {
    let iface = get_iface(env);
    if iface.is_null() {
        return;
    }
    if let Some(release) = unsafe { (*iface).ReleaseByteArrayElements } {
        unsafe { release(env, arr, ptr as *mut i8, 0) };
    }
}

// Helper to release short array
fn release_short_array_elements(env: *mut JNIEnv, arr: jshortArray, ptr: *const i16) {
    let iface = get_iface(env);
    if iface.is_null() {
        return;
    }
    if let Some(release) = unsafe { (*iface).ReleaseShortArrayElements } {
        unsafe { release(env, arr, ptr as *mut i16, 0) };
    }
}

// Helper to get array length
fn get_array_length(env: *mut JNIEnv, arr: jarray) -> Option<jint> {
    let iface = get_iface(env);
    if iface.is_null() {
        return None;
    }
    let get_len = unsafe { (*iface).GetArrayLength }?;
    Some(unsafe { get_len(env, arr) })
}

// Track if audio thread is running
static AUDIO_THREAD_RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();

fn audio_thread_running() -> &'static Mutex<bool> {
    AUDIO_THREAD_RUNNING.get_or_init(|| Mutex::new(false))
}

fn start_audio_thread(channels: u16, sample_rate: u32) {
    let mut running = match audio_thread_running().lock() {
        Ok(r) => r,
        Err(_) => return,
    };

    if *running {
        return;
    }

    std::thread::spawn(move || {
        use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                log::error!("No audio output device found");
                return;
            }
        };

        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = match device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if let Ok(mut buf) = audio_buffer().lock() {
                    buf.pop_samples(data);
                }
            },
            |err| {
                log::error!("Audio stream error: {}", err);
            },
            None,
        ) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create audio stream: {}", e);
                return;
            }
        };

        if let Err(e) = stream.play() {
            log::error!("Failed to start audio stream: {}", e);
            return;
        }

        log::info!("Audio thread started");

        // Keep the stream alive
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let running = match audio_thread_running().lock() {
                Ok(r) => *r,
                Err(_) => true,
            };
            if !running {
                break;
            }
        }

        log::info!("Audio thread stopping");
        drop(stream);
    });

    *running = true;
}

fn stop_audio_thread() {
    if let Ok(mut running) = audio_thread_running().lock() {
        *running = false;
    }
}

// org/fmod/AudioDevice.init(IIII)Z
#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_init(
    _env: *mut JNIEnv,
    _self: jobject,
    channels: jint,
    samplerate: jint,
    _c: jint,
    _d: jint,
) -> jboolean {
    // Clear any existing buffer
    if let Ok(mut buf) = audio_buffer().lock() {
        buf.samples.clear();
    }

    start_audio_thread(channels as u16, samplerate as u32);
    1
}

// org/fmod/AudioDevice.write([BI)V
#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_write(
    env: *mut JNIEnv,
    _self: jobject,
    data: jbyteArray,
    length: jint,
) {
    // Get byte array elements
    let ptr = match get_byte_array_elements(env, data) {
        Some(p) => p,
        None => return,
    };

    let len = match get_array_length(env, data as jarray) {
        Some(l) => l as usize,
        None => {
            release_byte_array_elements(env, data, ptr);
            return;
        }
    };

    // Convert bytes to i16 samples (S16NE = signed 16-bit native endian)
    let byte_len = length as usize;
    let sample_count = byte_len / 2;
    let mut samples = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let offset = i * 2;
        if offset + 1 < len && offset + 1 < byte_len {
            let bytes = [*(ptr.add(offset)), *(ptr.add(offset + 1))];
            let sample = i16::from_ne_bytes(bytes);
            samples.push(sample);
        }
    }

    release_byte_array_elements(env, data, ptr);

    // Add to buffer
    if let Ok(mut buf) = audio_buffer().lock() {
        buf.push_samples(&samples);
    }
}

// org/fmod/AudioDevice.write2([SI)V
#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_write2(
    env: *mut JNIEnv,
    _self: jobject,
    data: jshortArray,
    length: jint,
) {
    // Get short array elements
    let ptr = match get_short_array_elements(env, data) {
        Some(p) => p,
        None => return,
    };

    let sample_count = length as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let sample = *ptr.add(i);
        samples.push(sample);
    }

    release_short_array_elements(env, data, ptr);

    // Add to buffer
    if let Ok(mut buf) = audio_buffer().lock() {
        buf.push_samples(&samples);
    }
}

// org/fmod/AudioDevice.close()V
#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_close(
    _env: *mut JNIEnv,
    _self: jobject,
) {
    stop_audio_thread();
    // Clear buffer
    if let Ok(mut buf) = audio_buffer().lock() {
        buf.samples.clear();
    }
}

// Register native methods with libjnivm-sys
pub fn register(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"init\0".as_ptr() as *const c_char,
            signature: b"(IIII)Z\0".as_ptr() as *const c_char,
            fnPtr: Java_org_fmod_AudioDevice_init as *mut c_void,
        },
        JNINativeMethod {
            name: b"write\0".as_ptr() as *const c_char,
            signature: b"([BI)V\0".as_ptr() as *const c_char,
            fnPtr: Java_org_fmod_AudioDevice_write as *mut c_void,
        },
        JNINativeMethod {
            name: b"write2\0".as_ptr() as *const c_char,
            signature: b"([SI)V\0".as_ptr() as *const c_char,
            fnPtr: Java_org_fmod_AudioDevice_write2 as *mut c_void,
        },
        JNINativeMethod {
            name: b"close\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: Java_org_fmod_AudioDevice_close as *mut c_void,
        },
    ];

    let cls = unsafe {
        jnivm_find_class(
            env,
            b"org/fmod/AudioDevice\0".as_ptr() as *const c_char,
        )
    };
    if cls.is_null() {
        log::warn!("Could not find org/fmod/AudioDevice class");
        return;
    }
    unsafe {
        jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32);
    }
    log::info!("Registered AudioDevice native methods");
}
