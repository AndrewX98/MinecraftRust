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
    // Export launcher-provided JNI natives (`Java_com_*`) from the client
    // binary so `jni_resolve_symbol` (dlsym(NULL)) can find them, exactly like
    // the C++ mcpelauncher-client exported MainActivity::initializeXboxLive etc.
    println!("cargo:rustc-link-arg-bins=-Wl,--export-dynamic-symbol=Java_*");
}
