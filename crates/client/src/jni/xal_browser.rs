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
    //
    // IMPORTANT: do NOT touch anything under <data>/xal/ here. XAL starts
    // reading its webview-flow state file the moment this returns; deleting
    // the "WebViewFlowId" marker underneath it makes the REST thread parse an
    // empty read and die on an uncaught Xal::ParseException ("Token root is
    // not an object") ~7ms later. Stale markers are cleaned up pre-launch by
    // sanitize_xal_store() instead.
    call_url_operation_succeeded(env, op_id, &final_url);
}

/// Scheme + host of the OAuth redirect used by the game's XAL client id.
/// Brave/Chrome will delegate a `ms-xal-0000000048183522://auth?...` URL to
/// whatever `xdg-mime` handler is registered for this scheme; we install a
/// tiny script that writes the URL to a known file the client polls.
const REDIRECT_SCHEME: &str = "ms-xal-0000000048183522";
const REDIRECT_FILE: &str = "xal-redirect-url";

fn redirect_capture_path() -> std::path::PathBuf {
    let cache = std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/.cache", std::env::var("HOME").unwrap_or_default()));
    std::path::PathBuf::from(cache).join("mcpelauncher").join(REDIRECT_FILE)
}

/// Install a desktop handler mapping the XAL redirect scheme to a script that
/// records the final redirect URL in the capture file (so it survives the
/// browser→OS handoff without an app visible in the portal picker).
fn ensure_redirect_handler() {
    use std::os::unix::fs::PermissionsExt;
    let capture = redirect_capture_path();
    let cache_dir = capture.parent().unwrap_or(std::path::Path::new("/tmp"));
    if let Err(e) = std::fs::create_dir_all(cache_dir) {
        log::warn!("xal_browser: create_dir_all {} failed: {}", cache_dir.display(), e);
        return;
    }
    let script = cache_dir.join("xal-redirect.sh");
    let script_body = format!(
        "#!/bin/bash\nexec >/dev/null 2>&1\nprintf '%s' \"$1\" > \"{}\"\n",
        capture.display()
    );
    if std::fs::write(&script, script_body).is_ok() {
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let apps_dir = std::path::PathBuf::from(home).join(".local/share/applications");
    if std::fs::create_dir_all(&apps_dir).is_err() {
        return;
    }
    let desktop = apps_dir.join("minecraft-xal-redirect.desktop");
    let desktop_body = format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=Minecraft XAL Redirect\nExec=\"{}\" %u\nMimeType=x-scheme-handler/{};\n",
        script.display(),
        REDIRECT_SCHEME
    );
    if std::fs::write(&desktop, desktop_body).is_ok() {
        let _ = std::process::Command::new("xdg-mime")
            .args([
                "default",
                "minecraft-xal-redirect.desktop",
                &format!("x-scheme-handler/{}", REDIRECT_SCHEME),
            ])
            .output();
        let _ = std::process::Command::new("update-desktop-database")
            .arg(&apps_dir)
            .output();
    }
    if script.exists() && desktop.exists() {
        log::info!(
            "xal_browser: installed redirect handler {} -> {}",
            desktop.display(),
            script.display()
        );
    }
}

