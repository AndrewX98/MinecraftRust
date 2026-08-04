use std::ffi::{c_char, c_void, CStr};

extern "C" {
    fn mcpelauncher_dispatch_get_library_code_region(handle: *mut c_void, base: *mut usize, size: *mut usize);
}

/// Parse a byte pattern string into raw bytes + a match mask.
///
/// Faithful port of the parse loop in `PatchUtils::patternSearch`
/// (`patch_utils.cpp:14-30`): reads two characters at a time, skips spaces,
/// treats `??` as a wildcard (mask byte 0), otherwise hex-decodes a byte
/// (mask byte 0xFF). Stops at the first char whose pair is incomplete.
pub fn parse_pattern(pattern: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut raw = Vec::new();
    let mut mask = Vec::new();
    let mut idx = 0;
    while idx + 1 < pattern.len() {
        let a = pattern[idx];
        let b = pattern[idx + 1];
        if a == b' ' {
            idx += 1;
            continue;
        }
        if a == b'?' && b == b'?' {
            raw.push(0);
            mask.push(0);
        } else {
            raw.push((hex_nibble(a) << 4) | hex_nibble(b));
            mask.push(0xFF);
        }
        idx += 2;
    }
    (raw, mask)
}

fn hex_nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// Scan `buf` for the pattern, honouring wildcard mask bytes.
///
/// Faithful port of the scan loop in `PatchUtils::patternSearch`
/// (`patch_utils.cpp:32-42`): iterates `i` from `size - raw.len()` downwards
/// to 1 (inclusive), returning the first (i.e. highest) matching offset.
/// Offset 0 is never examined, matching C++. Returns `None` if `raw` is empty
/// or larger than the buffer, or nothing matches.
pub fn scan_pattern(buf: &[u8], raw: &[u8], mask: &[u8]) -> Option<usize> {
    if raw.is_empty() || raw.len() > buf.len() {
        return None;
    }
    let mut i = buf.len() - raw.len();
    loop {
        if i == 0 {
            break;
        }
        let mut matched = true;
        for j in 0..raw.len() {
            if raw[j] != (buf[i + j] & mask[j]) {
                matched = false;
                break;
            }
        }
        if matched {
            return Some(i);
        }
        i -= 1;
    }
    None
}

/// `#[no_mangle]` twin of `PatchUtils::patternSearch` (`patch_utils.cpp:14`).
/// Resolves the library code region via the Rust linker dispatch, then scans.
#[no_mangle]
pub unsafe extern "C" fn patternSearch(handle: *mut c_void, pattern: *const c_char) -> *mut c_void {
    if pattern.is_null() {
        return std::ptr::null_mut();
    }
    let pattern = CStr::from_ptr(pattern).to_bytes();
    let (raw, mask) = parse_pattern(pattern);
    let mut base: usize = 0;
    let mut size: usize = 0;
    mcpelauncher_dispatch_get_library_code_region(handle, &mut base, &mut size);
    if base == 0 || size == 0 {
        return std::ptr::null_mut();
    }
    let buf = std::slice::from_raw_parts(base as *const u8, size);
    match scan_pattern(buf, &raw, &mask) {
        Some(off) => base.wrapping_add(off) as *mut c_void,
        None => std::ptr::null_mut(),
    }
}

/// Pure null-terminator scan of a vtable, starting at slot 2 (skipping the
/// typeinfo/offset slots). Port of `PatchUtils::getVtableSize`
/// (`patch_utils.cpp:95-99`): returns the index of the first null slot.
pub fn get_vtable_size(vtable: &[*mut c_void]) -> usize {
    let mut size = 2usize;
    while size < vtable.len() && !vtable[size].is_null() {
        size += 1;
    }
    size
}

/// `#[no_mangle]` twin of `PatchUtils::getVtableSize` (`patch_utils.cpp:95`).
/// Walks the vtable until it hits a null entry.
#[no_mangle]
pub unsafe extern "C" fn getVtableSize(vtable: *mut *mut c_void) -> usize {
    let mut size = 2usize;
    loop {
        if (*vtable.add(size)).is_null() {
            return size;
        }
        size += 1;
    }
}

/// Encode an x86_64 relative call/jump instruction (5 bytes) for a patch site.
///
/// Port of the non-ARM branch of `PatchUtils::patchCallInstruction`
/// (`patch_utils.cpp:68-77`). Emits `0xE9` (jump) or `0xE8` (call) followed by
/// the 32-bit relative displacement `func - patch_off - 5`. Returns `Err` when
/// the target is out of i32 range (C++ throws `std::runtime_error`).
pub fn patch_call_instruction_bytes(patch_off: usize, func: usize, jump: bool) -> Result<[u8; 5], ()> {
    let disp = func as i64 - patch_off as i64 - 5;
    if !(i32::MIN as i64..=i32::MAX as i64).contains(&disp) {
        return Err(());
    }
    let disp = disp as i32;
    let opcode = if jump { 0xE9u8 } else { 0xE8u8 };
    Ok([opcode, disp as u8, (disp >> 8) as u8, (disp >> 16) as u8, (disp >> 24) as u8])
}

