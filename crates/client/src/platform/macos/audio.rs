//! macOS placeholder for the cpal-backed FMOD `AudioDevice` JNI bridge.
//!
//! cpal itself is cross-platform, but its macOS backend (`coreaudio-sys`) can
//! only be built where the Apple SDK exists (real macOS / CI), so this target
//! keeps local darwin checks green. Same exported surface as the Linux module;
//! audio is silent until Phase 5 wires a real output stream.

#![allow(unused)]

use std::ffi::{c_char, c_void};

use libjnivm_sys::JNIEnv;

fn unimplemented(what: &str) {
    log::error!("[audio-macos] {} not implemented yet (docs/PORT_MACOS.md Phase 5)", what);
}

#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_init(
    _env: *mut JNIEnv, _thiz: *mut c_void, _numoutput: i32, _numinput: i32,
    _format: i32, _samplerate: i32, _buffsize: i32, _channels: i32,
) -> i32 {
    unimplemented("AudioDevice.init");
    0
}

#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_write(
    _env: *mut JNIEnv, _thiz: *mut c_void, _samples: *const i16, _count: i32,
) {
}

#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_write2(
    _env: *mut JNIEnv, _thiz: *mut c_void, _samples: *const i16, _count: i32,
) {
}

#[no_mangle]
pub unsafe extern "C" fn Java_org_fmod_AudioDevice_close(_env: *mut JNIEnv, _thiz: *mut c_void) {}

#[no_mangle]
pub extern "C" fn rust_audio_start(channels: i32, sample_rate: i32) -> i32 { -1 }

#[no_mangle]
pub unsafe extern "C" fn rust_audio_push_i16(samples: *const i16, count: i32) {}

#[no_mangle]
pub extern "C" fn rust_audio_stop() {}

/// JNI native registration — registers nothing on macOS yet.
pub fn register(env: *mut JNIEnv) {}
