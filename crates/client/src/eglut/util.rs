use std::ffi::c_char;

pub unsafe fn hex_val(c: u8) -> i32 {
    match c {
        b'0'..=b'9' => (c - b'0') as i32,
        b'a'..=b'f' => (c - b'a' + 10) as i32,
        b'A'..=b'F' => (c - b'A' + 10) as i32,
        _ => -1,
    }
}

pub unsafe fn url_decode_len(s: *const c_char) -> i32 {
    let mut len = 0i32;
    let mut i = 0;
    loop {
        let c = *s.offset(i);
        if c == 0 { break; }
        if c == b'%' as i8 { i += 3; } else { i += 1; }
        len += 1;
    }
    len
}

pub unsafe fn url_decode(inp: *const c_char, outp: *mut c_char) {
    let mut j = 0;
    let mut i = 0;
    loop {
        let c = *inp.offset(i);
        if c == 0 { break; }
        if c == b'+' as i8 {
            *outp.offset(j) = b' ' as i8;
            i += 1;
        } else if c == b'%' as i8 {
            let hi = hex_val(*inp.offset(i + 1) as u8);
            let lo = hex_val(*inp.offset(i + 2) as u8);
            *outp.offset(j) = ((hi << 4) | lo) as i8;
            i += 3;
        } else {
            *outp.offset(j) = c;
            i += 1;
        }
        j += 1;
    }
    *outp.offset(j) = 0;
}

pub unsafe fn key_sym_to_eglut(sym: u64) -> i32 {
    use x11::keysym::*;
    match sym {
        k if k == XK_BackSpace as u64 => 0xFF08i32,
        k if k == XK_Tab as u64 => 0xFF09,
        k if k == XK_Linefeed as u64 => 0xFF0A,
        k if k == XK_Clear as u64 => 0xFF0B,
        k if k == XK_Return as u64 => 0xFF0D,
        k if k == XK_Pause as u64 => 0xFF13,
        k if k == XK_Scroll_Lock as u64 => 0xFF14,
        k if k == XK_Sys_Req as u64 => 0xFF15,
        k if k == XK_Escape as u64 => 0xFF1B,
        k if k == XK_Delete as u64 => 0xFFFF,
        k if k == XK_Home as u64 => 0xFF50,
        k if k == XK_Left as u64 => 0xFF51,
        k if k == XK_Up as u64 => 0xFF52,
        k if k == XK_Right as u64 => 0xFF53,
        k if k == XK_Down as u64 => 0xFF54,
        k if k == XK_Page_Up as u64 => 0xFF55,
        k if k == XK_Page_Down as u64 => 0xFF56,
        k if k == XK_End as u64 => 0xFF57,
        k if k == XK_Begin as u64 => 0xFF58,
        k if k == XK_Shift_L as u64 => 0xFFE1,
        k if k == XK_Shift_R as u64 => 0xFFE2,
        k if k == XK_Control_L as u64 => 0xFFE3,
        k if k == XK_Control_R as u64 => 0xFFE4,
        k if k == XK_Caps_Lock as u64 => 0xFFE5,
        k if k == XK_Shift_Lock as u64 => 0xFFE6,
        k if k == XK_Meta_L as u64 => 0xFFE7,
        k if k == XK_Meta_R as u64 => 0xFFE8,
        k if k == XK_Alt_L as u64 => 0xFFE9,
        k if k == XK_Alt_R as u64 => 0xFFEA,
        _ => sym as i32,
    }
}
