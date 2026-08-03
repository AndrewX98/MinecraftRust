//! Rust port of `MinecraftUtils::loadMinecraftLib` — Phase 4 core.
//!
//! Orchestrates loading `libminecraftpe.so` through the Rust linker with hook
//! relocations, replacing the C++ `MinecraftUtils::loadMinecraftLib` entry.
//! All `linker_rust_*` symbols are the public Rust extern "C" fns exported by
//! the `linker` crate; the C++ bionic linker is no longer consulted on this path.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};

use linker::McpelauncherHook;

// C++ helpers that keep type-dependent state (preinitHooks, HookManager) in C++.
extern "C" {
    fn mc_find_data_file(path: *const c_char) -> *const c_char;
    fn mc_get_preinit_hooks(
        names: *mut *const c_char,
        vals: *mut *mut c_void,
        max: usize,
    ) -> usize;
    fn mc_finalize_load(
        handle: *mut c_void,
        names: *const *const c_char,
        vals: *const *mut c_void,
        count: usize,
    );
    // core_patches_stub.cpp thunks
    fn core_patches_show_mouse_pointer();
    fn core_patches_hide_mouse_pointer();
    fn core_patches_set_fullscreen(t: *mut c_void, fs: bool);
    fn core_patches_install(handle: *mut c_void);
    // fake_looper_stub.cpp thunk
    fn fake_looper_on_game_activity_close(native: *mut c_void);
}

fn read_env_flag(name: &str, def: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => v == "true" || v == "1" || v == "on",
        Err(_) => def,
    }
}

/// Mirrors `PathHelper::getAbiDir` (now `path_helper::get_abi_dir`).
fn abi_dir() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64-v8a"
    } else if cfg!(target_arch = "arm") {
        "armeabi-v7a"
    } else {
        "unsupported"
    }
}

extern "C" fn telemetry_start() {
    log::error!("MinecraftUtils: TelemetrySystemBase::start");
}