/// Present the sign-in URL in the system browser, then wait for the flow to
/// complete:
///  - the browser redirects to `ms-xal-0000000048183522://auth?...` (captured
///    by the handler script into the capture file), and
///  - a manual paste of the redirect URL into stdin if it is a TTY.
/// Blocking the game thread for the duration is acceptable: this mirrors the
/// synchronous webview flow XAL expects.
fn cli_browser_flow(start: &str) -> Option<String> {
    eprintln!();
    eprintln!("===== Minecraft sign-in required =====");
    eprintln!("Sign in by visiting:");
    eprintln!("  {}", start);
    ensure_redirect_handler();
    // Clear any stale capture from a previous attempt.
    let _ = std::fs::remove_file(redirect_capture_path());
    let _ = std::process::Command::new("xdg-open").arg(start).spawn();
    eprintln!("Waiting for sign-in to complete in the browser…");
    eprintln!("If the browser shows “No apps installed that can open ms-xal-…”,");
    eprintln!("copy the full ms-xal-… URL from the address bar and paste it here:");
    eprintln!("------------------------------------");

    use std::io::{BufRead, BufReader, IsTerminal};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let stdin_reader = std::io::stdin();
    let capture = redirect_capture_path();
    let (tx, rx) = mpsc::channel::<Option<String>>();
    if stdin_reader.is_terminal() {
        std::thread::spawn(move || {
            let mut line = String::new();
            if BufReader::new(std::io::stdin()).read_line(&mut line).is_ok() {
                let out = line.trim().to_string();
                let _ = tx.send(if out.is_empty() { None } else { Some(out) });
            } else {
                let _ = tx.send(None);
            }
        });
    }

    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        if let Ok(content) = std::fs::read_to_string(&capture) {
            let out = content.trim().to_string();
            if !out.is_empty() {
                let _ = std::fs::remove_file(&capture);
                return Some(out);
            }
        }
        if stdin_reader.is_terminal() {
            if let Ok(msg) = rx.recv_timeout(Duration::from_millis(250)) {
                return match msg {
                    Some(u) => Some(u),
                    None => None,
                };
            }
        }
        if Instant::now() >= deadline {
            log::warn!("xal_browser: sign-in flow timed out after 600s");
            return None;
        }
        std::thread::sleep(Duration::from_millis(250));
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

/// XAL persists an interactive sign-in as a "pending webview flow" (a store
/// file under `xal/` carrying `WebViewFlowId` + `WelcomeBackSisu`). These are
/// resume tickets for silent auto-login on the next boot and MUST be
/// preserved — see `sanitize_xal_store()`, which no longer deletes them.
/// Deleting them mid-session — especially between urlOperationSucceeded and
/// XAL's completion handler reading the file back — races the REST thread's
/// read and aborts the process with an uncaught
/// Xal::ParseException ("Token root is not an object").
///
/// Boot-time XAL store repair. Runs BEFORE the game's XAL reads the store.
///
/// - Files containing "Serialized to SharedPreferences" are XAL's pointer to
///   the ECDSA device key living in SharedPreferences (`ecdsa_key.json` on
///   our side, see `jni_support::ecdsa_impl`). They are only junk when STALE
///   (their `Id` no longer matches the stored key) — a matching pointer is
///   exactly what XAL needs to restore the cached key and reuse its D/T
///   tokens, so it must be kept. Quarantining a matching pointer forces a
///   fresh device registration every boot and churns the token cache.
/// - Files containing WebViewFlowId (`WelcomeBackSisu` resume tickets) are
///   NEVER touched: XAL resumes them on boot for silent auto-login. Deleting
///   them is what forced a manual sign-in on every launch (the reference C++
///   launcher preserves the store untouched and signs in automatically).
///   Mid-session deletion is likewise forbidden: unlinking the flow file
///   under XAL's REST thread aborts the process with an uncaught
///   Xal::ParseException ("Token root is not an object").
/// - Token files (refresh_token / Xtoken / ...) are NEVER touched here —
///   deleting those is what forced repeated manual re-sign-ins.
/// - Files we already quarantined are skipped: quarantine lives in the
///   sibling `xal-quarantine/` directory (NOT as `.bad` files inside `xal/`,
///   which XAL still opens and parses during its store enumeration).
pub fn sanitize_xal_store() {
    let dir = unsafe {
        let ptr = crate::path_helper::path_helper_get_primary_data_directory();
        if ptr.is_null() {
            return;
        }
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    };
    let data_dir = std::path::Path::new(&dir);
    let store = data_dir.join("xal");
    let entries = match std::fs::read_dir(&store) {
        Ok(e) => e.filter_map(|x| x.ok()).collect::<Vec<_>>(),
        Err(_) => return,
    };
    let stored_id = ecdsa_store_id(data_dir);
    // Quarantine lives OUTSIDE the store: XAL opens every file under xal/
    // (including `*.bad` renames) during enumeration, so in-place renames
    // would still be parsed.
    let quarantine = data_dir.join("xal-quarantine");
    for e in entries {
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let content = match std::fs::read_to_string(e.path()) {
            Ok(s) => s,
            Err(_) => continue, // binary or unreadable: leave it alone
        };
        let corrupt_key = content.contains("Serialized to SharedPreferences");
        if !corrupt_key {
            continue;
        }
        // Key pointer: keep it when it matches the stored device key.
        let file_id = serde_json::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|v| {
                v.get("Id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            });
        if file_id.is_some() && file_id == stored_id {
            log::info!(
                "xal_store: keeping ECDSA key pointer {} (matches stored key {})",
                e.path().display(),
                stored_id.as_deref().unwrap_or("?")
            );
            continue;
        }
        log::warn!("xal_store: quarantining stale ECDSA key pointer ({})", e.path().display());
        if std::fs::create_dir_all(&quarantine).is_ok() {
            let dest = quarantine.join(e.file_name());
            let _ = std::fs::rename(e.path(), dest);
        }
    }
}

/// Read the `Id` field of the Rust ECDSA store (`ecdsa_key.json`).
/// `None` when the store is missing or unparsable.
fn ecdsa_store_id(data_dir: &std::path::Path) -> Option<String> {
    let data = std::fs::read_to_string(data_dir.join("ecdsa_key.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    let id = v.get("Id")?.as_str()?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
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