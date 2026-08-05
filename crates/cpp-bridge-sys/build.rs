use std::collections::HashMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::{fs, io::Read};

// ===========================================================================
// Incremental compilation — hash-based per-file change detection + caching
// ===========================================================================

/// Content hash of a file (fast, non-cryptographic). Returns 0 on error
/// (treated as "always changed" so compilation proceeds safely).
fn file_hash(path: &Path) -> u64 {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        Hasher::write(&mut hasher, &buf[..n]);
    }
    Hasher::finish(&hasher)
}

/// Replicate the object-file path that `cc::Build::compile_intermediates`
/// produces internally (see `objects_from_files` in the `cc` crate).
fn cc_object_path(src: &Path) -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let basename = src.file_name().unwrap().to_string_lossy();
    let dirname = src.parent().unwrap().to_string_lossy();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Ok(ref root) = std::env::var("CARGO_MANIFEST_DIR") {
        let dirname = dirname.strip_prefix(root).unwrap_or(&dirname);
        Hasher::write(&mut hasher, dirname.as_bytes());
    } else {
        Hasher::write(&mut hasher, dirname.as_bytes());
    }
    if let Some(ext) = src.extension() {
        Hasher::write(&mut hasher, ext.to_string_lossy().as_bytes());
    }

    let hash = Hasher::finish(&mut hasher);
    out_dir
        .join(format!("{:016x}-{}", hash, basename))
        .with_extension("o")
}