/// `#[no_mangle]` twin of `PatchUtils::patchCallInstruction`
/// (`patch_utils.cpp:45`). x86_64 only: mprotects the patch page RWX and writes
/// the 5-byte E9/E8 instruction. No-op on out-of-range targets.
#[no_mangle]
pub unsafe extern "C" fn patchCallInstruction(patch_off: *mut c_void, func: *mut c_void, jump: bool) {
    let off = patch_off as usize;
    let target = func as usize;
    if let Ok(bytes) = patch_call_instruction_bytes(off, target, jump) {
        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
        let page = off & !(page_size - 1);
        let end = off + bytes.len();
        let len = (end + page_size - 1) & !(page_size - 1);
        let len = len - page;
        if libc::mprotect(page as *mut c_void, len, libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC) == 0 {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), patch_off as *mut u8, bytes.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pattern_basic() {
        let (raw, mask) = parse_pattern(b"55 89 e5");
        assert_eq!(raw, vec![0x55, 0x89, 0xe5]);
        assert_eq!(mask, vec![0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn parse_pattern_wildcard() {
        let (raw, mask) = parse_pattern(b"55 89 ??");
        assert_eq!(raw, vec![0x55, 0x89, 0x00]);
        assert_eq!(mask, vec![0xFF, 0xFF, 0x00]);
    }

    #[test]
    fn parse_pattern_mixed_case_hex() {
        let (raw, _) = parse_pattern(b"0A BF aF");
        assert_eq!(raw, vec![0x0A, 0xBF, 0xAF]);
    }

    #[test]
    fn scan_pattern_hit() {
        let buf: &[u8] = &[0x00, 0x55, 0x89, 0xe5, 0x99, 0x55, 0x89, 0xe5, 0xff];
        let (raw, mask) = parse_pattern(b"55 89 e5");
        // Highest match (scanning downward): offset 5.
        assert_eq!(scan_pattern(buf, &raw, &mask), Some(5));
    }

    #[test]
    fn scan_pattern_miss() {
        let buf: &[u8] = &[0x00, 0x55, 0x99, 0x99, 0x99];
        let (raw, mask) = parse_pattern(b"55 89 e5");
        assert_eq!(scan_pattern(buf, &raw, &mask), None);
    }

    #[test]
    fn scan_pattern_wildcard_any_byte() {
        let buf: &[u8] = &[0x00, 0x55, 0x78, 0x12, 0x00, 0x00, 0x00, 0x00];
        let (raw, mask) = parse_pattern(b"55 ??");
        // Highest offset where buf[i]=0x55: index 1.
        assert_eq!(scan_pattern(buf, &raw, &mask), Some(1));
    }

    #[test]
    fn scan_pattern_oversized_pattern() {
        let buf: &[u8] = &[0x01, 0x02];
        let (raw, mask) = parse_pattern(b"55 89 e5");
        assert_eq!(scan_pattern(buf, &raw, &mask), None);
    }

    #[test]
    fn get_vtable_size_null_at_index() {
        let null: *mut c_void = std::ptr::null_mut();
        let mut slots = [
            null, null,
            1usize as *mut c_void,
            2usize as *mut c_void,
            null,
        ];
        let slice = unsafe { std::slice::from_raw_parts_mut(slots.as_mut_ptr(), slots.len()) };
        let slice: &[*mut c_void] = slice;
        assert_eq!(get_vtable_size(slice), 4);
    }

    #[test]
    fn patch_call_instruction_jump() {
        // patch at 0x1000, target at 0x1000+5 => disp 0.
        let bytes = patch_call_instruction_bytes(0x1000, 0x1005, true).unwrap();
        assert_eq!(bytes[0], 0xE9);
        assert_eq!(&bytes[1..], &[0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn patch_call_instruction_call() {
        // patch at 0x1000, target at 0x1005 => disp 0, opcode E8.
        let bytes = patch_call_instruction_bytes(0x1000, 0x1005, false).unwrap();
        assert_eq!(bytes[0], 0xE8);
        assert_eq!(&bytes[1..], &[0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn patch_call_instruction_displacement() {
        // target - off - 5 = 0x1234.
        let bytes = patch_call_instruction_bytes(0x1000, 0x1000 + 5 + 0x1234, true).unwrap();
        assert_eq!(bytes[0], 0xE9);
        assert_eq!(&bytes[1..], &[0x34, 0x12, 0x00, 0x00]);
    }

    #[test]
    fn patch_call_instruction_negative_displacement() {
        // target - off - 5 = -0x1234.
        let off = 0x10000usize;
        let bytes = patch_call_instruction_bytes(off, off + 5 - 0x1234, true).unwrap();
        assert_eq!(bytes[0], 0xE9);
        assert_eq!(&bytes[1..], &[0xCC, 0xED, 0xFF, 0xFF]);
    }

    #[test]
    fn patch_call_instruction_out_of_range() {
        let off = 0x1000usize;
        let far = 0x7fff_ffff_0000_0000usize;
        assert!(patch_call_instruction_bytes(off, far, true).is_err());
    }
}