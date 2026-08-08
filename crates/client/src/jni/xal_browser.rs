use std::ffi::{c_char, c_void, CStr, CString};

use libjnivm_sys::*;

extern "C" {
    fn jni_resolve_symbol(sym: *const c_char) -> *mut c_void;
}

fn iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

unsafe fn jstring_to_string(env: *mut JNIEnv, s: jstring) -> String {
    if env.is_null() || s.is_null() {
        return String::new();
    }
    let p = iface(env);
    match (*p).GetStringUTFChars {
        Some(f) => {
            let chars = f(env, s, std::ptr::null_mut());
            if chars.is_null() {
                return String::new();
            }
            let out = CStr::from_ptr(chars).to_string_lossy().into_owned();
            if let Some(rel) = (*p).ReleaseStringUTFChars {
                rel(env, s, chars);
            }
            out
        }
        None => String::new(),
    }
}

unsafe fn new_jstring(env: *mut JNIEnv, s: &str) -> jstring {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    let p = iface(env);
    match (*p).NewStringUTF {
        Some(f) => f(env, cstring(s).as_ptr()) as jstring,
        None => std::ptr::null_mut(),
    }
}

/// opId (J), then Context, starturl, endurl — the only 4 gp args the VM
/// forwards through jni_CallStaticVoidMethod. We only need opId + the two
/// URLs for CLI browser mode.
unsafe extern "C" fn browser_launch_show_url(
    env: *mut JNIEnv,
    _clazz: jclass,
    a1: i64,
    _a2: i64,
    a3: i64,
    _a4: i64,
) {
    let op_id = a1 as u64;
    let start_url = a3 as jstring;
    let start = jstring_to_string(env, start_url);
    log::info!(
        "xal_browser: showUrl(op={:016x}) start=\"{}\"",
        op_id,
        start
    );

    let final_url = match cli_browser_flow(&start) {
        Some(u) => u,
        None => {
            log::warn!("xal_browser: no final URL captured, cancelling");
            return;
        }
    };

    // Hand the final redirect URL back to XAL. Mimics mcpelauncher's
    // xal_webview: urlOperationSucceeded(opId, finalUrl, false, browserInfo).
    call_url_operation_succeeded(env, op_id, &final_url);
}

/// Present the sign-in URL. CLI mode: print and read a line from stdin.
/// Also best-effort: open the URL in the system browser (xdg-open) so an
/// interactive user can complete sign-in there.
fn cli_browser_flow(start: &str) -> Option<String> {
    eprintln!();
    eprintln!("===== Minecraft sign-in required =====");
    eprintln!("Sign in by visiting:");
    eprintln!("  {}", start);
    let _ = std::process::Command::new("xdg-open")
        .arg(start)
        .spawn()
        .and_then(|mut c| c.wait())
        .map(|_| ());
    eprintln!("After signing in, paste the final redirect URL here and press Enter");
    eprintln!("(empty line cancels):");
    eprintln!("------------------------------------");
    use std::io::{BufRead, BufReader};
    let mut line = String::new();
    let mut stdin = BufReader::new(std::io::stdin());
    if stdin.read_line(&mut line).is_err() {
        return None;
    }
    let out = line.trim().to_string();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

unsafe fn call_url_operation_succeeded(env: *mut JNIEnv, op_id: u64, final_url: &str) {
    let sym = jni_resolve_symbol(cstring(
        "Java_com_microsoft_xal_browser_BrowserLaunchActivity_urlOperationSucceeded",
    )
    .as_ptr());
    if sym.is_null() {
        log::warn!("xal_browser: urlOperationSucceeded symbol not resolved");
        return;
    }
    type FnT = unsafe extern "C" fn(
        _env: *mut JNIEnv,
        _clazz: jclass,
        op: u64,
        url: jstring,
        shared: jboolean,
        info: jstring,
    ) -> ();
    let f: FnT = std::mem::transmute(sym);
    let url_j = new_jstring(env, final_url);
    let info_j = new_jstring(env, "webkit-noDefault::0::none");
    f(env, std::ptr::null_mut(), op_id, url_j, 0, info_j);
    log::info!("xal_browser: sent urlOperationSucceeded(op_id={:x})", op_id);
}

fn reg(env: *mut JNIEnv) {
    let class_name = b"com/microsoft/xal/browser/BrowserLaunchActivity\0";
    let cls = unsafe { jnivm_find_class(env, class_name.as_ptr() as *const c_char) };
    if cls.is_null() {
        log::warn!("xal_browser: FindClass failed for BrowserLaunchActivity");
        return;
    }
    let methods = [
        JNINativeMethod {
            name: b"showUrl\0".as_ptr() as *const c_char,
            signature: b"(JLandroid/content/Context;Ljava/lang/String;Ljava/lang/String;I[Ljava/lang/String;[Ljava/lang/String;Z)V\0"
                .as_ptr() as *const c_char,
            fnPtr: browser_launch_show_url as *mut c_void,
        },
    ];
    let rc = unsafe {
        jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32)
    };
    if rc != 0 {
        log::warn!("xal_browser: RegisterNatives failed for showUrl (rc={})", rc);
    } else {
        log::info!("xal_browser: registered BrowserLaunchActivity.showUrl override");
    }
}

pub fn register(env: *mut JNIEnv) {
    reg(env);
}