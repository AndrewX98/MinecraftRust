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

    // 4. Quick exit if nothing changed and the archive already exists.
    let archive = out_dir.join(format!("lib{}.a", name));
    if n_changed == 0 && archive.exists() {
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
    let linker_src = client_dir.join("src/mcpelauncher-linker");
    let core_src = client_dir.join("src/mcpelauncher-core");

    // --- mcpelauncher-client-bridge (1 file) ---
    incr_compile("mcpelauncher-client-bridge", &[client_dir.join("src/capi.cpp")], |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("mcpelauncher-common"));
        b.include(local_inc.join("minecraft-imported-symbols"));
    });

    // --- linker (bionic, ~35 files) ---
    let linker_sources: Vec<PathBuf> = [
        "bionic/linker/rt.cpp",
        "bionic/linker/linker_gdb_support.cpp",
        "bionic/libc/bionic/bionic_call_ifunc_resolver.cpp",
        "bionic/linker/linker_dlwarning.cpp",
        "bionic/linker/dlfcn.cpp",
        "bionic/linker/linker_phdr.cpp",
        "bionic/linker/linker_soinfo.cpp",
        "bionic/linker/linker.cpp",
        "bionic/linker/linker_config.cpp",
        "bionic/linker/linker_utils.cpp",
        "bionic/linker/linker_debug.cpp",
        "bionic/linker/linker_block_allocator.cpp",
        "bionic/linker/linker_mapped_file_fragment.cpp",
        "bionic/linker/linker_relocate.cpp",
        "bionic/linker/linker_namespaces.cpp",
        "core/base/mapped_file.cpp",
        "bionic/linker/linker_globals.cpp",
        "bionic/linker/linker_main.cpp",
        "bionic/linker/linker_cfi.cpp",
        "bionic/linker/linker_sdk_versions.cpp",
        "bionic/linker/linker_logger.cpp",
        "core/base/file.cpp",
        "core/base/logging.cpp",
        "core/base/liblog_symbols.cpp",
        "bionic/libc/async_safe/async_safe_log.cpp",
        "core/base/stringprintf.cpp",
        "core/base/strings.cpp",
        "core/liblog/logger_write.cpp",
        "core/liblog/properties.cpp",
        "core/base/threads.cpp",
        "core/base/properties.cpp",
        "core/base/parsebool.cpp",
        "src/zip_archive_stream_entry.cc",
        "core/libziparchive/zip_archive.cc",
        "src/linker.cpp",
        "bionic/libdl/libdl.cpp",
    ]
    .iter()
    .map(|f| linker_src.join(f))
    .collect();
    incr_compile("linker", &linker_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(linker_src.join("include"));
        b.include(linker_src.join("bionic/libc"));
        b.include(linker_src.join("core/base/include"));
        b.include(linker_src.join("core/liblog/include"));
        b.include(linker_src.join("core/libcutils/include"));
        b.include(linker_src.join("public_include"));
        b.include(&linker_src);
        b.define("PATH_MAX", "256");
        b.define("_GNU_SOURCE", None);
        b.flag("-include");
        b.flag("compat.h");
    });

    // --- linker-c (2 C files, guarded by existence) ---
    {
        let mut linker_c_sources: Vec<PathBuf> = Vec::new();
        for f in &[
            "bionic/libc/upstream-openbsd/lib/libc/string/strlcpy.c",
            "bionic/libc/upstream-openbsd/lib/libc/string/strlcat.c",
        ] {
            let path = linker_src.join(f);
            if path.exists() {
                linker_c_sources.push(path);
            }
        }
        if !linker_c_sources.is_empty() {
            incr_compile("linker-c", &linker_c_sources, |b| {
                b.flag_if_supported("-w");
                b.include(linker_src.join("include"));
                b.include(linker_src.join("bionic/libc"));
                b.include(linker_src.join("bionic/libc/include"));
                b.include(&linker_src);
                b.define("PATH_MAX", "256");
                b.define("_GNU_SOURCE", None);
                b.flag("-include");
                b.flag("compat.h");
            });
        }
    }

    // --- mcpelauncher-core (9 files) ---
    let core_sources: Vec<PathBuf> = [
        "src/hook.cpp",
        "src/mod_loader.cpp",
        "src/hybris_utils.cpp",
        "src/hybris_android_log_hook.cpp",
        "src/crash_handler.cpp",
        "src/patch_utils.cpp",
        "src/minecraft_utils.cpp",
        "src/minecraft_version.cpp",
        "src/fmod_utils.cpp",
    ]
    .iter()
    .map(|f| core_src.join(f))
    .collect();
    incr_compile("mcpelauncher-core", &core_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(core_src.join("include"));
        b.include(linker_src.join("include"));
        b.include(linker_src.join("bionic/libc"));
        b.include(linker_src.join("core/base/include"));
        b.include(linker_src.join("core/liblog/include"));
        b.include(linker_src.join("core/libcutils/include"));
        b.include(linker_src.join("public_include"));
        b.include(&linker_src);
        b.include(local_inc.join("android-support-headers"));
        b.include(local_inc.join("logger"));
        b.include(local_inc.join("mcpelauncher-common"));
        b.include(local_inc.join("file-util"));
        b.include(local_inc.join("minecraft-imported-symbols"));
        b.include(local_inc.join("libjnivm"));
        b.include(local_inc.join("game-window"));
        b.include(local_inc.join("libc-shim"));
        b.define("PATH_MAX", "256");
        b.define("_GNU_SOURCE", None);
        b.flag("-include");
        b.flag("compat.h");
    });

    // --- mcpelauncher-manifest-libs (4 files) ---
    let manifest_libs_sources: Vec<PathBuf> = [
        "logger/log.cpp",
        "file-util/FileUtil.cpp",
        "file-util/EnvPathUtil.cpp",
        "mcpelauncher-common/path_helper.cpp",
    ]
    .iter()
    .map(|f| client_dir.join("src/manifest_libs").join(f))
    .collect();
    incr_compile("mcpelauncher-manifest-libs", &manifest_libs_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("logger"));
        b.include(local_inc.join("file-util"));
        b.include(local_inc.join("mcpelauncher-common"));
        b.include(local_inc.join("mcpelauncher-core"));
        b.define("HAVE_LOGGER", "1");
        let runtime_dir = client_dir.join("../../runtime");
        let dev_extra = format!(
            "\"{}:{}\"",
            runtime_dir.display(),
            runtime_dir.join("gamecontrollerdb").display(),
        );
        b.define("DEV_EXTRA_PATHS", dev_extra.as_str());
    });

    let nlohmann_json_include =
        local_inc.join("build/_deps/nlohmann_json_ext-src/single_include");

    // --- base64 (1 file) ---
    incr_compile(
        "mcpelauncher-base64",
        &[client_dir.join("src/manifest_libs/base64/base64.cpp")],
        |b| {
            b.cpp(true).std("c++17").flag_if_supported("-w");
            b.include(&local_inc);
        },
    );

    // --- simpleipc (14 files) ---
    let sim = client_dir.join("src/manifest_libs/simpleipc");
    let simpleipc_sources: Vec<PathBuf> = [
        "common/connection_internal.cpp",
        "common/encoding/encodings.cpp",
        "common/encoding/encoding_json.cpp",
        "common/encoding/encoding_json_cbor.cpp",
        "common/encoding/varint.cpp",
        "common/message/error_code.cpp",
        "server/rpc_handler.cpp",
        "server/default_rpc_handler.cpp",
        "client/service_client.cpp",
        "client/rpc_json_call.cpp",
        "unix/common/unix_connection.cpp",
        "unix/server/unix_service_impl.cpp",
        "unix/client/unix_service_client.cpp",
        "unix/epoll_io_handler.cpp",
    ]
    .iter()
    .map(|f| sim.join(f))
    .collect();
    incr_compile("mcpelauncher-simpleipc", &simpleipc_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("simple-ipc"));
        b.include(&sim);
        if nlohmann_json_include.exists() {
            b.include(&nlohmann_json_include);
        }
    });

    // --- daemon-client-utils (1 file) ---
    let daemon = client_dir.join("src/manifest_libs/daemon-utils");
    incr_compile(
        "mcpelauncher-daemon-client-utils",
        &[daemon.join("client/src/daemon_launcher.cpp")],
        |b| {
            b.cpp(true).std("c++17").flag_if_supported("-w");
            b.include(local_inc.join("daemon-utils"));
            b.include(local_inc.join("simple-ipc"));
            b.include(local_inc.join("logger"));
            b.include(local_inc.join("file-util"));
            b.include(local_inc.join("mcpelauncher-common"));
        },
    );

    // --- msa-daemon-client (2 files) ---
    let msa = client_dir.join("src/manifest_libs/msa-daemon-client");
    let msa_sources: Vec<PathBuf> =
        ["src/service_client.cpp", "src/token.cpp"].iter().map(|f| msa.join(f)).collect();
    incr_compile("mcpelauncher-msa-daemon-client", &msa_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("msa-daemon-client"));
        b.include(local_inc.join("simple-ipc"));
        b.include(local_inc.join("daemon-utils"));
        b.include(local_inc.join("logger"));
        b.include(&local_inc);
    });

    // --- cll-telemetry (15 files) ---
    let cll = client_dir.join("src/manifest_libs/cll-telemetry");
    let cll_sources: Vec<PathBuf> = [
        "src/event_manager.cpp",
        "src/configuration.cpp",
        "src/file_configuration_cache.cpp",
        "src/file_event_batch.cpp",
        "src/event_serializer.cpp",
        "src/event_serializer_extensions.cpp",
        "src/memory_event_batch.cpp",
        "src/multi_file_event_batch.cpp",
        "src/buffered_event_batch.cpp",
        "src/task_with_delay_thread.cpp",
        "src/event_uploader.cpp",
        "src/event_compressor.cpp",
        "src/http/curl_request.cpp",
        "src/http/curl_client.cpp",
        "src/http/mock_http_client.cpp",
    ]
    .iter()
    .map(|f| cll.join(f))
    .collect();
    incr_compile("mcpelauncher-cll-telemetry", &cll_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("cll-telemetry"));
        b.include(local_inc.join("logger"));
        if nlohmann_json_include.exists() {
            b.include(&nlohmann_json_include);
        }
    });

    // --- linux-gamepad (5 files) ---
    let gamepad = client_dir.join("src/manifest_libs/linux-gamepad");
    let gamepad_sources: Vec<PathBuf> = [
        "src/gamepad.cpp",
        "src/gamepad_mapping.cpp",
        "src/gamepad_manager.cpp",
        "src/linux_joystick_manager.cpp",
        "src/linux_joystick.cpp",
    ]
    .iter()
    .map(|f| gamepad.join(f))
    .collect();
    incr_compile("mcpelauncher-linux-gamepad", &gamepad_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("linux-gamepad"));
        b.include(gamepad.join("src"));
    });

    // --- gamewindow (7 files) ---
    let gwin = client_dir.join("src/manifest_libs/gamewindow");
    let gwin_sources: Vec<PathBuf> = [
        "game_window_manager.cpp",
        "game_window_error_handler.cpp",
        "joystick_manager.cpp",
        "window_eglut.cpp",
        "window_manager_eglut.cpp",
        "joystick_manager_linux_gamepad.cpp",
        "window_with_linux_gamepad.cpp",
    ]
    .iter()
    .map(|f| gwin.join(f))
    .collect();
    incr_compile("mcpelauncher-gamewindow", &gwin_sources, |b| {
        b.cpp(true).std("c++17").flag_if_supported("-w");
        b.include(local_inc.join("game-window"));
        b.include(local_inc.join("linux-gamepad"));
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
        "core_patches_stub.cpp",
        "cll_upload_auth_step_stub.cpp",
        "xal_webview_factory_stub.cpp",
        "window_callbacks_stub.cpp",
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
        "sdl3audio_stub.cpp",
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
        b.include(local_inc.join("msa-daemon-client"));
        b.include(local_inc.join("cll-telemetry"));
        b.include(local_inc.join("file-util"));
        b.include(local_inc.join("mcpelauncher-core"));
        b.include(local_inc.join("sdl3"));
        b.include(local_inc.join("daemon-utils"));
        b.include(local_inc.join("simple-ipc"));
        b.include(local_inc.join("epoll-shim"));
        b.include(local_inc.join("properties-parser"));
        b.include(local_inc.join("mcpelauncher-errorwindow"));
        b.include(local_inc.join("linux-gamepad"));
        b.include(local_inc.join("file-picker"));
        b.include(local_inc.join("libc-shim"));
        if nlohmann_json_include.exists() {
            b.include(&nlohmann_json_include);
        }
        b.include(client_dir.join("include"));
        b.include(local_inc.join("minecraft-imported-symbols"));
        b.include(local_inc.join("libjnivm/src"));
        b.define("HAVE_LOGGER", "1");
        b.define("JNI_DEBUG", "1");
        b.define("JNI_RETURN_NON_ZERO", "1");
        b.define("JNIVM_FAKE_JNI_SYNTAX", "0");
        b.define("JNIVM_FAKE_JNI_MINECRAFT_LINUX_COMPAT", "1");
        // FMOD AAudio path: enables AudioDevice registration + matches upstream
        // mcpelauncher-client. FakeAudio (libaaudio.so) is always linked.
        b.define("HAVE_SDL3AUDIO", "1");
    });
}
