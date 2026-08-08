use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;

use libjnivm_sys::*;

// libHttpClient.Android.so exports the HCHttpCall* C API; the Rust linker loads
// that .so, and jni_resolve_symbol (rust_bridge.rs) resolves these at runtime.
extern "C" {
    fn jni_resolve_symbol(sym: *const c_char) -> *mut c_void;
}

type HcGetBodyFn = unsafe extern "C" fn(call: *mut c_void, bytes: *mut *const u8, size: *mut u32) -> i32;

unsafe fn hc_symbol(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    jni_resolve_symbol(c.as_ptr())
}

// Read the request body set on the HCCallHandle by the game (via
// HCHttpCallRequestSetRequestBodyBytes) before doRequestAsync was invoked.
unsafe fn read_call_request_body(call_handle: i64) -> Vec<u8> {
    let sym = hc_symbol("HCHttpCallRequestGetRequestBodyBytes");
    if sym.is_null() {
        return Vec::new();
    }
    let f: HcGetBodyFn = std::mem::transmute(sym);
    let mut bytes: *const u8 = std::ptr::null();
    let mut size: u32 = 0;
    if f(call_handle as *mut c_void, &mut bytes, &mut size) != 0 || size == 0 || bytes.is_null() {
        return Vec::new();
    }
    std::slice::from_raw_parts(bytes, size as usize).to_vec()
}

// Deliver the response body back into the HCCallHandle so the game can read it
// via HCHttpCallResponseGetResponseString. This mirrors what the real Java
// NativeOutputStream.nativeWrite does: fetch the call's response-body write
// function and invoke it with the body bytes (HCHttpCallResponseSetResponseBodyBytes
// returns E_FAIL whenever the call uses a custom write function).
type HcGetWriteFn = unsafe extern "C" fn(call: *mut c_void, write_function: *mut *mut c_void, context: *mut *mut c_void) -> i32;
type HcWriteFn = unsafe extern "C" fn(call: *mut c_void, source: *const u8, bytes_available: usize, context: *mut c_void) -> i32;

unsafe fn write_call_response_body(call_handle: i64, body: &[u8]) {
    if body.is_empty() {
        return;
    }
    let get_sym = hc_symbol("HCHttpCallResponseGetResponseBodyWriteFunction");
    if get_sym.is_null() {
        log::warn!("HTTP: HCHttpCallResponseGetResponseBodyWriteFunction not resolvable");
        return;
    }
    let get_fn: HcGetWriteFn = std::mem::transmute(get_sym);
    let mut write_fn: *mut c_void = std::ptr::null_mut();
    let mut write_ctx: *mut c_void = std::ptr::null_mut();
    let hr = get_fn(call_handle as *mut c_void, &mut write_fn, &mut write_ctx);
    if hr != 0 || write_fn.is_null() {
        log::warn!("HTTP: get response write function failed hr={:#x}", hr);
        return;
    }
    let wf: HcWriteFn = std::mem::transmute(write_fn);
    wf(call_handle as *mut c_void, body.as_ptr(), body.len(), write_ctx);
}

// Response state stored per-instance
struct HttpResponseState {
    response_code: i32,
    response_headers: Vec<(String, String)>,
    response_body: Vec<u8>,
    call_handle: i64,
}

static RESPONSE_STATES: OnceLock<Mutex<HashMap<usize, Arc<Mutex<HttpResponseState>>>>> = OnceLock::new();

fn response_states() -> &'static Mutex<HashMap<usize, Arc<Mutex<HttpResponseState>>>> {
    RESPONSE_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

// HTTP request state stored per-instance
struct HttpRequestState {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    call_handle: i64,
    body_length: usize,
}

// Map from jobject to request state
static REQUEST_STATES: OnceLock<Mutex<HashMap<usize, Arc<Mutex<HttpRequestState>>>>> = OnceLock::new();

fn request_states() -> &'static Mutex<HashMap<usize, Arc<Mutex<HttpRequestState>>>> {
    REQUEST_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

// Helper to get JNI vtable
fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

// Helper to create a Java string
fn new_jstring(env: *mut JNIEnv, s: &str) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let new_string = match unsafe { (*iface).NewStringUTF } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let c_str = std::ffi::CString::new(s).unwrap_or_default();
    unsafe { new_string(env, c_str.as_ptr()) as jstring }
}

