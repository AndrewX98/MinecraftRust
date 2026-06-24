fn main() {
    cc::Build::new()
        .file("src/variadic.c")
        .compile("libc_shim_variadic");
}
