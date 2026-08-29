//! Phase timing — zero cost unless `client` is built with `--features perf`.
//! With `perf` off all fns are `#[inline(always)]` no-ops so `perf::span("x", || f())` compiles to just `f()`.

#[cfg(feature = "perf")]
mod imp {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    pub fn init() {
        let _ = START.get_or_init(Instant::now);
    }
    fn elapsed_ms() -> u128 {
        START.get().map(|s| s.elapsed().as_millis()).unwrap_or(0)
    }
    pub fn checkpoint(label: &str) {
        eprintln!("[PERF] checkpoint label={} elapsed_ms={}", label, elapsed_ms());
    }
    pub fn span<R>(label: &str, f: impl FnOnce() -> R) -> R {
        let t0 = Instant::now();
        let r = f();
        let dt = t0.elapsed();
        eprintln!("[PERF] span label={} ms={} us={}", label, dt.as_millis(), dt.as_micros());
        r
    }
    pub fn span_ms(label: &str, ms: u128) {
        eprintln!("[PERF] span label={} ms={}", label, ms);
    }
}

#[cfg(not(feature = "perf"))]
mod imp {
    #[inline(always)] pub fn init() {}
    #[inline(always)] pub fn checkpoint(_: &str) {}
    #[inline(always)] pub fn span<R>(_: &str, f: impl FnOnce() -> R) -> R { f() }
    #[inline(always)] pub fn span_ms(_: &str, _: u128) {}
}

pub use imp::{checkpoint, init, span, span_ms};