// Helper to read Java string
fn get_jstring_content(env: *mut JNIEnv, s: jstring) -> Option<String> {
    let iface = get_iface(env);
    if iface.is_null() {
        return None;
    }
    let get_chars = unsafe { (*iface).GetStringUTFChars }?;
    let release = unsafe { (*iface).ReleaseStringUTFChars };
    let c_str = unsafe { get_chars(env, s, std::ptr::null_mut()) };
    if c_str.is_null() {
        return None;
    }
    let result = Some(unsafe {
        std::ffi::CStr::from_ptr(c_str)
            .to_string_lossy()
            .into_owned()
    });
    if let Some(f) = release {
        unsafe { f(env, s, c_str) };
    }
    result
}

// Helper to read byte array
fn new_byte_array(env: *mut JNIEnv, data: &[u8]) -> jbyteArray {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let new_array = match unsafe { (*iface).NewByteArray } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let arr = unsafe { new_array(env, data.len() as i32) };
    if !arr.is_null() {
        if let Some(set_region) = unsafe { (*iface).SetByteArrayRegion } {
            unsafe { set_region(env, arr, 0, data.len() as i32, data.as_ptr() as *const i8) };
        }
    }
    arr
}

// Helper to call void method on object
fn call_void_method(env: *mut JNIEnv, obj: jobject, name: &str, sig: &str, args: &mut [jvalue]) {
    let iface = get_iface(env);
    if iface.is_null() {
        return;
    }
    let get_class = match unsafe { (*iface).GetObjectClass } {
        Some(f) => f,
        None => return,
    };
    let get_mid = match unsafe { (*iface).GetMethodID } {
        Some(f) => f,
        None => return,
    };
    let call = match unsafe { (*iface).CallVoidMethodA } {
        Some(f) => f,
        None => return,
    };

    let cls = unsafe { get_class(env, obj) };
    let name_c = std::ffi::CString::new(name).unwrap_or_default();
    let sig_c = std::ffi::CString::new(sig).unwrap_or_default();
    let mid = unsafe { get_mid(env, cls, name_c.as_ptr(), sig_c.as_ptr()) };
    if !mid.is_null() {
        unsafe { call(env, obj, mid, args.as_mut_ptr()) };
    }
}

// com/xbox/httpclient/HttpClientRequest constructor
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_init(
    _env: *mut JNIEnv,
    self_: jobject,
) {
    let state = Arc::new(Mutex::new(HttpRequestState {
        url: String::new(),
        method: String::new(),
        headers: Vec::new(),
        body: Vec::new(),
        call_handle: 0,
        body_length: 0,
    }));

    let key = self_ as usize;
    if let Ok(mut states) = request_states().lock() {
        states.insert(key, state);
    }
}

// com/xbox/httpclient/HttpClientRequest destructor
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_destroy(
    _env: *mut JNIEnv,
    self_: jobject,
) {
    let key = self_ as usize;
    if let Ok(mut states) = request_states().lock() {
        states.remove(&key);
    }
}

// com/xbox/httpclient/HttpClientRequest.isNetworkAvailable(Landroid/content/Context;)Z
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_isNetworkAvailable(
    _env: *mut JNIEnv,
    _self: jobject,
    _context: jobject,
) -> jboolean {
    1 // Always return true
}

// com/xbox/httpclient/HttpClientRequest.createClientRequest()Lcom/xbox/httpclient/HttpClientRequest;
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_createClientRequest(
    env: *mut JNIEnv,
    _self: jobject,
) -> jobject {
    // Create a new HttpClientRequest instance via JNI NewObject
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }

    let find_class = match unsafe { (*iface).FindClass } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let get_method_id = match unsafe { (*iface).GetMethodID } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let new_object = match unsafe { (*iface).NewObject } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };

    let cls = find_class(env, b"com/xbox/httpclient/HttpClientRequest\0".as_ptr() as *const c_char);
    if cls.is_null() {
        return std::ptr::null_mut();
    }

    let init_mid = get_method_id(
        env,
        cls,
        b"<init>\0".as_ptr() as *const c_char,
        b"()V\0".as_ptr() as *const c_char,
    );

    if init_mid.is_null() {
        return std::ptr::null_mut();
    }

    new_object(env, cls, init_mid)
}

// com/xbox/httpclient/HttpClientRequest.setHttpUrl(Ljava/lang/String;)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_setHttpUrl(
    env: *mut JNIEnv,
    self_: jobject,
    url: jstring,
) {
    let url_str = match get_jstring_content(env, url) {
        Some(s) => s,
        None => return,
    };

    let key = self_ as usize;
    if let Ok(states) = request_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(mut s) = state.lock() {
                s.url = url_str;
            }
        }
    }
}

