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
    static STATIC_LIBS: &[&str] = &[
        "mcpelauncher-client-bridge",
        "mcpelauncher-core",
        "mcpelauncher-manifest-libs",
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
}
