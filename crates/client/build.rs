fn main() {
    // System libraries (dylib)
    // Emit both `rustc-link-lib` (for lib target) and `rustc-link-arg-bins`
    // (for bin target — same-package lib+bin doesn't propagate native deps).
    static DYLIB_NAMES: &[&str] = &[
        "stdc++", "pthread", "dl", "m", "z",
        "GL", "EGL", "curl", "crypto", "ssl",
        "SDL2-2.0", "SDL3", "pulse", "pulse-simple",
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
}