// com/xbox/httpclient/HttpClientRequest.setHttpMethodAndBody(Ljava/lang/String;JLjava/lang/String;J)V
// (method jstring, call jlong = HCCallHandle, contentType jstring, contentLength jlong)
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_setHttpMethodAndBody(
    env: *mut JNIEnv,
    self_: jobject,
    method: jstring,
    call_handle: jlong,
    _content_type: jstring,
    body_length: jlong,
) {
    let method_str = match get_jstring_content(env, method) {
        Some(s) => s,
        None => return,
    };

    let key = self_ as usize;
    if let Ok(states) = request_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(mut s) = state.lock() {
                if !method_str.is_empty() {
                    s.method = method_str;
                }
                s.call_handle = call_handle;
                s.body_length = body_length.max(0) as usize;
            }
        }
    }
}

// com/xbox/httpclient/HttpClientRequest.setHttpHeader(Ljava/lang/String;Ljava/lang/String;)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_setHttpHeader(
    env: *mut JNIEnv,
    self_: jobject,
    name: jstring,
    value: jstring,
) {
    let name_str = match get_jstring_content(env, name) {
        Some(s) => s,
        None => return,
    };
    let value_str = match get_jstring_content(env, value) {
        Some(s) => s,
        None => return,
    };

    let key = self_ as usize;
    if let Ok(states) = request_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(mut s) = state.lock() {
                s.headers.push((name_str, value_str));
            }
        }
    }
}

// com/xbox/httpclient/HttpClientRequest.doRequestAsync(J)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_doRequestAsync(
    _env: *mut JNIEnv,
    self_: jobject,
    source_call: jlong,
) {
    let key = self_ as usize;
    let state = if let Ok(states) = request_states().lock() {
        states.get(&key).cloned()
    } else {
        None
    };

    let state = match state {
        Some(s) => s,
        None => return,
    };

    let call_handle = {
        if let Ok(mut s) = state.lock() {
            if s.call_handle != 0 {
                s.call_handle
            } else {
                s.call_handle = source_call;
                source_call
            }
        } else {
            source_call
        }
    };

    // The request body (if any) lives on the HCCallHandle in libHttpClient; the
    // game registered it via HCHttpCallRequestSetRequestBodyBytes before firing
    // doRequestAsync. Read it on the game thread while the call is still owned
    // by the game, then hand it to the worker thread.
    let prefetch_body = {
        if let Ok(s) = state.lock() {
            s.body_length
        } else {
            0
        }
    };
    let body = if prefetch_body > 0 {
        unsafe { read_call_request_body(call_handle) }
    } else {
        Vec::new()
    };
    if let Ok(mut s) = state.lock() {
        s.body = body.clone();
    }

    let thread_state = state.clone();
    let self_ptr = self_ as usize;

    std::thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to create HTTP client: {}", e);
                return;
            }
        };

        let (url, method, headers, body) = {
            match thread_state.lock() {
                Ok(s) => (
                    s.url.clone(),
                    s.method.clone(),
                    s.headers.clone(),
                    s.body.clone(),
                ),
                Err(_) => return,
            }
        };

        let method_string = method.to_uppercase();
        let method = match method_string.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "HEAD" => reqwest::Method::HEAD,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            _ => reqwest::Method::GET,
        };
        let needs_zero_length = body.is_empty()
            && matches!(method, reqwest::Method::POST | reqwest::Method::PUT | reqwest::Method::PATCH);

        let mut req = client.request(method, &url);
        if needs_zero_length {
            req = req.header("Content-Length", "0");
        }
        for (name, value) in &headers {
            req = req.header(name.as_str(), value.as_str());
        }
        if !body.is_empty() {
            req = req.body(body);
        }

        let before = std::time::Instant::now();
        let result = req.send();

        // Get JNI env for this thread
        let vm = jnivm_create_vm();
        let env = jnivm_get_env(vm);
        if env.is_null() {
            log::error!("HTTP: failed to get JNI env in background thread");
            return;
        }

        match result {
            Ok(response) => {
                let status = response.status().as_u16() as i32;
                let resp_headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let resp_body = response.bytes().unwrap_or_default().to_vec();

                let resp_obj = create_response_object(env, status, resp_headers, resp_body, call_handle);
                if resp_obj.is_null() {
                    log::error!("HTTP: failed to create HttpClientResponse object for {} {}", method_string, url);
                    return;
                }

                let mut args = [
                    jvalue { j: source_call },
                    jvalue { l: resp_obj },
                ];
                call_void_method(env, self_ptr as jobject, "OnRequestCompleted",
                    "(JLcom/xbox/httpclient/HttpClientResponse;)V", &mut args);
                log::info!("HTTP request: {} {} -> {} ({}ms)", method_string, url, status, before.elapsed().as_millis());
            }
            Err(e) => {
                log::error!("HTTP request failed: {} {} {}", method_string, url, e);
                call_request_failed_direct(env, source_call, &e.to_string());
            }
        }
    });
}

