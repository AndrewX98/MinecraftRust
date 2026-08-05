fn main() {
    // System libraries (dylib)
    // Emit both `rustc-link-lib` (for lib target) and `rustc-link-arg-bins`
    // (for bin target — same-package lib+bin doesn't propagate native deps).
    static DYLIB_NAMES: &[&str] = &[
        "stdc++", "pthread", "dl", "m", "z",
        "GL", "EGL", "curl", "crypto", "ssl",
        "pulse", "pulse-simple",
        "X11", "evdev", "png", "udev",
    ];
    for name in DYLIB_NAMES {
        println!("cargo:rustc-link-lib=dylib={name}");
        println!("cargo:rustc-link-arg-bins=-l{name}");
    }

    // Static C++ libs from cc::Build (compiled by cpp-bridge-sys).
    // cc::Build emits `rustc-link-lib=static=...` which reaches the lib
    // target but not the binary — same-package lib+bin skips the rlib.
    // Phase 10 removed `mcpelauncher-client-bridge` (capi.cpp deleted).
    // mcpelauncher-core was removed when its last two files
    // (android_log_varargs.cpp, jnivm_mod_api.cpp) were ported to Rust.
    // mcpelauncher-cll-telemetry was removed when cll-telemetry was ported
    // to the Rust crate crates/cll-telemetry (client/src/cll_telemetry.rs).
    static STATIC_LIBS: &[&str] = &[
        "mcpelauncher-gamewindow",
        "mcpelauncher-client-jni",
    ];
    println!("cargo:rustc-link-arg-bins=-Wl,-Bstatic");
    for lib in STATIC_LIBS {
        println!("cargo:rustc-link-arg-bins=-l{lib}");
    }
    println!("cargo:rustc-link-arg-bins=-Wl,-Bdynamic");
}
