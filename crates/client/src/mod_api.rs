//! Rust mod-facing API funnel (previously the C++ shim `jnivm_mod_api.cpp`).
//! Moved here once the crate switched to nightly and enabled
//! `#![feature(c_variadic)]`.
//!
//! `mc_mod_log` / `mc_mod_vlog` are true C varargs wrappers over the Rust
//! logger. `mc_mod_request_google_credentials` keeps the fork/exec helper
//! launch; the caller's return address (the C++ `__builtin_return_address(0)`)
//! is captured by a tiny `#[naked]` trampoline that reads `[rsp]` before the
//! prologue. `mc_mod_jnivm_register_method` cannot be ported (it binds
//! `jnivm::Method` native handles on the C++ FakeJni VM); it is stubbed to
//! return false and full `jnivm` mod integration is deferred to the JNI port
//! (`libjnivm-sys`). None of these are on the boot path (mods are not loaded).

use std::arch::naked_asm;
use std::ffi::{c_char, c_int, c_void, CStr, CString};

fn mod_log_level(level: c_int) -> util::logger::LogLevel {
    match level {
        0 => util::logger::LogLevel::Trace,
        1 => util::logger::LogLevel::Debug,
        2 => util::logger::LogLevel::Info,
        3 => util::logger::LogLevel::Warn,
        _ => util::logger::LogLevel::Error,
    }
}

unsafe fn log_mod_message(level: c_int, tag: *const c_char, fmt: *const c_char, ap: *mut c_void) {
    if tag.is_null() || fmt.is_null() {
        return;
    }
    let tag = CStr::from_ptr(tag).to_string_lossy().into_owned();
    let text = crate::android_log_hook::format_va_list(fmt, ap);
    util::logger::Log::log(mod_log_level(level), &tag, &text);
}

#[no_mangle]
pub unsafe extern "C" fn mc_mod_log(
    level: c_int,
    tag: *const c_char,
    fmt: *const c_char,
    args: ...,
) {
    let ap = &args as *const std::ffi::VaList as *mut c_void;
    log_mod_message(level, tag, fmt, ap);
}

#[no_mangle]
pub unsafe extern "C" fn mc_mod_vlog(
    level: c_int,
    tag: *const c_char,
    fmt: *const c_char,
    args: ...,
) {
    let ap = &args as *const std::ffi::VaList as *mut c_void;
    log_mod_message(level, tag, fmt, ap);
}

// ---- Google credentials helper (fork/exec the mcpelauncher-ui-qt helper) ----

/// `GoogleCredentials` passed by value to `onsuccess` (mirrors the C++ struct).
#[repr(C)]
struct GoogleCredentials {
    email: *const c_char,
    token: *const c_char,
}

type CredentialsCb = unsafe extern "C" fn(GoogleCredentials);
type ErrorCb = unsafe extern "C" fn(*const c_char);

/// C++ `getUiExecutablePath`: prefer the bundled UI path, then PATH.
fn get_ui_executable_path() -> String {
    let app_dir = util::file_util::EnvPathUtil::get_app_dir();
    if let Some(p) = util::file_util::EnvPathUtil::find_in_path_with(
        "mcpelauncher-ui-qt",
        ".",
        Some(&app_dir),
    ) {
        return p;
    }
    util::file_util::EnvPathUtil::find_in_path("mcpelauncher-ui-qt")
        .unwrap_or_else(|| "mcpelauncher-ui-qt".to_string())
}