// Call libHttpClient's Java_com_xbox_httpclient_HttpClientRequest_OnRequestFailed
// native directly with the full 5-arg signature the game's DEX declares
// (call, errorMessage, stackTrace, networkDetails, isNoNetwork). Going straight
// to the exported symbol avoids the VM's 4-jvalue CallVoidMethodA limit.
unsafe fn call_request_failed_direct(env: *mut JNIEnv, call: jlong, message: &str) {
    let sym = hc_symbol("Java_com_xbox_httpclient_HttpClientRequest_OnRequestFailed");
    if sym.is_null() {
        return;
    }
    let err_str = new_jstring(env, message);
    let empty = new_jstring(env, "");
    if err_str.is_null() {
        return;
    }
    let f: unsafe extern "C" fn(
        *mut JNIEnv,
        jobject,
        jlong,
        jstring,
        jstring,
        jstring,
        jboolean,
    ) = std::mem::transmute(sym);
    f(env, std::ptr::null_mut(), call, err_str, empty, empty, 0);
}

unsafe fn create_response_object(env: *mut JNIEnv, status: i32, headers: Vec<(String, String)>, body: Vec<u8>, call_handle: i64) -> jobject {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }

    let find_class = match (*iface).FindClass { Some(f) => f, None => return std::ptr::null_mut() };
    let get_mid = match (*iface).GetMethodID { Some(f) => f, None => return std::ptr::null_mut() };
    let new_obj = match (*iface).NewObject { Some(f) => f, None => return std::ptr::null_mut() };

    let cls = find_class(env, b"com/xbox/httpclient/HttpClientResponse\0".as_ptr() as *const c_char);
    if cls.is_null() { return std::ptr::null_mut(); }

    let init_mid = get_mid(env, cls,
        b"<init>\0".as_ptr() as *const c_char,
        b"()V\0".as_ptr() as *const c_char);
    if init_mid.is_null() { return std::ptr::null_mut(); }

    let obj = new_obj(env, cls, init_mid);
    if obj.is_null() { return std::ptr::null_mut(); }

    // Store response data for the new object
    let resp_key = obj as usize;
    let resp_state = Arc::new(Mutex::new(HttpResponseState {
        response_code: status,
        response_headers: headers,
        response_body: body,
        call_handle,
    }));

    if let Ok(mut states) = response_states().lock() {
        states.insert(resp_key, resp_state);
    }

    obj
}

// com/xbox/httpclient/HttpClientResponse.getNumHeaders()I
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getNumHeaders(
    _env: *mut JNIEnv,
    self_: jobject,
) -> jint {
    let key = self_ as usize;
    if let Ok(states) = response_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                return s.response_headers.len() as jint;
            }
        }
    }
    0
}

// com/xbox/httpclient/HttpClientResponse.getHeaderNameAtIndex(I)Ljava/lang/String;
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getHeaderNameAtIndex(
    env: *mut JNIEnv,
    self_: jobject,
    index: jint,
) -> jstring {
    let key = self_ as usize;
    if let Ok(states) = response_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                if let Some((name, _)) = s.response_headers.get(index as usize) {
                    return new_jstring(env, name);
                }
            }
        }
    }
    new_jstring(env, "")
}

// com/xbox/httpclient/HttpClientResponse.getHeaderValueAtIndex(I)Ljava/lang/String;
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getHeaderValueAtIndex(
    env: *mut JNIEnv,
    self_: jobject,
    index: jint,
) -> jstring {
    let key = self_ as usize;
    if let Ok(states) = response_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                if let Some((_, value)) = s.response_headers.get(index as usize) {
                    return new_jstring(env, value);
                }
            }
        }
    }
    new_jstring(env, "")
}

// com/xbox/httpclient/HttpClientResponse.getResponseBodyBytes()[B
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getResponseBodyBytes(
    env: *mut JNIEnv,
    self_: jobject,
) -> jbyteArray {
    let key = self_ as usize;
    if let Ok(states) = response_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                return new_byte_array(env, &s.response_body);
            }
        }
    }
    new_byte_array(env, &[])
}

// com/xbox/httpclient/HttpClientResponse.getResponseCode()I
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getResponseCode(
    _env: *mut JNIEnv,
    self_: jobject,
) -> jint {
    let key = self_ as usize;
    if let Ok(states) = response_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                return s.response_code;
            }
        }
    }
    0
}