/// Port of `MinecraftUtils::loadMinecraftLib`. Returns a Rust linker handle
/// cast to a pointer, or null on failure. Rust-only — no C++ fallback.
pub unsafe fn load_minecraft() -> *mut c_void {
    // Keep dynamically-built hook names alive for the duration of the call.
    let mut owned: Vec<CString> = Vec::new();
    let mut hook_name = |s: &str| -> *const c_char {
        let c = CString::new(s).unwrap();
        owned.push(c);
        owned.last().unwrap().as_ptr()
    };

    // 1. Load libc++_shared.so via Rust linker (INIT_ARRAY, no IFUNC/TLS).
    let libcxx_rust = linker::linker_rust_dlopen_libcxx(c"libc++_shared.so".as_ptr());
    if libcxx_rust != 0 {
        log::info!("MinecraftUtils: Loaded libc++_shared (rust_handle={})", libcxx_rust);
    } else {
        log::error!("MinecraftUtils: Failed to load libc++_shared via Rust linker");
    }

    // 2. libstdc++ stub: relocate the __cxa_* runtime symbols from libc++ so
    //    libfmod standalone loads resolve against the C++ runtime.
    let libstdcxx = linker::dlopen("libstdc++.so", 0);
    if libcxx_rust != 0 {
        let mut cxa: HashMap<String, *mut c_void> = HashMap::new();
        for s in ["__cxa_pure_virtual", "__cxa_guard_acquire", "__cxa_guard_release"] {
            if let Some(addr) = linker::dlsym(libcxx_rust, s) {
                cxa.insert(s.to_string(), addr);
            }
        }
        if cxa.len() == 3 {
            if let Some(libstdcxx_h) = libstdcxx {
                linker::add_symbols(libstdcxx_h, &cxa);
            }
        }
    }

    // 3. Assemble the hook list (order mirrors the C++ orchestrator).
    let mut hooks: Vec<McpelauncherHook> = Vec::new();

    // SwappyGL hooks.
    let mut swappy: Vec<McpelauncherHook> = (0..15)
        .map(|_| McpelauncherHook {
            name: std::ptr::null(),
            value: std::ptr::null_mut(),
        })
        .collect();
    crate::rust_bridge::fake_swappygl_fill_hooks(
        swappy.as_mut_ptr() as *mut crate::rust_bridge::McpelauncherHook,
        15,
    );
    hooks.append(&mut swappy);

    // Mod-registered preinit hooks.
    const MAX_PREINIT: usize = 64;
    let mut preinit_names: [*const c_char; MAX_PREINIT] = [std::ptr::null(); MAX_PREINIT];
    let mut preinit_vals: [*mut c_void; MAX_PREINIT] = [std::ptr::null_mut(); MAX_PREINIT];
    let n_preinit =
        mc_get_preinit_hooks(preinit_names.as_mut_ptr(), preinit_vals.as_mut_ptr(), MAX_PREINIT);
    for i in 0..n_preinit {
        hooks.push(McpelauncherHook {
            name: preinit_names[i],
            value: preinit_vals[i],
        });
    }

    // AppPlatform mouse pointer / fullscreen / close callbacks.
    hooks.push(McpelauncherHook {
        name: hook_name("_ZN11AppPlatform16showMousePointerEv"),
        value: core_patches_show_mouse_pointer as *mut c_void,
    });
    hooks.push(McpelauncherHook {
        name: hook_name("_ZN11AppPlatform16hideMousePointerEv"),
        value: core_patches_hide_mouse_pointer as *mut c_void,
    });
    hooks.push(McpelauncherHook {
        name: hook_name("_ZN11AppPlatform17setFullscreenModeE14FullscreenMode"),
        value: core_patches_set_fullscreen as *mut c_void,
    });
    hooks.push(McpelauncherHook {
        name: hook_name("GameActivity_finish"),
        value: fake_looper_on_game_activity_close as *mut c_void,
    });

    // 4. FMOD: load via Rust linker, resolve System functions, hook init/setOutput.
    let mut fmod: usize = 0;
    if read_env_flag("MCPELAUNCHER_PATCH_FMOD", true) {
        fmod = linker::linker_rust_dlopen_fmod(c"libfmod.so".as_ptr());
        if fmod != 0 {
            log::info!("MinecraftUtils: Loaded libfmod (rust_handle={})", fmod);
        } else {
            log::error!("MinecraftUtils: Failed to load libfmod via Rust linker");
        }
    }
    if fmod != 0 {
        if linker::get_library_base(fmod) != 0 {
            if crate::fmod_utils::setup(fmod) {
                hooks.push(McpelauncherHook {
                    name: hook_name("_ZN4FMOD6System4initEijPv"),
                    value: crate::fmod_utils::init_hook as *mut c_void,
                });
                hooks.push(McpelauncherHook {
                    name: hook_name("_ZN4FMOD6System9setOutputE15FMOD_OUTPUTTYPE"),
                    value: crate::fmod_utils::set_output_hook as *mut c_void,
                });
            }
        } else {
            linker::dlclose(fmod);
        }
    }

    // 5. libc (stub) + libpairipcore (real ELF, Android Integrity Protection).
    let libc = linker::dlopen("libc.so", 0);
    let pairipcore_rust = linker::linker_rust_dlopen_pairipcore(c"libpairipcore.so".as_ptr());
    if pairipcore_rust != 0 {
        log::info!("MinecraftUtils: Loaded libpairipcore (rust_handle={})", pairipcore_rust);
    } else {
        log::error!("MinecraftUtils: Failed to load libpairipcore: Rust linker returned 0");
    }

    // 6. webrtc ifaddrs: forward to the libc shim.
    if let Some(libc_h) = libc {
        if let Some(addr) = linker::dlsym(libc_h, "getifaddrs") {
            hooks.push(McpelauncherHook {
                name: hook_name("_ZN3rtc10getifaddrsEPP7ifaddrs"),
                value: addr,
            });
        }
        if let Some(addr) = linker::dlsym(libc_h, "freeifaddrs") {
            hooks.push(McpelauncherHook {
                name: hook_name("_ZN3rtc11freeifaddrsEP7ifaddrs"),
                value: addr,
            });
        }
    }

    // 7. Telemetry: stub out, or load libsqliteX.so when pairipcore is present.
    if read_env_flag("MCPELAUNCHER_DISABLE_TELEMETRY", false) {
        hooks.push(McpelauncherHook {
            name: hook_name("_ZN9Microsoft12Applications6Events19TelemetrySystemBase5startEv"),
            value: telemetry_start as *mut c_void,
        });
    } else if pairipcore_rust != 0 {
        let sqlite_rel = format!("lib/{}/libsqliteX.so", abi_dir());
        let path_c = CString::new(sqlite_rel).unwrap();
        let sqlite3_path = mc_find_data_file(path_c.as_ptr());
        if !sqlite3_path.is_null() {
            let resolved = CStr::from_ptr(sqlite3_path).to_string_lossy().into_owned();
            if let Some(pos) = resolved.rfind('/') {
                let dir = &resolved[..pos];
                let dir_c = CString::new(dir).unwrap();
                linker::linker_rust_add_search_path(dir_c.as_ptr());
            }
        }
        let sqlite3 = linker::linker_rust_dlopen_sqlite(c"libsqliteX.so".as_ptr());
        if sqlite3 != 0 {
            log::info!("MinecraftUtils: Rust linker loaded libsqliteX.so (handle={})", sqlite3);
        } else {
            log::error!("MinecraftUtils: Rust linker failed to load libsqliteX.so");
        }
    }

    // 8. Load the game through the Rust linker (no constructors run yet).
    let game_path = CString::new("libminecraftpe.so").unwrap();
    let names: Vec<*const c_char> = hooks.iter().map(|h| h.name).collect();
    let vals: Vec<*mut c_void> = hooks.iter().map(|h| h.value).collect();
    let rust_handle = linker::linker_rust_dlopen_ext(
        game_path.as_ptr(),
        0,
        names.as_ptr(),
        vals.as_ptr(),
        hooks.len(),
    );

    if rust_handle != 0 {
        let handle_ptr = rust_handle as *mut c_void;
        log::info!(
            "MinecraftUtils: Rust linker loaded libminecraftpe.so (handle={})",
            rust_handle
        );
        // dlsym each hook for diagnostics, fire preinit callbacks, register the
        // library with HookManager (C++ keeps that state).
        mc_finalize_load(handle_ptr, names.as_ptr(), vals.as_ptr(), hooks.len());
        core_patches_install(handle_ptr);
        handle_ptr
    } else {
        log::error!(
            "MinecraftUtils: Rust linker failed to load libminecraftpe.so (Rust-only, no C++ fallback)"
        );
        std::ptr::null_mut()
    }
}
