mod capi;
mod rust_bridge;
mod jni_support;
mod file_picker;
mod eglut;
mod fake_looper;
mod main_activity;
mod jnivm_class_wrappers;
mod jnivm_globals;
mod jni;
mod text_input_handler;

use std::ffi::{c_char, c_void, CStr};

extern "C" {
    fn fake_thread_mover_store_start_thread_id();
    fn fake_thread_mover_execute_main_thread();
    fn jni_resolve_symbol(sym: *const c_char) -> *mut c_void;
    fn path_helper_get_primary_data_directory() -> *const c_char;
}

fn main() {
    // MinecraftUtils::workaroundLocaleBug — force a locale that MCPE's libc++
    // can construct. Without this, collate_byname throws on Android-style
    // names like "en.UTF-8" (from getLocale() "en" + ".UTF-8") on Linux hosts
    // that only ship C / C.UTF-8.
    std::env::set_var("LC_ALL", "C");

    // Mesa 23.1+ black screen with RenderDragon: hide instanced-array extension
    // so the game does not enable the broken path. Complements FakeEGL nulling
    // of glDraw*Instanced* / glVertexAttribDivisor* in eglGetProcAddress.
    // Official mcpelauncher troubleshooting recommendation.
    if std::env::var_os("MESA_EXTENSION_OVERRIDE").is_none() {
        std::env::set_var(
            "MESA_EXTENSION_OVERRIDE",
            "-GL_EXT_instanced_arrays,-GL_ANGLE_instanced_arrays,-GL_NV_instanced_arrays,-GL_EXT_draw_instanced",
        );
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let args: Vec<String> = std::env::args().collect();

    let mut parser = util::arg_parser::ArgParser::new();
    let game_dir = util::arg_parser::arg_string(&mut parser, "--game-dir", "-dg", "Directory with the game and assets", "");
    let data_dir = util::arg_parser::arg_string(&mut parser, "--data-dir", "-dd", "Directory to use for the data", "");
    let cache_dir = util::arg_parser::arg_string(&mut parser, "--cache-dir", "-dc", "Directory to use for cache", "");
    let print_version = util::arg_parser::arg_flag(&mut parser, "--version", "-v", "Print version info");

    let had_help_flag = args.iter().any(|a| a == "-h" || a == "--help");

    if !parser.parse(&args) {
        if !had_help_flag {
            if args.len() <= 1 {
                eprintln!("Error: --game-dir (-dg) is required");
                parser.print_help();
            }
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if print_version.get() {
        println!("mcpelauncher-client-rust {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let minecraft_dir = game_dir.get();
    if minecraft_dir.is_empty() {
        eprintln!("Error: --game-dir (-dg) is required");
        parser.print_help();
        std::process::exit(1);
    }

    log::info!("mcpelauncher-client: starting");

    // Set up paths — only pass -dd/-dc when explicitly provided.
    // C++ PathHelper defaults to XDG dirs (~/.local/share/mcpelauncher/, ~/.cache/mcpelauncher/).
    let data_dir_str = data_dir.get();
    let cache_dir_str = cache_dir.get();
    capi::setup_paths(
        Some(&minecraft_dir),
        if data_dir_str.is_empty() { None } else { Some(&data_dir_str) },
        if cache_dir_str.is_empty() { None } else { Some(&cache_dir_str) },
    );

    // Init version info
    capi::init_version("com.mojang.minecraftpe", 0);

    // Set up filesystem rewrite rules (matching C++ client behavior).
    // Redirects Minecraft's Android data paths to the real data dir
    // so cache files (~2GB) land in XDG dirs, not the working directory.
    let data_dir_real = unsafe {
        let ptr = path_helper_get_primary_data_directory();
        if ptr.is_null() { String::new() } else { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    };
    if !data_dir_real.is_empty() {
        // chdir to data dir so the game's relative paths (minecraftpe/, xal/, games/)
        // resolve to the data directory instead of the working directory.
        if let Err(e) = std::env::set_current_dir(&data_dir_real) {
            log::warn!("mcpelauncher-client: failed to chdir to data dir: {}", e);
        }
        libc_shim::path_rewrite::set_rules(&[
            ("/data/data/com.mojang.minecraftpe".to_string(), data_dir_real.clone()),
            ("/data/data".to_string(), data_dir_real.clone()),
        ]);
        log::info!("mcpelauncher-client: chdir to data dir: {}", data_dir_real);
    }

    // Get merged C++ + Rust libc symbols from the C bridge
    let libc_syms = capi::get_libc_symbols_from_cpp();
    log::info!("mcpelauncher-client: {} merged libc symbols from C++ bridge", libc_syms.len());

    if !libc_syms.is_empty() {
        linker::load_library("libc.so", &libc_syms);
    }

    // Load core libraries (loads libm, libz, etc. via C++ linker)
    match capi::load_core_libraries(&minecraft_dir) {
        Ok(()) => log::info!("mcpelauncher-client: core libraries loaded successfully"),
        Err(code) => log::error!("mcpelauncher-client: failed to load core libraries (code={})", code),
    }

    // Set up android hooks (FakeLooper, FakeAssetManager, FakeInputQueue, FakeWindow,
    // CorePatches) — MUST happen before loading the game library so its relocations
    // resolve to real implementations.
    capi::setup_android_hooks();
    log::info!("mcpelauncher-client: android hooks registered successfully");

    // Create window via GameWindowManager and register GLES2 symbols from real GL driver.
    capi::create_window_and_setup_graphics();
    log::info!("mcpelauncher-client: window created and GLES2 symbols registered");

    // Try loading libminecraftpe.so
    log::info!("mcpelauncher-client: attempting to load libminecraftpe.so...");
    let game_handle = match capi::load_minecraft() {
        Ok(handle) => {
            log::info!("mcpelauncher-client: libminecraftpe.so loaded at {:p}", handle);
            handle
        }
        Err(()) => {
            log::error!("mcpelauncher-client: failed to load libminecraftpe.so");
            return;
        }
    };

    // Set the game handle for the native symbol resolver
    unsafe { rust_bridge::jni_set_game_handle(game_handle) };

    // Create C++ JniSupport for FakeLooper (window callbacks, text input, etc.)
    let _cpp_support = capi::create_cpp_jni_support();
    capi::set_fake_looper_jni_support(_cpp_support);
    log::info!("mcpelauncher-client: C++ JniSupport created for FakeLooper");

    // Register game native methods (nativeRegisterThis, etc.) with the C++ Baron JVM.
    // This MUST happen after libminecraftpe.so is loaded but before startGame().
    log::info!("mcpelauncher-client: registering C++ JniSupport natives...");
    capi::register_minecraft_natives_cpp(_cpp_support, game_handle);

    // Create Rust JniSupport with libjnivm-sys VM and register all classes
    log::info!("mcpelauncher-client: initializing Rust JNI VM...");
    let rust_support = unsafe { jni_support::jni_support_new() };
    log::info!("mcpelauncher-client: Rust JNI VM created and classes registered");

    // Tell FakeLooper about the Rust JniSupport so it can forward the window
    capi::set_fake_looper_rust_jni_support(rust_support);

    // Register native methods from the game library
    log::info!("mcpelauncher-client: registering native methods...");
    unsafe { jni_support::jni_support_register_natives(rust_support, Some(jni_resolve_symbol)) };
    log::info!("mcpelauncher-client: native methods registered");

    // Create FakeAssetManager for game asset loading
    let assets_dir = format!("{}/assets", minecraft_dir);
    capi::create_and_set_global_asset_manager(&assets_dir);
    log::info!("mcpelauncher-client: FakeAssetManager created with root: {}", assets_dir);

    // Resolve game startup symbols
    let game_create = capi::dlsym(game_handle, "GameActivity_onCreate");
    let stbi_load = capi::dlsym(game_handle, "stbi_load_from_memory");
    let stbi_free = capi::dlsym(game_handle, "stbi_image_free");

    // Start the game via Rust JniSupport (libjnivm-sys VM)
    // The C++ JniSupport is still used by FakeLooper for event dispatch;
    // Rust populates its callbacks after GameActivity_onCreate returns.
    log::info!("mcpelauncher-client: starting game via Rust JniSupport...");
    unsafe {
        fake_thread_mover_store_start_thread_id();
        jni_support::jni_support_start_game(rust_support, _cpp_support, game_create, stbi_load, stbi_free);
    }
    log::info!("mcpelauncher-client: game started, entering event loop...");

    // Block the main thread forever (game thread runs independently)
    unsafe { fake_thread_mover_execute_main_thread() };
}