/// Compile a group of source files into a static library, skipping
/// files whose content has not changed since the last build.
///
/// `name`       — static library name (e.g. `"linker"`)
/// `sources`    — absolute paths to every `.cpp`/`.cc`/`.c` file
/// `configure`  — closure that applies flags, includes, defines to a
///                fresh `cc::Build`
fn incr_compile<F>(name: &str, sources: &[PathBuf], configure: F)
where
    F: Fn(&mut cc::Build),
{
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let cache_dir = out_dir.join(format!("__incr_{}", name));
    let stamp_path = cache_dir.join("stamps");
    fs::create_dir_all(&cache_dir).unwrap();

    // 1. Emit rerun-if-changed so Cargo tracks these files properly.
    for src in sources {
        println!("cargo:rerun-if-changed={}", src.display());
    }

    // 2. Load previous stamp file (hash → path).
    let mut stamps: HashMap<String, u64> = {
        let mut map = HashMap::new();
        if let Ok(content) = fs::read_to_string(&stamp_path) {
            for line in content.lines() {
                if let Some((h, p)) = line.split_once(' ') {
                    if let Ok(hash) = u64::from_str_radix(h, 16) {
                        map.insert(p.to_string(), hash);
                    }
                }
            }
        }
        map
    };

    // 3. Determine which source files actually changed.
    let mut changed_srcs: Vec<&PathBuf> = Vec::new();
    for src in sources {
        let key = src.to_string_lossy().to_string();
        let hash = file_hash(src);
        let obj = cc_object_path(src);
        if stamps.get(&key) != Some(&hash) || !obj.exists() {
            changed_srcs.push(src);
        }
    }

    // 3b. Show progress.
    let total = sources.len();
    let n_changed = changed_srcs.len();
    let client_prefix = PathBuf::from("client");
    let project_root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent().unwrap().parent().unwrap()
        .to_string_lossy().to_string();
    if n_changed == 0 {
        println!("cargo:warning= [{name}] {total} files ─ up to date");
    } else {
        println!("cargo:warning= [{name}] {n_changed}/{total} files changed, compiling…");
        for src in &changed_srcs {
            let s = src.to_string_lossy();
            let rel = s.strip_prefix(&project_root).unwrap_or(&s);
            println!("cargo:warning=   compile   {rel}");
        }
    }

    // 4. Quick exit if nothing changed AND the archive already reflects the
    //    current source list (a removed source leaves a stale object in the
    //    archive otherwise, so compare member names against expected objects).
    let expected_objects: std::collections::HashSet<String> = sources
        .iter()
        .map(|s| cc_object_path(s).file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let archive = out_dir.join(format!("lib{}.a", name));
    let archive_current = if n_changed == 0 && archive.exists() {
        let members: std::collections::HashSet<String> = String::from_utf8_lossy(
            &std::process::Command::new("ar")
                .arg("t")
                .arg(&archive)
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default(),
        )
        .lines()
        .map(|l| l.to_string())
        .collect();
        members == expected_objects
    } else {
        false
    };
    if archive_current {
        println!("cargo:rustc-link-lib=static={}", name);
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        return;
    }

    // 5. Compile only the changed files.
    if n_changed > 0 {
        let mut build = cc::Build::new();
        configure(&mut build);
        build.cargo_metadata(false); // we emit our own link directives
        for src in &changed_srcs {
            build.file(src);
        }
        let new_objects = build.compile_intermediates();

        // Copy freshly compiled objects into the cache + update stamps.
        for (j, src) in changed_srcs.iter().enumerate() {
            if let Some(new_obj) = new_objects.get(j) {
                let cached = cache_dir.join(new_obj.file_name().unwrap());
                fs::copy(new_obj, &cached).unwrap();
            }
            stamps.insert(
                src.to_string_lossy().to_string(),
                file_hash(src),
            );
        }
    }

    // 6. Ensure every expected object exists (pull from cache for unchanged
    //    files that may have been cleaned by cargo).
    for src in sources {
        let obj = cc_object_path(src);
        if !obj.exists() {
            let cached = cache_dir.join(obj.file_name().unwrap());
            if !cached.exists() {
                panic!(
                    "missing object for {}. Do a full rebuild: cargo clean -p cpp-bridge-sys",
                    src.display()
                );
            }
            if let Some(parent) = obj.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::copy(&cached, &obj).unwrap();
        }
    }

    // 7. Build the final archive from ALL objects.
    let all_objects: Vec<PathBuf> = sources.iter().map(|s| cc_object_path(s)).collect();
    let _ = fs::remove_file(&archive);
    let mut ar = std::process::Command::new("ar");
    ar.arg("crsD").arg(&archive);
    for obj in &all_objects {
        ar.arg(obj);
    }
    let status = ar.status().expect("failed to run ar");
    assert!(status.success(), "ar failed for lib{}.a", name);

    // 8. Emit Cargo metadata so the linker can find the archive.
    println!("cargo:rustc-link-lib=static={}", name);
    println!("cargo:rustc-link-search=native={}", out_dir.display());

    // 9. Prune stamps for files that no longer exist in the source list.
    let current: std::collections::HashSet<String> = sources
        .iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect();
    stamps.retain(|k, _| current.contains(k));

    // 10. Persist stamps (sorted for deterministic output).
    let mut content = String::new();
    let mut entries: Vec<_> = stamps.iter().collect();
    entries.sort_by_key(|(k, _)| *k);
    for (path, hash) in &entries {
        content.push_str(&format!("{:016x} {}\n", hash, path));
    }
    fs::write(&stamp_path, &content).unwrap();
}

// ===========================================================================
// Main build script
// ===========================================================================

fn main() {
    let client_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../client");
    let local_inc = client_dir.join("include");

    // --- mcpelauncher-client-bridge: Phase 10 DELETED (capi.cpp ported to
    //     Rust capi.rs; linker::register_stub replaces rust_load_stub) ---

    // --- mcpelauncher-core: DELETED. The last two files
    //     (android_log_varargs.cpp, jnivm_mod_api.cpp) were ported to Rust
    //     (client android_log_hook.rs + mod_api.rs) once the client crate
    //     enabled nightly c_variadic. mcpelauncher-core is now fully Rust. ---
    // --- cll-telemetry (15 files): ported to Rust crate cll-telemetry
    //     (crates/cll-telemetry/), wired via client/src/cll_telemetry.rs
    //     (docs/PORT_CLL_TELEMETRY.md). C++ lib + nlohmann vendored header
    //     deleted. ---
    // --- simpleipc (14 files): ported to Rust crate simple-ipc (see docs/PORT_SIMPLEIPC.md) ---
    // --- daemon-client-utils: ported to Rust crate daemon-utils ---
    // --- msa-daemon-client: ported to Rust crate msa-daemon-client ---
    let gwin = client_dir.join("src/manifest_libs/gamewindow");
    let gwin_sources: Vec<PathBuf> = [
        "game_window_manager.cpp",
        "game_window_error_handler.cpp",
        "joystick_manager.cpp",
        "window_eglut.cpp",
        "window_manager_eglut.cpp",
        "window_with_linux_gamepad.cpp",
    ]
    .iter()
    .map(|f| gwin.join(f))
    .collect();
    incr_compile("mcpelauncher-gamewindow", &gwin_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("game-window"));
        b.include(local_inc.join("eglut"));
        b.include(&gwin);
    });

    // --- mcpelauncher-client-jni (JNI bridge + stubs + libjnivm) ---
    let mut client_sources: Vec<PathBuf> = Vec::new();

    // JNI class files (excluding ported ones)
    let excluded_jni: std::collections::HashSet<&str> = [
        "jbase64.cpp",
        "arrays.cpp",
        "asset_manager.cpp",
        "package_source.cpp",
        "securerandom.cpp",
        "signature.cpp",
        "accounts.cpp",
        "locale.cpp",
        "playfab.cpp",
        "fmod.cpp",
        "cert_manager.cpp",
        "webview.cpp",
        "shahasher.cpp",
        "http_stub.cpp",
        "ecdsa.cpp",
        "store.cpp",
        "uuid.cpp",
        // pulseaudio.cpp excluded: conflicts with sdl3audio AudioDevice when
        // HAVE_SDL3AUDIO is set. AAudio (fake_audio.cpp) is the primary path.
        "pulseaudio.cpp",
        "xbox_live.cpp",
    ]
    .into_iter()
    .collect();
    for entry in std::fs::read_dir(client_dir.join("src/jni")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().and_then(|s| s.to_str()) == Some("cpp")
            && !excluded_jni.contains(fname)
        {
            client_sources.push(path);
        }
    }

    // Stub files
    let stub_files = [
        "settings_stub.cpp",
        "xal_webview_factory_stub.cpp",
        "fake_egl_stub.cpp",
        "fake_inputqueue_stub.cpp",
        "text_input_handler_stub.cpp",
        "fake_assetmanager_stub.cpp",
        "fake_looper_stub.cpp",
        "xbox_live_helper_stub.cpp",
        "xbox_live_stub.cpp",
        "jni_bridge_stub.cpp",
        "jnivm_class_wrappers.cpp",
        "jbase64_stub.cpp",
        "logger_stub.cpp",
        "arrays_stub.cpp",
        "asset_manager_stub.cpp",
        "package_source_stub.cpp",
        "securerandom_stub.cpp",
        "signature_stub.cpp",
        "accounts_stub.cpp",
        "locale_stub.cpp",
        "playfab_stub.cpp",
        "fmod_stub.cpp",
        "webview_stub.cpp",
        "shahasher_stub.cpp",
        "file_picker_stub.cpp",
        "store_stub.cpp",
        "uuid_stub.cpp",
        "pulseaudio_stub.cpp",
        // AAudio shim — FMOD forces AAudio via setOutput hook, then dlopen's libaaudio.so
        "fake_audio.cpp",
        // Prevents loading the real libHttpClient.Android.so (broken under
        // the Rust linker — HCTraceInit@plt SIGSEGV at 0x49dd6).
        "http_client_stubs.cpp",
    ];
    for f in &stub_files {
        let path = client_dir.join("src").join(f);
        if path.exists() {
            client_sources.push(path);
        }
    }

    // main_stubs.cpp
    client_sources.push(client_dir.join("src/main_stubs.cpp"));

    // libjnivm C++ sources
    client_sources.push(local_inc.join("libjnivm/src/jnivm/env.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/vm.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/method.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/object.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/array.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/bytebuffer.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/field.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/findclass.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/jValuesfromValist.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/method.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/skipJNIType.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/string.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/stringUtil.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/codegen/class.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/codegen/field.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/codegen/method.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/codegen/namespace.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/codegen/parseJNIType.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/jnivm/internal/codegen/vm.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/fake-jni/fake-jni.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/fake-jni/jvm.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/fake-jni/method.cpp"));
    client_sources.push(local_inc.join("libjnivm/src/baron/jvm.cpp"));

    incr_compile("mcpelauncher-client-jni", &client_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(client_dir.join("src"));
        b.include(client_dir.join("src/manifest_headers"));
        b.define("EnableJNIVMGC", "1");
        b.include(local_inc.join("libjnivm"));
        b.include(local_inc.join("android-support-headers"));
        b.include(local_inc.join("game-window"));
        b.include(local_inc.join("logger"));
        b.include(local_inc.join("mcpelauncher-common"));
        b.include(local_inc.join("epoll-shim"));
        b.include(local_inc.join("properties-parser"));
        b.include(local_inc.join("mcpelauncher-errorwindow"));
        b.include(local_inc.join("linux-gamepad"));
        b.include(local_inc.join("file-picker"));
        b.include(local_inc.join("libc-shim"));
        b.include(client_dir.join("include"));
        b.include(local_inc.join("minecraft-imported-symbols"));
        b.include(local_inc.join("libjnivm/src"));
        b.define("HAVE_LOGGER", "1");
        b.define("JNI_DEBUG", "1");
        b.define("JNI_RETURN_NON_ZERO", "1");
        b.define("JNIVM_FAKE_JNI_SYNTAX", "0");
        b.define("JNIVM_FAKE_JNI_MINECRAFT_LINUX_COMPAT", "1");
    });
}