// com/xbox/httpclient/HttpClientResponse.getResponseBodyBytes()V
// The game (libHttpClient android_http_request.cpp ProcessResponseBody) invokes
// this via CallVoidMethod; we deliver the captured response body back onto the
// HCCallHandle so HCHttpCallResponseGetResponseString sees it.
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getResponseBodyBytesVoid(
    _env: *mut JNIEnv,
    self_: jobject,
) {
    let key = self_ as usize;
    let (call_handle, body) = if let Ok(states) = response_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                (s.call_handle, s.response_body.clone())
            } else {
                return;
            }
        } else {
            return;
        }
    } else {
        return;
    };
    write_call_response_body(call_handle, &body);
}

// Clean up response state when object is destroyed
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_destroy(
    _env: *mut JNIEnv,
    self_: jobject,
) {
    let key = self_ as usize;
    if let Ok(mut states) = response_states().lock() {
        states.remove(&key);
    }
}

// Register native methods with libjnivm-sys
pub fn register(env: *mut JNIEnv) {
    let request_methods = [
        JNINativeMethod {
            name: b"<init>\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_init as *mut c_void,
        },
        JNINativeMethod {
            name: b"<init>\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_init as *mut c_void,
        },
        JNINativeMethod {
            name: b"destroy\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_destroy as *mut c_void,
        },
        JNINativeMethod {
            name: b"isNetworkAvailable\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;)Z\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_isNetworkAvailable as *mut c_void,
        },
        JNINativeMethod {
            name: b"createClientRequest\0".as_ptr() as *const c_char,
            signature: b"()Lcom/xbox/httpclient/HttpClientRequest;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_createClientRequest as *mut c_void,
        },
        JNINativeMethod {
            name: b"setHttpUrl\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_setHttpUrl as *mut c_void,
        },
        JNINativeMethod {
            name: b"setHttpMethodAndBody\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;JLjava/lang/String;J)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_setHttpMethodAndBody as *mut c_void,
        },
        JNINativeMethod {
            name: b"setHttpHeader\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_setHttpHeader as *mut c_void,
        },
        JNINativeMethod {
            name: b"doRequestAsync\0".as_ptr() as *const c_char,
            signature: b"(J)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientRequest_doRequestAsync as *mut c_void,
        },
    ];

    let response_methods = [
        JNINativeMethod {
            name: b"getNumHeaders\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getNumHeaders as *mut c_void,
        },
        JNINativeMethod {
            name: b"getHeaderNameAtIndex\0".as_ptr() as *const c_char,
            signature: b"(I)Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getHeaderNameAtIndex as *mut c_void,
        },
        JNINativeMethod {
            name: b"getHeaderValueAtIndex\0".as_ptr() as *const c_char,
            signature: b"(I)Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getHeaderValueAtIndex as *mut c_void,
        },
        JNINativeMethod {
            name: b"getResponseBodyBytes\0".as_ptr() as *const c_char,
            signature: b"()[B\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getResponseBodyBytes as *mut c_void,
        },
        JNINativeMethod {
            name: b"getResponseBodyBytes\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getResponseBodyBytesVoid as *mut c_void,
        },
        JNINativeMethod {
            name: b"getResponseCode\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getResponseCode as *mut c_void,
        },
        JNINativeMethod {
            name: b"destroy\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_destroy as *mut c_void,
        },
    ];

    // Register HttpClientRequest
    let request_cls = unsafe {
        jnivm_find_class(
            env,
            b"com/xbox/httpclient/HttpClientRequest\0".as_ptr() as *const c_char,
        )
    };
    if !request_cls.is_null() {
        unsafe {
            jnivm_register_natives(
                env,
                request_cls,
                request_methods.as_ptr(),
                request_methods.len() as i32,
            );
        }
        log::info!("Registered HttpClientRequest native methods");
    } else {
        log::warn!("Could not find HttpClientRequest class");
    }

    // Register HttpClientResponse
    let response_cls = unsafe {
        jnivm_find_class(
            env,
            b"com/xbox/httpclient/HttpClientResponse\0".as_ptr() as *const c_char,
        )
    };
    if !response_cls.is_null() {
        unsafe {
            jnivm_register_natives(
                env,
                response_cls,
                response_methods.as_ptr(),
                response_methods.len() as i32,
            );
        }
        log::info!("Registered HttpClientResponse native methods");
    } else {
        log::warn!("Could not find HttpClientResponse class");
    }
}
