use std::ffi::{c_char, c_void, CStr, CString};

type TextCallback = unsafe extern "C" fn(*mut c_void, *const c_char);
type CaretCallback = unsafe extern "C" fn(*mut c_void, i32);

fn is_word_separator(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '!'
        | '"' | '#' | '$' | '%' | '&' | '\'' | '(' | ')'
        | '*' | '+' | ',' | '-' | '.' | '/'
        | ':' | ';' | '<' | '=' | '>' | '?' | '@'
        | '[' | '\\' | ']' | '^' | '`' | '{' | '|' | '}' | '~')
}

pub struct TextInputHandler {
    enabled: bool,
    multiline: bool,
    alt_pressed: bool,
    shift_pressed: bool,
    current_text: String,
    current_text_position: usize,
    current_text_position_utf: usize,
    current_text_copy_position: usize,
    current_text_copy_position_utf: usize,
    enabled_no: usize,
    keep_once: bool,
    last_input: String,
    callback_ctx: *mut c_void,
    text_callback: Option<TextCallback>,
    caret_callback: Option<CaretCallback>,
}

impl TextInputHandler {
    pub fn new() -> Self {
        Self {
            enabled: false,
            multiline: false,
            alt_pressed: false,
            shift_pressed: false,
            current_text: String::new(),
            current_text_position: 0,
            current_text_position_utf: 0,
            current_text_copy_position: 0,
            current_text_copy_position_utf: 0,
            enabled_no: 0,
            keep_once: false,
            last_input: String::new(),
            callback_ctx: std::ptr::null_mut(),
            text_callback: None,
            caret_callback: None,
        }
    }

    pub fn set_callbacks(&mut self, ctx: *mut c_void, text_cb: TextCallback, caret_cb: CaretCallback) {
        self.callback_ctx = ctx;
        self.text_callback = Some(text_cb);
        self.caret_callback = Some(caret_cb);
    }

    fn notify_text(&self) {
        if let Some(cb) = self.text_callback {
            let cstr = CString::new(self.current_text.as_str()).unwrap_or_default();
            unsafe { cb(self.callback_ctx, cstr.as_ptr()); }
        }
    }

    fn notify_caret(&self) {
        if let Some(cb) = self.caret_callback {
            unsafe { cb(self.callback_ctx, self.current_text_position_utf as i32); }
        }
    }

    fn utf8_count(s: &str) -> usize {
        s.chars().count()
    }

    fn utf8_byte_pos(s: &str, utf_pos: usize) -> usize {
        s.chars().take(utf_pos).map(|c| c.len_utf8()).sum()
    }