/// `MCPELAUNCHER_UI_PATH` default ".". The helper is launched with
/// `--request-google-credentials -v --mod <caller lib>`.
unsafe fn google_credentials_impl(
    onsuccess: *const c_void,
    onfailure: *const c_void,
    caller_ra: usize,
) {
    let onsuccess: CredentialsCb = std::mem::transmute(onsuccess);
    let onfailure: ErrorCb = std::mem::transmute(onfailure);

    let mod_name = match linker::dladdr(caller_ra as *const c_void) {
        Some((_, name)) => name,
        None => {
            util::logger::Log::error("Launcher", "Google credentials requested from unknown caller");
            onfailure(c"Unknown caller".as_ptr());
            return;
        }
    };
    util::logger::Log::info(
        "Launcher",
        &format!("Google credentials requested from {}", mod_name),
    );

    let ui_path = get_ui_executable_path();
    util::logger::Log::info(
        "Launcher",
        &format!("Executing google credentials helper: {}", ui_path),
    );

    const PIPE_STDOUT: usize = 0;
    const PIPE_STDERR: usize = 1;
    const PIPE_STDIN: usize = 2;
    const PIPE_READ: usize = 0;
    const PIPE_WRITE: usize = 1;
    let mut pipes = [[0i32; 2]; 3];
    libc::pipe(pipes[PIPE_STDOUT].as_mut_ptr());
    libc::pipe(pipes[PIPE_STDERR].as_mut_ptr());
    libc::pipe(pipes[PIPE_STDIN].as_mut_ptr());

    let args = [
        ui_path,
        "--request-google-credentials".to_string(),
        "-v".to_string(),
        "--mod".to_string(),
        mod_name,
    ];
    let argv: Vec<CString> = args.iter().map(|a| CString::new(a.as_str()).unwrap()).collect();
    let mut argv_ptrs: Vec<*const c_char> = argv.iter().map(|a| a.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());

    let pid = libc::fork();
    if pid == 0 {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        libc::dup2(pipes[PIPE_STDOUT][PIPE_WRITE], libc::STDOUT_FILENO);
        libc::dup2(pipes[PIPE_STDERR][PIPE_WRITE], libc::STDERR_FILENO);
        libc::dup2(pipes[PIPE_STDIN][PIPE_READ], libc::STDIN_FILENO);
        for p in [
            pipes[PIPE_STDIN][PIPE_WRITE],
            pipes[PIPE_STDOUT][PIPE_WRITE],
            pipes[PIPE_STDERR][PIPE_WRITE],
            pipes[PIPE_STDIN][PIPE_READ],
            pipes[PIPE_STDOUT][PIPE_READ],
            pipes[PIPE_STDERR][PIPE_READ],
        ] {
            libc::close(p);
        }
        let r = libc::execvp(argv_ptrs[0], argv_ptrs.as_ptr());
        let err = CStr::from_ptr(libc::strerror(*libc::__errno_location()))
            .to_string_lossy()
            .into_owned();
        eprintln!("Show: execvp() error {} {}", r, err);
        libc::close(libc::STDOUT_FILENO);
        libc::close(libc::STDERR_FILENO);
        libc::close(libc::STDIN_FILENO);
        libc::_exit(r);
    } else if pid > 0 {
        libc::close(pipes[PIPE_STDIN][PIPE_WRITE]);
        libc::close(pipes[PIPE_STDIN][PIPE_READ]);
        libc::close(pipes[PIPE_STDOUT][PIPE_WRITE]);
        libc::close(pipes[PIPE_STDERR][PIPE_WRITE]);
        let mut out_stdout = Vec::new();
        let mut out_stderr = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let r = libc::read(pipes[PIPE_STDOUT][PIPE_READ], buf.as_mut_ptr() as *mut c_void, buf.len());
            if r <= 0 {
                break;
            }
            out_stdout.extend_from_slice(&buf[..r as usize]);
        }
        loop {
            let r = libc::read(pipes[PIPE_STDERR][PIPE_READ], buf.as_mut_ptr() as *mut c_void, buf.len());
            if r <= 0 {
                break;
            }
            out_stderr.extend_from_slice(&buf[..r as usize]);
        }
        libc::close(pipes[PIPE_STDOUT][PIPE_READ]);
        libc::close(pipes[PIPE_STDERR][PIPE_READ]);
        let mut status: i32 = 0;
        loop {
            let err = libc::waitpid(pid, &mut status, 0);
            if err == -1 {
                if *libc::__errno_location() == libc::EINTR {
                    continue;
                }
                let msg = format!(
                    "Failed to wait for Google credentials process: {}",
                    CStr::from_ptr(libc::strerror(*libc::__errno_location())).to_string_lossy()
                );
                let cs = CString::new(msg).unwrap_or_default();
                onfailure(cs.as_ptr());
                return;
            }
            break;
        }
        if (status & 0x7f) != 0 {
            let msg = format!("Google credentials process terminated by signal {}", status & 0x7f);
            let cs = CString::new(msg).unwrap_or_default();
            onfailure(cs.as_ptr());
            return;
        }
        if (status & 0xff) != 0 {
            let cs = CString::new("Google credentials process did not exit normally").unwrap_or_default();
            onfailure(cs.as_ptr());
            return;
        }
        let code = (status >> 8) & 0xff;
        let stderr = String::from_utf8_lossy(&out_stderr).into_owned();
        if code == 0 {
            util::logger::Log::info("Launcher", "Obtained Google credentials from helper");
            let credstr = stderr
                .find("CRED=")
                .map(|pos| &stderr[pos + 5..])
                .map(|s| match s.find('\n') {
                    Some(nl) => &s[..nl],
                    None => s,
                });
            if let Some(credstr) = credstr {
                if let Some(sep) = credstr.find(':') {
                    let email = CString::new(&credstr[..sep]).unwrap_or_default();
                    let token = CString::new(&credstr[sep + 1..]).unwrap_or_default();
                    onsuccess(GoogleCredentials {
                        email: email.as_ptr(),
                        token: token.as_ptr(),
                    });
                    return;
                }
            }
            let msg = format!(
                "Failed to parse Google credentials from helper output{} stdout: {} stderr: {}",
                code,
                String::from_utf8_lossy(&out_stdout),
                stderr
            );
            let cs = CString::new(msg).unwrap_or_default();
            onfailure(cs.as_ptr());
        } else {
            let msg = format!(
                "Failed to get Google credentials exit code {} stdout: {} stderr: {}",
                code,
                String::from_utf8_lossy(&out_stdout),
                stderr
            );
            let cs = CString::new(msg).unwrap_or_default();
            onfailure(cs.as_ptr());
        }
    }
}

/// Naked trampoline: reads the caller's return address from `[rsp]` at entry
/// (the C++ `__builtin_return_address(0)`) and forwards it to the impl. The
/// `sub rsp, 8` keeps the stack 16-byte aligned for the `call`; the return
/// address lives at `[rsp + 8]` after it.
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn mc_mod_request_google_credentials(
    onsuccess: *const c_void,
    onfailure: *const c_void,
) {
    naked_asm!(
        "sub rsp, 8",
        "mov rdx, [rsp + 8]",
        "call {impl}",
        "add rsp, 8",
        "ret",
        impl = sym google_credentials_impl,
    );
}

/// Stub for `mc_mod_jnivm_register_method`: the C++ implementation registered
/// `ModHandle` native methods on the FakeJni `jnivm` VM, which Rust cannot
/// express. Mods are not loaded, so this returns false (cannot register) until
/// the `libjnivm-sys` JNI port covers the same ground.
#[no_mangle]
pub unsafe extern "C" fn mc_mod_jnivm_register_method(
    _env: *mut c_void,
    _cl: *mut c_void,
    _ty: i32,
    _name: *const c_char,
    _signature: *const c_char,
    _cbk: *mut c_void,
) -> bool {
    false
}
