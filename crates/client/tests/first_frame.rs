//! First-frame render test — spawns the real `client` binary and waits for
//! `eglSwapBuffers ok n=0` (first `fake_egl_swap_buffers`).
//!
//! Parses `[PERF]` lines from the child (`client::perf` + `linker`) and prints
//! a clean per-phase breakdown plus wall total to first frame.
//!
//! ```sh
//! cargo test -p client --test first_frame -- --ignored --nocapture
//! # verbose — also dump every child line:
//! VERBOSE=1 cargo test -p client --test first_frame -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const TIMEOUT: Duration = Duration::from_secs(30);
const FIRST_FRAME_MARKER: &str = "eglSwapBuffers ok n=0";

#[derive(Debug, Clone)]
pub struct PerfSpan {
    pub label: String,
    pub ms: u128,
    pub us: Option<u128>,
}

#[derive(Debug)]
pub struct FirstFrameResult {
    pub elapsed: Duration,
    pub spans: Vec<PerfSpan>,
}

fn parse_perf_span(line: &str) -> Option<PerfSpan> {
    // "[PERF] span label=foo ms=12 us=12345" or "[PERF] checkpoint label=foo elapsed_ms=12"
    if !line.contains("[PERF]") {
        return None;
    }
    // span
    if line.contains("span label=") {
        let label = line.split("label=").nth(1)?.split_whitespace().next()?.to_string();
        let ms_str = line.split("ms=").nth(1)?.split_whitespace().next()?;
        let ms: u128 = ms_str.parse().ok()?;
        let us = line.split("us=").nth(1).and_then(|s| s.split_whitespace().next()?.parse().ok());
        return Some(PerfSpan { label, ms, us });
    }
    if line.contains("checkpoint label=") {
        let label = line.split("label=").nth(1)?.split_whitespace().next()?.to_string();
        let ms_str = line.split("elapsed_ms=").nth(1)?.split_whitespace().next()?;
        let ms: u128 = ms_str.parse().ok()?;
        return Some(PerfSpan { label, ms, us: None });
    }
    None
}

pub async fn measure_first_frame(bin: &PathBuf, game_dir: &PathBuf) -> Result<FirstFrameResult, String> {
    let start = Instant::now();
    let default_log = if bin.to_string_lossy().contains("release") { "error" } else { "info" };
    let verbose = std::env::var("VERBOSE").is_ok_and(|v| v == "1" || v == "true");
    let mut child = Command::new(bin)
        .arg("-dg")
        .arg(game_dir)
        .env("RUST_LOG", std::env::var("RUST_LOG").unwrap_or_else(|_| default_log.to_string()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;

    let stderr = child.stderr.take().expect("piped stderr");
    let stdout = child.stdout.take().expect("piped stdout");
    // drain stdout quietly unless VERBOSE
    tokio::spawn(async move {
        let mut r = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = r.next_line().await {
            if std::env::var("VERBOSE").is_ok() {
                println!("[client stdout] {line}");
            }
        }
    });

    let mut lines = BufReader::new(stderr).lines();
    let mut saw_any_swap = false;
    let mut spans: Vec<PerfSpan> = Vec::new();

    let res = tokio::time::timeout(TIMEOUT, async {
        while let Ok(Some(line)) = lines.next_line().await {
            if verbose {
                println!("[client] {line}");
            }
            if let Some(ps) = parse_perf_span(&line) {
                spans.push(ps);
            }
            if line.contains("eglSwapBuffers ok") {
                saw_any_swap = true;
            }
            if line.contains(FIRST_FRAME_MARKER)
                || (line.contains("eglSwapBuffers ok") && line.contains("n=0"))
            {
                return Ok::<(), ()>(());
            }
        }
        Err(())
    })
    .await;

    let elapsed = start.elapsed();
    let _ = child.kill().await;
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;

    match res {
        Ok(Ok(())) => Ok(FirstFrameResult { elapsed, spans }),
        Ok(Err(())) => Err(format!(
            "client exited before first frame (any_swap={saw_any_swap}) elapsed={elapsed:?} game_dir={}",
            game_dir.display()
        )),
        Err(_) => Err(format!(
            "TIMEOUT no '{FIRST_FRAME_MARKER}' within {TIMEOUT:?} elapsed={elapsed:?} game_dir={}",
            game_dir.display()
        )),
    }
}

fn find_client_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CLIENT_BIN") {
        let pb = PathBuf::from(&p);
        if pb.is_relative() {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let cand = manifest.join("../../").join(&pb);
            if cand.exists() {
                return cand;
            }
        }
        return pb;
    }
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_client") {
        return PathBuf::from(p);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in ["../../target/release/client", "../../target/debug/client"] {
        let candidate = manifest.join(rel);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("target/debug/client")
}

fn find_game_dir() -> Option<PathBuf> {
    for key in ["MINECRAFT_GAME_DIR", "GAME_DIR"] {
        if let Ok(p) = std::env::var(key) {
            let pb = PathBuf::from(&p);
            if pb.exists() {
                return Some(pb);
            } else {
                eprintln!("{key}={p} does not exist");
            }
        }
    }
    None
}

fn print_breakdown(res: &FirstFrameResult) {
    println!();
    println!("first_frame breakdown  (wall total = {} ms)", res.elapsed.as_millis());
    println!("{:-<44}", "");
    println!("{:<26} {:>8}", "phase", "ms");
    println!("{:-<44}", "");
    if res.spans.is_empty() {
        println!("(no per-phase data — rebuild client with --features perf)");
        println!("  cargo build -p client --release --features perf");
    } else {
        for s in &res.spans {
            let indent = if s.label.starts_with("goblin_parse") || s.label.starts_with("reloc:") { "  " } else { "" };
            println!("{:<26} {:>8}", format!("{}{}", indent, s.label), s.ms);
        }
    }
    println!("{:-<44}", "");
    println!("{:<26} {:>8}", "first_frame (wall)", res.elapsed.as_millis());
    println!();
    println!("first_frame_ms={}", res.elapsed.as_millis());
    println!("first_frame_s={:.3}", res.elapsed.as_secs_f64());
}

#[tokio::test]
#[ignore]
async fn first_frame_renders_within_30s() {
    if std::env::var("DISPLAY").is_err() {
        eprintln!("SKIP: no DISPLAY set — need X11/EGL (try: xvfb-run -a cargo test -p client --test first_frame -- --ignored --nocapture)");
        return;
    }
    let game_dir = match find_game_dir() {
        Some(d) => d,
        None => {
            eprintln!("SKIP: set MINECRAFT_GAME_DIR (or GAME_DIR) to extracted game dir");
            return;
        }
    };
    let bin = find_client_bin();
    if !bin.exists() {
        panic!("client binary not found at {} — run `cargo build -p client` first", bin.display());
    }
    match measure_first_frame(&bin, &game_dir).await {
        Ok(res) => {
            print_breakdown(&res);
            println!("PASS: first frame (n=0) in {:.2?} ( < {:?} )", res.elapsed, TIMEOUT);
            assert!(res.elapsed < TIMEOUT, "first frame took {:?} >= {:?}", res.elapsed, TIMEOUT);
        }
        Err(e) => panic!("{e}"),
    }
}
