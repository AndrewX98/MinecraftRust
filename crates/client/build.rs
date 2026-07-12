use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let local_inc = manifest_dir.join("include");
    let linker_src = manifest_dir.join("src/mcpelauncher-linker");
    let core_src = manifest_dir.join("src/mcpelauncher-core");

    // --- Compile capi.cpp bridge (simple file without client C++ deps) ---
    let mut bridge = cc::Build::new();
    bridge.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");

    // Add include paths for mcpelauncher headers (needed by capi.cpp forward declarations)
    bridge.include(local_inc.join("mcpelauncher-common"));
    bridge.include(local_inc.join("minecraft-imported-symbols"));
    // NOTE: NOT adding mcpelauncher-linker/bionic includes — they conflict with GCC 16.

    // Compile capi.cpp
    bridge.file(manifest_dir.join("src").join("capi.cpp"));
    bridge.compile("mcpelauncher-client-bridge");

    // --- Compile mcpelauncher-linker sources (replaces cmake-built liblinker.a) ---
    let mut linker = cc::Build::new();
    linker.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    linker.include(linker_src.join("include"));
    linker.include(linker_src.join("bionic/libc"));
    linker.include(linker_src.join("core/base/include"));
    linker.include(linker_src.join("core/liblog/include"));
    linker.include(linker_src.join("core/libcutils/include"));
    linker.include(linker_src.join("public_include"));
    linker.include(&linker_src);
    linker.define("PATH_MAX", "256");
    linker.define("_GNU_SOURCE", None);
    linker.flag("-include");
    linker.flag("compat.h");
    let linker_files = [
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
    ];
    for f in &linker_files {
        linker.file(linker_src.join(f));
    }
    linker.compile("linker");
    // C sources need a separate cc::Build (not .cpp(true))
    let mut linker_c = cc::Build::new();
    linker_c.flag_if_supported("-w");
    linker_c.include(linker_src.join("include"));
    linker_c.include(linker_src.join("bionic/libc"));
    linker_c.include(linker_src.join("bionic/libc/include"));
    linker_c.include(&linker_src);
    linker_c.define("PATH_MAX", "256");
    linker_c.define("_GNU_SOURCE", None);
    linker_c.flag("-include");
    linker_c.flag("compat.h");
    for f in &[
        "bionic/libc/upstream-openbsd/lib/libc/string/strlcpy.c",
        "bionic/libc/upstream-openbsd/lib/libc/string/strlcat.c",
    ] {
        let path = linker_src.join(f);
        if path.exists() {
            linker_c.file(path);
        }
    }
    linker_c.compile("linker-c");

    // --- Compile mcpelauncher-core sources (replaces cmake-built libmcpelauncher-core.a) ---
    let mut core = cc::Build::new();
    core.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    core.include(core_src.join("include"));
    core.include(linker_src.join("include"));
    core.include(linker_src.join("bionic/libc"));
    core.include(linker_src.join("core/base/include"));
    core.include(linker_src.join("core/liblog/include"));
    core.include(linker_src.join("core/libcutils/include"));
    core.include(linker_src.join("public_include"));
    core.include(&linker_src);
    core.include(local_inc.join("android-support-headers"));
    core.include(local_inc.join("logger"));
    core.include(local_inc.join("mcpelauncher-common"));
    core.include(local_inc.join("file-util"));
    core.include(local_inc.join("minecraft-imported-symbols"));
    core.include(local_inc.join("libjnivm"));
    core.include(local_inc.join("game-window"));
    core.include(local_inc.join("libc-shim"));
    core.define("PATH_MAX", "256");
    core.define("_GNU_SOURCE", None);
    core.flag("-include");
    core.flag("compat.h");
    for f in &[
        "src/hook.cpp",
        "src/mod_loader.cpp",
        "src/hybris_utils.cpp",
        "src/hybris_android_log_hook.cpp",
        "src/crash_handler.cpp",
        "src/patch_utils.cpp",
        "src/minecraft_utils.cpp",
        "src/minecraft_version.cpp",
        "src/fmod_utils.cpp",
    ] {
        core.file(core_src.join(f));
    }
    core.compile("mcpelauncher-core");

    // --- Compile manifest library sources (logger, file-util, mcpelauncher-common) ---
    let mut manifest_libs = cc::Build::new();
    manifest_libs.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    manifest_libs.include(local_inc.join("logger"));
    manifest_libs.include(local_inc.join("file-util"));
    manifest_libs.include(local_inc.join("mcpelauncher-common"));
    manifest_libs.include(local_inc.join("mcpelauncher-core"));
    // file-util may optionally use logger for debug messages
    manifest_libs.define("HAVE_LOGGER", "1");
    // DEV_EXTRA_PATHS for runtime data files (libsqliteX.so, gamecontrollerdb)
    // path_helper::findDataFile("lib/x86_64/libsqliteX.so") appends the relative path
    // to each search dir, so the base dir must be the parent of lib/x86_64/.
    let runtime_dir = manifest_dir.join("../../runtime");
    let dev_extra = format!("\"{}:{}\"",
        runtime_dir.display(),
        runtime_dir.join("gamecontrollerdb").display(),
    );
    manifest_libs.define("DEV_EXTRA_PATHS", dev_extra.as_str());
    for f in &[
        "logger/log.cpp",
        "file-util/FileUtil.cpp",
        "file-util/EnvPathUtil.cpp",
        "mcpelauncher-common/path_helper.cpp",
    ] {
        manifest_libs.file(manifest_dir.join("src/manifest_libs").join(f));
    }
    manifest_libs.compile("mcpelauncher-manifest-libs");

    // Use cmake-built nlohmann_json headers (must match what static archives were compiled with)
    let nlohmann_json_include = local_inc.join("build/_deps/nlohmann_json_ext-src/single_include");

    // --- Compile base64 (standalone, used by msa-daemon-client) ---
    let mut base64 = cc::Build::new();
    base64.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    base64.include(&local_inc);  // for base64.h
    base64.file(manifest_dir.join("src/manifest_libs/base64/base64.cpp"));
    base64.compile("mcpelauncher-base64");

    // --- Compile simpleipc (IPC library, depends on nlohmann_json) ---
    let sim = manifest_dir.join("src/manifest_libs/simpleipc");
    let mut simpleipc = cc::Build::new();
    simpleipc.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    simpleipc.include(local_inc.join("simple-ipc"));
    simpleipc.include(&sim);
    if nlohmann_json_include.exists() {
        simpleipc.include(&nlohmann_json_include);
    }
    for f in &[
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
    ] {
        simpleipc.file(sim.join(f));
    }
    simpleipc.compile("mcpelauncher-simpleipc");

    // --- Compile daemon-client-utils (daemon launcher, depends on simpleipc, logger, file-util) ---
    let daemon = manifest_dir.join("src/manifest_libs/daemon-utils");
    let mut daemon_utils = cc::Build::new();
    daemon_utils.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    daemon_utils.include(local_inc.join("daemon-utils"));
    daemon_utils.include(local_inc.join("simple-ipc"));
    daemon_utils.include(local_inc.join("logger"));
    daemon_utils.include(local_inc.join("file-util"));
    daemon_utils.include(local_inc.join("mcpelauncher-common"));
    daemon_utils.file(daemon.join("client/src/daemon_launcher.cpp"));
    daemon_utils.compile("mcpelauncher-daemon-client-utils");

    // --- Compile msa-daemon-client (MSA auth, depends on simpleipc, logger, base64, daemon-client-utils) ---
    let msa = manifest_dir.join("src/manifest_libs/msa-daemon-client");
    let mut msa_client = cc::Build::new();
    msa_client.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    msa_client.include(local_inc.join("msa-daemon-client"));
    msa_client.include(local_inc.join("simple-ipc"));
    msa_client.include(local_inc.join("daemon-utils"));
    msa_client.include(local_inc.join("logger"));
    msa_client.include(&local_inc);  // for base64.h
    for f in &["src/service_client.cpp", "src/token.cpp"] {
        msa_client.file(msa.join(f));
    }
    msa_client.compile("mcpelauncher-msa-daemon-client");

    // --- Compile cll-telemetry (telemetry/eventing, depends on logger, nlohmann_json, curl) ---
    let cll = manifest_dir.join("src/manifest_libs/cll-telemetry");
    let mut cll_telemetry = cc::Build::new();
    cll_telemetry.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    cll_telemetry.include(local_inc.join("cll-telemetry"));
    cll_telemetry.include(local_inc.join("logger"));
    if nlohmann_json_include.exists() {
        cll_telemetry.include(&nlohmann_json_include);
    }
    for f in &[
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
    ] {
        cll_telemetry.file(cll.join(f));
    }
    cll_telemetry.compile("mcpelauncher-cll-telemetry");

    // --- Compile linux-gamepad (gamepad/joystick support, depends on udev, evdev) ---
    let gamepad = manifest_dir.join("src/manifest_libs/linux-gamepad");
    let mut linux_gamepad = cc::Build::new();
    linux_gamepad.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    linux_gamepad.include(local_inc.join("linux-gamepad"));
    linux_gamepad.include(gamepad.join("src"));
    for f in &[
        "src/gamepad.cpp",
        "src/gamepad_mapping.cpp",
        "src/gamepad_manager.cpp",
        "src/linux_joystick_manager.cpp",
        "src/linux_joystick.cpp",
    ] {
        linux_gamepad.file(gamepad.join(f));
    }
    linux_gamepad.compile("mcpelauncher-linux-gamepad");

    // --- Compile gamewindow (window/input management, depends on linux-gamepad, eglut) ---
    let gwin = manifest_dir.join("src/manifest_libs/gamewindow");
    let mut gamewindow = cc::Build::new();
    gamewindow.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");
    gamewindow.include(local_inc.join("game-window"));
    gamewindow.include(local_inc.join("linux-gamepad"));
    gamewindow.include(local_inc.join("eglut"));
    gamewindow.include(&gwin);
    for f in &[
        "game_window_manager.cpp",
        "game_window_error_handler.cpp",
        "joystick_manager.cpp",
        "window_eglut.cpp",
        "window_manager_eglut.cpp",
        "joystick_manager_linux_gamepad.cpp",
        "window_with_linux_gamepad.cpp",
    ] {
        gamewindow.file(gwin.join(f));
    }
    gamewindow.compile("mcpelauncher-gamewindow");

    // --- Compile client-side C++ (JNI bridge + JNI support classes) ---
    let mut client = cc::Build::new();
    client.cpp(true)
        .std("c++17")
        .flag_if_supported("-w");

    // manifest_dir/src/ is added first so #include "jni/foo.h" resolves
    // to the local copy before falling through to manifest_headers/.
    client.include(manifest_dir.join("src"));                          // local jni/ copies, stubs, etc.
    client.include(manifest_dir.join("src/manifest_headers"));         // copied mcpelauncher-client/src/ headers

    // Match the cmake build's EnableJNIVMGC flag to avoid ODR violations in
    // template instantiations (ToJNIType, JNICast, etc. from jnitypes.h).
    client.define("EnableJNIVMGC", "1");
    client.include(local_inc.join("libjnivm"));                       // baron, fake-jni, jnivm, jni.h
    client.include(local_inc.join("android-support-headers"));        // android/*, EGL/*
    client.include(local_inc.join("game-window"));                    // game_window.h, key_mapping.h
    client.include(local_inc.join("logger"));                         // log.h
    client.include(local_inc.join("mcpelauncher-common"));            // mcpelauncher/*, properties/*
    client.include(local_inc.join("msa-daemon-client"));              // msa/client/*.h
    client.include(local_inc.join("cll-telemetry"));                  // cll/*
    client.include(local_inc.join("file-util"));                      // EnvPathUtil.h (needed by msa-daemon-client headers)
    client.include(local_inc.join("mcpelauncher-core"));              // mcpelauncher/minecraft_version.h
    client.include(local_inc.join("sdl3"));                           // SDL3/SDL.h
    client.include(local_inc.join("daemon-utils"));                   // needed by msa-daemon-client headers
    client.include(local_inc.join("simple-ipc"));                     // simpleipc/*
    client.include(local_inc.join("epoll-shim"));                     // epoll/* (might be needed by android headers)
    client.include(local_inc.join("properties-parser"));              // properties_parser.h
    client.include(local_inc.join("mcpelauncher-errorwindow"));       // errorwindow.h
    client.include(local_inc.join("linux-gamepad"));                  // gamepad_mappings.h
    client.include(local_inc.join("file-picker"));                    // file_picker_factory.h
    client.include(local_inc.join("libc-shim"));                      // libc_shim.h
    if nlohmann_json_include.exists() {
        client.include(nlohmann_json_include);
    }
    // Our stub mcpelauncher/linker.h that replaces the bionic one
    client.include(manifest_dir.join("include"));

    // All JNI class implementations (with exclusions for Rust-ported files)
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
        "pulseaudio.cpp",
        "sdl3audio.cpp",
        "xbox_live.cpp",
    ].into_iter().collect();
    for entry in std::fs::read_dir(manifest_dir.join("src/jni")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().and_then(|s| s.to_str()) == Some("cpp") && !excluded_jni.contains(fname) {
            client.file(path);
        }
    }

    // Replace settings.cpp, core_patches.cpp, cll_upload_auth_step.cpp,
    // xal_webview_factory.cpp with stubs in our crate.
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
    ];
    for f in &stub_files {
        let path = manifest_dir.join("src").join(f);
        if path.exists() {
            client.file(path);
        }
    }
    // Auto-generated symbol arrays (needed by jni_bridge.cpp for android_symbols.h)
    client.include(local_inc.join("minecraft-imported-symbols"));

    // Stub definitions for symbols from main.cpp that are not compiled
    client.file(manifest_dir.join("src").join("main_stubs.cpp"));

    // jni_bridge.cpp replaced by jni_bridge_stub.cpp (above) + Rust code (rust_bridge.rs)

    // Compile all libjnivm sources ourselves to eliminate struct layout ODR
    // violations with the cmake-built archives (GCC 16 padding/alignment).
    client.define("HAVE_LOGGER", "1");
    client.define("JNI_DEBUG", "1");
    client.define("JNI_RETURN_NON_ZERO", "1");
    client.define("JNIVM_FAKE_JNI_SYNTAX", "0");
    client.define("JNIVM_FAKE_JNI_MINECRAFT_LINUX_COMPAT", "1");
    client.include(local_inc.join("libjnivm/src"));
    let jnivm_src = local_inc.join("libjnivm/src");
    // jnivm core (matches cmake jnivm target)
    for f in &[
        "jnivm/env.cpp",
        "jnivm/vm.cpp",
        "jnivm/method.cpp",
        "jnivm/object.cpp",
        "jnivm/internal/array.cpp",
        "jnivm/internal/bytebuffer.cpp",
        "jnivm/internal/field.cpp",
        "jnivm/internal/findclass.cpp",
        "jnivm/internal/jValuesfromValist.cpp",
        "jnivm/internal/method.cpp",
        "jnivm/internal/skipJNIType.cpp",
        "jnivm/internal/string.cpp",
        "jnivm/internal/stringUtil.cpp",
    ] {
        client.file(jnivm_src.join(f));
    }
    // codegen (compiled when JNI_DEBUG is ON)
    for f in &[
        "jnivm/internal/codegen/class.cpp",
        "jnivm/internal/codegen/field.cpp",
        "jnivm/internal/codegen/method.cpp",
        "jnivm/internal/codegen/namespace.cpp",
        "jnivm/internal/codegen/parseJNIType.cpp",
        "jnivm/internal/codegen/vm.cpp",
    ] {
        client.file(jnivm_src.join(f));
    }
    // fake-jni (matches cmake fake-jni target)
    for f in &["fake-jni/fake-jni.cpp", "fake-jni/jvm.cpp", "fake-jni/method.cpp"] {
        client.file(jnivm_src.join(f));
    }
    // baron (matches cmake baron target)
    client.file(jnivm_src.join("baron/jvm.cpp"));

    client.compile("mcpelauncher-client-jni");

    // System libraries (dylib)
    // Emit both `rustc-link-lib` (for lib target) and `rustc-link-arg-bins`
    // (for bin target — same-package lib+bin doesn't propagate native deps).
    static DYLIB_NAMES: &[&str] = &[
        "stdc++", "pthread", "dl", "m", "z",
        "GL", "EGL", "curl", "crypto", "ssl",
        "SDL2-2.0", "pulse", "pulse-simple",
        "X11", "evdev", "png", "udev",
    ];
    for name in DYLIB_NAMES {
        println!("cargo:rustc-link-lib=dylib={name}");
        println!("cargo:rustc-link-arg-bins=-l{name}");
    }

    // Static C++ libs from cc::Build — same propagation fix.
    static STATIC_LIBS: &[&str] = &[
        "mcpelauncher-client-bridge",
        "linker",
        "linker-c",
        "mcpelauncher-core",
        "mcpelauncher-manifest-libs",
        "mcpelauncher-base64",
        "mcpelauncher-simpleipc",
        "mcpelauncher-daemon-client-utils",
        "mcpelauncher-msa-daemon-client",
        "mcpelauncher-cll-telemetry",
        "mcpelauncher-linux-gamepad",
        "mcpelauncher-gamewindow",
        "mcpelauncher-client-jni",
    ];
    println!("cargo:rustc-link-arg-bins=-Wl,-Bstatic");
    for lib in STATIC_LIBS {
        println!("cargo:rustc-link-arg-bins=-l{lib}");
    }
    println!("cargo:rustc-link-arg-bins=-Wl,-Bdynamic");

    // Linker defsym flags (expected by mcpelauncher-linker)
    println!("cargo:rustc-link-arg-bins=-Wl,--defsym=__rela_iplt_start=0");
    println!("cargo:rustc-link-arg-bins=-Wl,--defsym=__rela_iplt_end=0");
    println!("cargo:rustc-link-arg-bins=-Wl,--defsym=__rel_iplt_start=0");
    println!("cargo:rustc-link-arg-bins=-Wl,--defsym=__rel_iplt_end=0");

    // Watch all C++ source files compiled by this build script
    println!("cargo:rerun-if-changed=src/capi.cpp");
    println!("cargo:rerun-if-changed=src/main_stubs.cpp");
    for f in &linker_files {
        println!("cargo:rerun-if-changed=src/mcpelauncher-linker/{}", f);
    }
    for f in &[
        "src/hook.cpp",
        "src/mod_loader.cpp",
        "src/hybris_utils.cpp",
        "src/hybris_android_log_hook.cpp",
        "src/crash_handler.cpp",
        "src/patch_utils.cpp",
        "src/minecraft_utils.cpp",
        "src/minecraft_version.cpp",
        "src/fmod_utils.cpp",
    ] {
        println!("cargo:rerun-if-changed=src/mcpelauncher-core/{}", f);
    }
    for f in &stub_files {
        println!("cargo:rerun-if-changed=src/{}", f);
    }
    // Watch JNI files (found dynamically)
    for entry in std::fs::read_dir(manifest_dir.join("src/jni")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("cpp") {
            if let Some(rel) = path.to_str() {
                println!("cargo:rerun-if-changed={}", rel);
            }
        }
    }
    // Watch manifest_libs sources
    println!("cargo:rerun-if-changed=src/manifest_libs/base64/base64.cpp");
    for f in &[
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
    ] {
        println!("cargo:rerun-if-changed=src/manifest_libs/simpleipc/{}", f);
    }
    println!("cargo:rerun-if-changed=src/manifest_libs/daemon-utils/client/src/daemon_launcher.cpp");
    for f in &["src/service_client.cpp", "src/token.cpp"] {
        println!("cargo:rerun-if-changed=src/manifest_libs/msa-daemon-client/{}", f);
    }
    for f in &[
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
    ] {
        println!("cargo:rerun-if-changed=src/manifest_libs/cll-telemetry/{}", f);
    }
    for f in &[
        "src/gamepad.cpp",
        "src/gamepad_mapping.cpp",
        "src/gamepad_manager.cpp",
        "src/linux_joystick_manager.cpp",
        "src/linux_joystick.cpp",
    ] {
        println!("cargo:rerun-if-changed=src/manifest_libs/linux-gamepad/{}", f);
    }
    for f in &[
        "game_window_manager.cpp",
        "game_window_error_handler.cpp",
        "joystick_manager.cpp",
        "window_eglut.cpp",
        "window_manager_eglut.cpp",
        "joystick_manager_linux_gamepad.cpp",
        "window_with_linux_gamepad.cpp",
    ] {
        println!("cargo:rerun-if-changed=src/manifest_libs/gamewindow/{}", f);
    }
}