    fn utf8_char_len(s: &str, byte_pos: usize) -> usize {
        if byte_pos >= s.len() { return 0; }
        s[byte_pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(0)
    }

    fn prev_word_boundary(s: &str, byte_pos: usize) -> usize {
        let mut pos = byte_pos;
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        // Skip trailing separators
        while pos > 0 {
            let ci = chars.iter().rposition(|(i, _)| *i < pos).unwrap_or(0);
            if ci > 0 && is_word_separator(chars[ci].1) {
                pos = chars[ci].0;
            } else {
                break;
            }
        }
        // Find start of word
        while pos > 0 {
            let ci = chars.iter().rposition(|(i, _)| *i < pos).unwrap_or(0);
            if !is_word_separator(chars[ci].1) {
                pos = chars[ci].0;
            } else {
                break;
            }
        }
        if pos < byte_pos && !s[pos..].chars().next().map_or(false, |c| is_word_separator(c)) {
            pos
        } else {
            byte_pos
        }
    }

    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn is_multiline(&self) -> bool { self.multiline }
    pub fn get_text(&self) -> &str { &self.current_text }
    pub fn get_cursor_position(&self) -> i32 { self.current_text_position_utf as i32 }
    pub fn get_copy_position(&self) -> i32 { self.current_text_copy_position_utf as i32 }

    pub fn enable(&mut self, text: String, multiline: bool) {
        self.enabled = true;
        self.multiline = multiline;
        self.enabled_no += 1;
        if self.keep_once {
            if let Some(last_char) = self.current_text.chars().last() {
                let mut new_text = text;
                new_text.push(last_char);
                self.current_text = new_text;
            } else {
                self.current_text = text;
            }
            self.keep_once = false;
        } else {
            self.current_text = text;
        }
        self.update_text_state();
    }

    pub fn update(&mut self, text: String) {
        self.current_text = text;
        self.update_text_state();
    }

    fn update_text_state(&mut self) {
        self.current_text_position = self.current_text.len();
        self.current_text_position_utf = Self::utf8_count(&self.current_text);
        self.current_text_copy_position = self.current_text.len();
        self.current_text_copy_position_utf = Self::utf8_count(&self.current_text);
        self.notify_text();
        self.notify_caret();
    }

    pub fn disable(&mut self) {
        if !self.keep_once {
            self.current_text.clear();
            self.last_input.clear();
        }
        self.current_text_position = 0;
        self.current_text_position_utf = 0;
        self.current_text_copy_position = 0;
        self.current_text_copy_position_utf = 0;
        self.enabled = false;
    }

    pub fn on_text_input(&mut self, val: &str) {
        if !self.enabled {
            if let Some(cb) = self.text_callback {
                let cstr = CString::new(val).unwrap_or_default();
                unsafe { cb(self.callback_ctx, cstr.as_ptr()); }
            }
            return;
        }

        if val == "\x08" {
            if self.alt_pressed && self.current_text_position > 0 {
                let new_pos = Self::prev_word_boundary(&self.current_text, self.current_text_position);
                if new_pos < self.current_text_position {
                    self.current_text.drain(new_pos..self.current_text_position);
                    self.current_text_position = new_pos;
                    self.current_text_position_utf = Self::utf8_count(&self.current_text[..new_pos]);
                }
            } else if self.current_text_position > 0 {
                let char_len = Self::utf8_char_len(&self.current_text, self.current_text_position - 1);
                if char_len > 0 && self.current_text_position >= char_len {
                    let prev = self.current_text_position - char_len;
                    self.current_text.drain(prev..self.current_text_position);
                    self.current_text_position = prev;
                    self.current_text_position_utf = self.current_text_position_utf.saturating_sub(1);
                }
            }
        } else if val == "\x7f" {
            if self.current_text_position < self.current_text.len() {
                let char_len = Self::utf8_char_len(&self.current_text, self.current_text_position);
                self.current_text.drain(self.current_text_position..self.current_text_position + char_len);
            }
        } else {
            self.current_text.insert_str(self.current_text_position, val);
            self.current_text_position += val.len();
            self.current_text_position_utf = Self::utf8_count(&self.current_text[..self.current_text_position]);
        }

        if self.current_text_copy_position != self.current_text_position {
            self.current_text_copy_position = self.current_text_position;
            self.current_text_copy_position_utf = self.current_text_position_utf;
        }

        self.notify_text();
        self.notify_caret();
    }

    pub fn on_key_pressed(&mut self, key: i32, action: i32, mods: i32) {
        if action == 1 { return; } // RELEASE

        let ctrl = (mods & 2) != 0; // KEY_MOD_CTRL
        if ctrl {
            return;
        }

        self.shift_pressed = (mods & 1) != 0; // KEY_MOD_SHIFT
        self.alt_pressed = (mods & 8) != 0; // KEY_MOD_ALT

        match key {
            37 ..= 40 => { // LEFT, UP, RIGHT, DOWN
                let is_right = key == 39;
                if self.shift_pressed {
                    if is_right && self.current_text_position < self.current_text.len() {
                        let char_len = Self::utf8_char_len(&self.current_text, self.current_text_position);
                        self.current_text_position += char_len;
                        self.current_text_position_utf += 1;
                    } else if !is_right && self.current_text_position > 0 {
                        let char_len = Self::utf8_char_len(&self.current_text, self.current_text_position - 1);
                        self.current_text_position -= char_len;
                        self.current_text_position_utf = self.current_text_position_utf.saturating_sub(1);
                    }
                } else {
                    if self.current_text_copy_position != self.current_text_position {
                        if is_right {
                            self.current_text_position = self.current_text_copy_position.max(self.current_text_position);
                        } else {
                            self.current_text_position = self.current_text_copy_position.min(self.current_text_position);
                        }
                        self.current_text_position_utf = Self::utf8_count(&self.current_text[..self.current_text_position]);
                        self.current_text_copy_position = self.current_text_position;
                        self.current_text_copy_position_utf = self.current_text_position_utf;
                    } else {
                        if is_right && self.current_text_position < self.current_text.len() {
                            let char_len = Self::utf8_char_len(&self.current_text, self.current_text_position);
                            self.current_text_position += char_len;
                            self.current_text_position_utf += 1;
                        } else if !is_right && self.current_text_position > 0 {
                            let char_len = Self::utf8_char_len(&self.current_text, self.current_text_position - 1);
                            self.current_text_position -= char_len;
                            self.current_text_position_utf = self.current_text_position_utf.saturating_sub(1);
                        }
                    }
                }
                self.notify_caret();
            }
            36 => { // HOME
                if self.shift_pressed {
                    self.current_text_copy_position = self.current_text_position;
                    self.current_text_copy_position_utf = self.current_text_position_utf;
                }
                self.current_text_position = 0;
                self.current_text_position_utf = 0;
                if !self.shift_pressed {
                    self.current_text_copy_position = 0;
                    self.current_text_copy_position_utf = 0;
                }
                self.notify_caret();
            }
            35 => { // END
                if self.shift_pressed {
                    self.current_text_copy_position = self.current_text_position;
                    self.current_text_copy_position_utf = self.current_text_position_utf;
                }
                self.current_text_position = self.current_text.len();
                self.current_text_position_utf = Self::utf8_count(&self.current_text);
                if !self.shift_pressed {
                    self.current_text_copy_position = self.current_text_position;
                    self.current_text_copy_position_utf = self.current_text_position_utf;
                }
                self.notify_caret();
            }
            _ => {}
        }
    }

    pub fn get_copy_text(&self) -> String {
        let start = self.current_text_copy_position.min(self.current_text_position);
        let end = self.current_text_copy_position.max(self.current_text_position);
        if start != end {
            self.current_text[start..end].to_string()
        } else {
            self.current_text.clone()
        }
    }

    pub fn set_cursor_position(&mut self, pos: i32) {
        if pos < 0 {
            self.current_text_position = self.current_text.len();
            self.current_text_position_utf = Self::utf8_count(&self.current_text);
        } else {
            let utf_pos = pos as usize;
            self.current_text_position_utf = utf_pos.min(Self::utf8_count(&self.current_text));
            self.current_text_position = Self::utf8_byte_pos(&self.current_text, self.current_text_position_utf);
        }
        if !self.shift_pressed {
            self.current_text_copy_position = self.current_text_position;
            self.current_text_copy_position_utf = self.current_text_position_utf;
        }
        self.notify_caret();
    }

    pub fn set_keep_last_char_once(&mut self) {
        self.keep_once = true;
    }

    pub fn get_keep_last_char_once(&self) -> bool {
        self.keep_once
    }
}

// ================================================================
// extern "C" FFI bridge functions
// ================================================================

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_new() -> *mut c_void {
    Box::into_raw(Box::new(TextInputHandler::new())) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_set_callbacks(
    h: *mut c_void,
    ctx: *mut c_void,
    text_cb: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
    caret_cb: Option<unsafe extern "C" fn(*mut c_void, i32)>,
) {
    if h.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    if let Some(cb) = text_cb {
        if let Some(ccb) = caret_cb {
            handler.set_callbacks(ctx, cb, ccb);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_is_enabled(h: *mut c_void) -> bool {
    if h.is_null() { return false; }
    let handler = &*(h as *mut TextInputHandler);
    handler.is_enabled()
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_is_multiline(h: *mut c_void) -> bool {
    if h.is_null() { return false; }
    let handler = &*(h as *mut TextInputHandler);
    handler.is_multiline()
}

#[no_mangle]
pub unsafe extern "C" fn text_handler_on_text_input(h: *mut c_void, text: *const c_char) {
    if h.is_null() || text.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    let s = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    handler.on_text_input(s);
}

#[no_mangle]
pub unsafe extern "C" fn text_handler_on_key_pressed(h: *mut c_void, key: i32, action: i32, mods: i32) {
    if h.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    handler.on_key_pressed(key, action, mods);
}

#[no_mangle]
pub unsafe extern "C" fn text_handler_get_copy_text(h: *mut c_void, len: *mut usize) -> *const c_char {
    if h.is_null() {
        if !len.is_null() { *len = 0; }
        return std::ptr::null();
    }
    let handler = &*(h as *mut TextInputHandler);
    let text = handler.get_copy_text();
    // Leak the string — caller is responsible (matching C++ behavior)
    let cstr = CString::new(text).unwrap_or_default();
    let ptr = cstr.into_raw();
    if !len.is_null() {
        *len = unsafe { CStr::from_ptr(ptr).to_bytes().len() };
    }
    ptr as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_enable(h: *mut c_void, text: *const c_char, multiline: bool) {
    if h.is_null() || text.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    let s = match CStr::from_ptr(text).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    handler.enable(s, multiline);
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_disable(h: *mut c_void) {
    if h.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    handler.disable();
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_update(h: *mut c_void, text: *const c_char) {
    if h.is_null() || text.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    let s = match CStr::from_ptr(text).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    handler.update(s);
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_set_cursor_position(h: *mut c_void, pos: i32) {
    if h.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    handler.set_cursor_position(pos);
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_get_cursor_position(h: *mut c_void) -> i32 {
    if h.is_null() { return -1; }
    let handler = &*(h as *mut TextInputHandler);
    handler.get_cursor_position()
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_get_copy_position(h: *mut c_void) -> i32 {
    if h.is_null() { return -1; }
    let handler = &*(h as *mut TextInputHandler);
    handler.get_copy_position()
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_get_text(h: *mut c_void) -> *const c_char {
    if h.is_null() { return std::ptr::null(); }
    let handler = &*(h as *mut TextInputHandler);
    let cstr = CString::new(handler.get_text()).unwrap_or_default();
    cstr.into_raw() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_set_keep_last_char_once(h: *mut c_void) {
    if h.is_null() { return; }
    let handler = &mut *(h as *mut TextInputHandler);
    handler.set_keep_last_char_once();
}

#[no_mangle]
pub unsafe extern "C" fn text_input_handler_get_keep_last_char_once(h: *mut c_void) -> bool {
    if h.is_null() { return false; }
    let handler = &*(h as *mut TextInputHandler);
    handler.get_keep_last_char_once()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEXT_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CARET_CALLS: AtomicUsize = AtomicUsize::new(0);
    static LAST_TEXT: Mutex<String> = Mutex::new(String::new());

    unsafe extern "C" fn test_text_cb(_ctx: *mut c_void, text: *const c_char) {
        let s = CStr::from_ptr(text).to_str().unwrap_or("");
        *LAST_TEXT.lock().unwrap() = s.to_string();
        TEXT_CALLS.fetch_add(1, Ordering::SeqCst);
    }

    unsafe extern "C" fn test_caret_cb(_ctx: *mut c_void, _pos: i32) {
        CARET_CALLS.fetch_add(1, Ordering::SeqCst);
    }

    fn new_handler() -> TextInputHandler {
        TEXT_CALLS.store(0, Ordering::SeqCst);
        CARET_CALLS.store(0, Ordering::SeqCst);
        let mut h = TextInputHandler::new();
        h.set_callbacks(std::ptr::null_mut(), test_text_cb, test_caret_cb);
        h
    }

    #[test]
    fn enable_updates_state_and_notifies() {
        let mut h = new_handler();
        h.enable("hello".to_string(), false);
        assert!(h.is_enabled());
        assert_eq!(h.get_text(), "hello");
        assert_eq!(h.get_cursor_position(), 5);
        assert_eq!(h.get_copy_position(), 5);
        assert_eq!(TEXT_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(CARET_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(*LAST_TEXT.lock().unwrap(), "hello");
    }

    #[test]
    fn typed_text_is_pushed_to_callback() {
        let mut h = new_handler();
        h.enable(String::new(), false);
        h.on_text_input("wo");
        h.on_text_input("rld");
        assert_eq!(h.get_text(), "world");
        assert_eq!(h.get_cursor_position(), 5);
        assert_eq!(TEXT_CALLS.load(Ordering::SeqCst), 3);
        assert_eq!(*LAST_TEXT.lock().unwrap(), "world");
    }

    #[test]
    fn backspace_and_caret_navigation() {
        let mut h = new_handler();
        h.enable("abcd".to_string(), false);
        h.on_text_input("\x08"); // backspace
        assert_eq!(h.get_text(), "abc");
        assert_eq!(h.get_cursor_position(), 3);
        h.on_key_pressed(37, 0, 0); // LEFT arrow
        assert_eq!(h.get_cursor_position(), 2);
        h.on_text_input("X"); // insert mid-text
        assert_eq!(h.get_text(), "abXc");
    }

    #[test]
    fn update_from_game_reflects_back() {
        let mut h = new_handler();
        h.enable(String::new(), false);
        h.update("server".to_string());
        assert_eq!(h.get_text(), "server");
        assert_eq!(*LAST_TEXT.lock().unwrap(), "server");
        h.set_cursor_position(3);
        assert_eq!(h.get_cursor_position(), 3);
    }

    #[test]
    fn disable_clears_state() {
        let mut h = new_handler();
        h.enable("abc".to_string(), false);
        h.disable();
        assert!(!h.is_enabled());
        assert_eq!(h.get_text(), "");
    }
}
