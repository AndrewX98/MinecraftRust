use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;

use libjnivm_sys::*;

// WebSocket state stored per-instance
struct WebSocketState {
    url: String,
    ws_protocol: String,
    headers: Vec<(String, String)>,
    connected: bool,
    call_handle: i64,
}

// Map from jobject to WebSocket state
static WS_STATES: OnceLock<Mutex<HashMap<usize, Arc<Mutex<WebSocketState>>>>> = OnceLock::new();

fn ws_states() -> &'static Mutex<HashMap<usize, Arc<Mutex<WebSocketState>>>> {
    WS_STATES.get_or_init(|| Mutex::new(HashMap::new()))
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

// com/xbox/httpclient/HttpClientWebSocket constructor
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_init(
    _env: *mut JNIEnv,
    self_: jobject,
    owner: jlong,
) {
    let state = Arc::new(Mutex::new(WebSocketState {
        url: String::new(),
        ws_protocol: String::new(),
        headers: Vec::new(),
        connected: false,
        call_handle: owner,
    }));

    let key = self_ as usize;
    if let Ok(mut states) = ws_states().lock() {
        states.insert(key, state);
    }
}

// com/xbox/httpclient/HttpClientWebSocket destructor
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_destroy(
    _env: *mut JNIEnv,
    self_: jobject,
) {
    let key = self_ as usize;
    if let Ok(mut states) = ws_states().lock() {
        states.remove(&key);
    }
}

// com/xbox/httpclient/HttpClientWebSocket.connect(Ljava/lang/String;Ljava/lang/String;)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_connect(
    env: *mut JNIEnv,
    self_: jobject,
    url: jstring,
    wst: jstring,
) {
    let url_str = match get_jstring_content(env, url) {
        Some(s) => s,
        None => return,
    };
    let wst_str = match get_jstring_content(env, wst) {
        Some(s) => s,
        None => return,
    };

    let key = self_ as usize;
    let state = if let Ok(states) = ws_states().lock() {
        states.get(&key).cloned()
    } else {
        None
    };

    let state = match state {
        Some(s) => s,
        None => return,
    };

    // Update state
    {
        if let Ok(mut s) = state.lock() {
            s.url = url_str.clone();
            s.ws_protocol = wst_str;
        }
    }

    let self_ptr = self_ as usize;
    let thread_state = state.clone();

    std::thread::spawn(move || {
        // Use tungstenite for WebSocket connection
        let url_str = {
            match thread_state.lock() {
                Ok(s) => s.url.clone(),
                Err(_) => return,
            }
        };

        // Parse URL
        let url = match url::Url::parse(&url_str) {
            Ok(u) => u,
            Err(e) => {
                log::error!("WebSocket URL parse error: {}", e);
                // Call onFailure
                return;
            }
        };

        // Connect
        let (ws_stream, _response) = match tungstenite::connect(url) {
            Ok(r) => r,
            Err(e) => {
                log::error!("WebSocket connection error: {}", e);
                // Call onFailure
                return;
            }
        };

        log::info!("WebSocket connected");

        // Mark as connected
        {
            if let Ok(mut s) = thread_state.lock() {
                s.connected = true;
            }
        }

        // TODO: Call onOpen callback

        // Read messages in a loop
        use tungstenite::Message;
        let mut ws_stream = ws_stream;

        loop {
            let connected = match thread_state.lock() {
                Ok(s) => s.connected,
                Err(_) => false,
            };
            if !connected {
                break;
            }

            // Read next message
            match ws_stream.read() {
                Ok(Message::Text(text)) => {
                    log::debug!("WebSocket got text: {}", text);
                    // TODO: Call onMessage callback
                }
                Ok(Message::Binary(data)) => {
                    log::debug!("WebSocket got binary: {} bytes", data.len());
                    // TODO: Call onBinaryMessage callback
                }
                Ok(Message::Close(_)) => {
                    log::info!("WebSocket closed by server");
                    break;
                }
                Ok(Message::Ping(_)) => {
                    // Pong is handled automatically by tungstenite
                }
                Ok(_) => {}
                Err(e) => {
                    log::error!("WebSocket read error: {}", e);
                    break;
                }
            }
        }

        log::info!("WebSocket disconnected");
    });
}

// com/xbox/httpclient/HttpClientWebSocket.addHeader(Ljava/lang/String;Ljava/lang/String;)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_addHeader(
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
    if let Ok(states) = ws_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(mut s) = state.lock() {
                s.headers.push((name_str, value_str));
            }
        }
    }
}

// com/xbox/httpclient/HttpClientWebSocket.sendMessage(Ljava/lang/String;)Z
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_sendMessage(
    _env: *mut JNIEnv,
    self_: jobject,
    _msg: jstring,
) -> jboolean {
    let key = self_ as usize;
    if let Ok(states) = ws_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                if !s.connected {
                    return 0;
                }
                // TODO: Send via tungstenite
            }
        }
    }
    1
}

// com/xbox/httpclient/HttpClientWebSocket.sendBinaryMessage(Ljava/nio/ByteBuffer;)Z
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_sendBinaryMessage(
    _env: *mut JNIEnv,
    self_: jobject,
    _msg: jobject,
) -> jboolean {
    let key = self_ as usize;
    if let Ok(states) = ws_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(s) = state.lock() {
                if !s.connected {
                    return 0;
                }
                // TODO: Send via tungstenite
            }
        }
    }
    1
}

// com/xbox/httpclient/HttpClientWebSocket.disconnect(I)V
#[no_mangle]
pub unsafe extern "C" fn Java_com_xbox_httpclient_HttpClientWebSocket_disconnect(
    _env: *mut JNIEnv,
    self_: jobject,
    _id: jint,
) {
    let key = self_ as usize;
    if let Ok(states) = ws_states().lock() {
        if let Some(state) = states.get(&key) {
            if let Ok(mut s) = state.lock() {
                s.connected = false;
            }
        }
    }
    // TODO: Close the WebSocket connection
}

// Register native methods with libjnivm-sys
pub fn register(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"<init>\0".as_ptr() as *const c_char,
            signature: b"(J)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_init as *mut c_void,
        },
        JNINativeMethod {
            name: b"destroy\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_destroy as *mut c_void,
        },
        JNINativeMethod {
            name: b"connect\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_connect as *mut c_void,
        },
        JNINativeMethod {
            name: b"addHeader\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_addHeader as *mut c_void,
        },
        JNINativeMethod {
            name: b"sendMessage\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)Z\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_sendMessage as *mut c_void,
        },
        JNINativeMethod {
            name: b"sendBinaryMessage\0".as_ptr() as *const c_char,
            signature: b"(Ljava/nio/ByteBuffer;)Z\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_sendBinaryMessage as *mut c_void,
        },
        JNINativeMethod {
            name: b"disconnect\0".as_ptr() as *const c_char,
            signature: b"(I)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_xbox_httpclient_HttpClientWebSocket_disconnect as *mut c_void,
        },
    ];

    let cls = unsafe {
        jnivm_find_class(
            env,
            b"com/xbox/httpclient/HttpClientWebSocket\0".as_ptr() as *const c_char,
        )
    };
    if cls.is_null() {
        log::warn!("Could not find HttpClientWebSocket class");
        return;
    }
    unsafe {
        jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32);
    }
    log::info!("Registered HttpClientWebSocket native methods");
}
