use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;

use libjnivm_sys::*;

// HTTP request state stored per-instance
struct HttpRequestState {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    response_body: Vec<u8>,
    response_headers: Vec<(String, String)>,
    response_code: i32,
    call_handle: i64,
    input_stream_handle: i64,
    output_stream_handle: i64,
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
fn get_byte_array_elements(env: *mut JNIEnv, arr: jbyteArray) -> Option<*const u8> {
    let iface = get_iface(env);
    if iface.is_null() {
        return None;
    }
    let get_bytes = unsafe { (*iface).GetByteArrayElements }?;
    let ptr = unsafe { get_bytes(env, arr, std::ptr::null_mut()) };
    if ptr.is_null() {
        None
    } else {
        Some(ptr as *const u8)
    }
}

// Helper to release byte array
fn release_byte_array_elements(env: *mut JNIEnv, arr: jbyteArray, ptr: *const u8) {
    let iface = get_iface(env);
    if iface.is_null() {
        return;
    }
    if let Some(release) = unsafe { (*iface).ReleaseByteArrayElements } {
        unsafe { release(env, arr, ptr as *mut i8, 0) };
    }
}

// Helper to get array length
fn get_array_length(env: *mut JNIEnv, arr: jarray) -> Option<jint> {
    let iface = get_iface(env);
    if iface.is_null() {
        return None;
    }
    let get_len = unsafe { (*iface).GetArrayLength }?;
    Some(unsafe { get_len(env, arr) })
}

// Helper to create byte array
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
    env: *mut JNIEnv,
    self_: jobject,
) {
    let state = Arc::new(Mutex::new(HttpRequestState {
        url: String::new(),
        method: String::new(),
        headers: Vec::new(),
        body: Vec::new(),
        response_body: Vec::new(),
        response_headers: Vec::new(),
        response_code: 0,
        call_handle: 0,
        input_stream_handle: 0,
        output_stream_handle: 0,
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

// com/xbox/httpclient/HttpClientRequest.setHttpMethodAndBody(Ljava/lang/String;Ljava/lang/String;[B)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientRequest_setHttpMethodAndBody(
    env: *mut JNIEnv,
    self_: jobject,
    method: jstring,
    _content_type: jstring,
    body: jbyteArray,
) {
    let method_str = match get_jstring_content(env, method) {
        Some(s) => s,
        None => return,
    };

    let body_data = if !body.is_null() {
        let ptr = get_byte_array_elements(env, body);
        let len = get_array_length(env, body as jarray).unwrap_or(0) as usize;
        if let Some(p) = ptr {
            let data = std::slice::from_raw_parts(p, len).to_vec();
            release_byte_array_elements(env, body, p);
            data
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let key = self_ as usize;
    if let Ok(states) = request_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(mut s) = state.lock() {
                s.method = method_str;
                s.body = body_data;
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

    // Clone state for the thread
    let thread_state = state.clone();
    let self_ptr = self_ as usize;

    std::thread::spawn(move || {
        // Build the request
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

        let method = match method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "HEAD" => reqwest::Method::HEAD,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            _ => reqwest::Method::GET,
        };

        let mut req = client.request(method, &url);

        for (name, value) in &headers {
            req = req.header(name.as_str(), value.as_str());
        }

        if !body.is_empty() {
            req = req.body(body);
        }

        // Execute the request
        let result = req.send();

        match result {
            Ok(response) => {
                let status = response.status().as_u16() as i32;
                let resp_headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                let resp_body = response.bytes().map(|b| b.to_vec()).unwrap_or_default();

                // Store response in state
                if let Ok(mut s) = thread_state.lock() {
                    s.response_code = status;
                    s.response_headers = resp_headers;
                    s.response_body = resp_body;
                }

                // TODO: Call OnRequestCompleted callback
                log::info!("HTTP request completed: {}", status);
            }
            Err(e) => {
                log::error!("HTTP request failed: {}", e);
                // TODO: Call OnRequestFailed callback
            }
        }
    });
}

// com/xbox/httpclient/HttpClientResponse.getNumHeaders()I
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getNumHeaders(
    _env: *mut JNIEnv,
    self_: jobject,
) -> jint {
    // Response headers are stored in the request state
    // For now, return 0
    0
}

// com/xbox/httpclient/HttpClientResponse.getHeaderNameAtIndex(I)Ljava/lang/String;
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getHeaderNameAtIndex(
    env: *mut JNIEnv,
    _self: jobject,
    _index: jint,
) -> jstring {
    new_jstring(env, "")
}

// com/xbox/httpclient/HttpClientResponse.getHeaderValueAtIndex(I)Ljava/lang/String;
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getHeaderValueAtIndex(
    env: *mut JNIEnv,
    _self: jobject,
    _index: jint,
) -> jstring {
    new_jstring(env, "")
}

// com/xbox/httpclient/HttpClientResponse.getResponseBodyBytes()[B
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getResponseBodyBytes(
    env: *mut JNIEnv,
    _self: jobject,
) -> jbyteArray {
    // For now, return empty array
    new_byte_array(env, &[])
}

// com/xbox/httpclient/HttpClientResponse.getResponseCode()I
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientResponse_getResponseCode(
    _env: *mut JNIEnv,
    _self: jobject,
) -> jint {
    0
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
            signature: b"(Ljava/lang/String;Ljava/lang/String;[B)V\0".as_ptr() as *const c_char,
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
            name: b"getResponseCode\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientResponse_getResponseCode as *mut c_void,
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
