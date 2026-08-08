//! Rust port of `FmodUtils` (mcpelauncher-core fmod_utils.cpp).
//! Holds the FMOD System function pointers and provides the `init` hook that
//! the game's `FMOD::System::init` is redirected to.

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::OnceLock;

type FmodInit = unsafe extern "C" fn(*mut c_void, i32, u32, *mut c_void) -> i32;
type FmodSetSoftwareFormat = unsafe extern "C" fn(*mut c_void, i32, i32, i32) -> i32;
type FmodSetDspBufferSize = unsafe extern "C" fn(*mut c_void, u32, i32) -> i32;
type FmodGetDspBufferSize = unsafe extern "C" fn(*mut c_void, *mut u32, *mut i32) -> i32;
type FmodSetOutput = unsafe extern "C" fn(*mut c_void, i32) -> i32;

struct FmodPointers {
    system_init: FmodInit,
    set_software_format: FmodSetSoftwareFormat,
    set_dsp_buffer_size: FmodSetDspBufferSize,
    get_dsp_buffer_size: FmodGetDspBufferSize,
    set_output: FmodSetOutput,
}

static FMOD_PTRS: OnceLock<FmodPointers> = OnceLock::new();

static SAMPLE_RATE: AtomicI32 = AtomicI32::new(48000);

fn read_env_int(name: &str, def: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(def)
}

/// dlsym the four FMOD System functions off `handle` (Rust linker handle).
/// Returns true only when all four are present.
pub unsafe fn setup(handle: usize) -> bool {
    let init = linker::dlsym(handle, "_ZN4FMOD6System4initEijPv");
    let set_software_format =
        linker::dlsym(handle, "_ZN4FMOD6System17setSoftwareFormatEi16FMOD_SPEAKERMODEi");
    let set_dsp_buffer_size =
        linker::dlsym(handle, "_ZN4FMOD6System16setDSPBufferSizeEji");
    let get_dsp_buffer_size =
        linker::dlsym(handle, "_ZN4FMOD6System16getDSPBufferSizeEPjPi");
    let set_output =
        linker::dlsym(handle, "_ZN4FMOD6System9setOutputE15FMOD_OUTPUTTYPE");

    let (init, set_software_format, set_dsp_buffer_size, get_dsp_buffer_size, set_output) = match (
        init,
        set_software_format,
        set_dsp_buffer_size,
        get_dsp_buffer_size,
        set_output,
    ) {
        (Some(init), Some(sf), Some(dsb), Some(gdsb), Some(so)) => (init, sf, dsb, gdsb, so),
        _ => {
            log::warn!(
                "fmod_utils: could not resolve all FMOD System hooks (init={:?}, sf={:?}, dsb={:?}, gdsb={:?}, setOutput={:?})",
                init, set_software_format, set_dsp_buffer_size, get_dsp_buffer_size, set_output
            );
            return false;
        }
    };

    let _ = FMOD_PTRS.set(FmodPointers {
        system_init: std::mem::transmute(init),
        set_software_format: std::mem::transmute(set_software_format),
        set_dsp_buffer_size: std::mem::transmute(set_dsp_buffer_size),
        get_dsp_buffer_size: std::mem::transmute(get_dsp_buffer_size),
        set_output: std::mem::transmute(set_output),
    });
    true
}

/// Hook replacing `FMOD::System::init`: invoke the environment overrides before
/// forwarding to the real implementation.
pub extern "C" fn init_hook(
    system: *mut c_void,
    maxchannels: i32,
    flags: u32,
    extradriverdata: *mut c_void,
) -> i32 {
    log::info!(
        "FMOD init_hook: called (maxchannels={}, flags=0x{:x})",
        maxchannels,
        flags
    );
    let Some(f) = FMOD_PTRS.get() else {
        return -1;
    };
    let mut default_buffer_len: u32 = 0;
    let mut default_num_buffers: i32 = 0;
    let result = unsafe {
        (f.get_dsp_buffer_size)(system, &mut default_buffer_len, &mut default_num_buffers);
        (f.set_dsp_buffer_size)(
            system,
            read_env_int("FMOD_DSP_BUFFER_LENGTH", default_buffer_len as i32) as u32,
            read_env_int("FMOD_DSP_NUM_BUFFERS", default_num_buffers),
        );
        (f.set_software_format)(
            system,
            SAMPLE_RATE.load(Ordering::Relaxed),
            read_env_int("FMOD_SPEAKER_MODE", 0),
            0,
        );
        (f.system_init)(system, maxchannels, flags, extradriverdata)
    };
    log::info!("FMOD init_hook returned {:?}", result);
    result
}

/// Hook replacing `FMOD::System::setOutput`. Forwards to the real call so FMOD
/// actually configures its output backend (AAudio), which then dlopens
/// `libaaudio.so` through our dispatch and hits the Rust `fake_audio` stub.
pub extern "C" fn set_output_hook(this: *mut c_void, outputtype: i32) -> i32 {
    log::info!(
        "FMOD set_output_hook: forwarding outputtype={}",
        outputtype
    );
    let Some(f) = FMOD_PTRS.get() else {
        return -1;
    };
    let result = unsafe { (f.set_output)(this, outputtype) };
    log::info!("FMOD set_output_hook returned {}", result);
    result
}

/// Set the FMOD output sample rate (called from C++ `FakeAudio::updateDefaults`).
#[no_mangle]
pub extern "C" fn mc_fmod_set_sample_rate(rate: i32) {
    SAMPLE_RATE.store(rate, Ordering::Relaxed);
}
