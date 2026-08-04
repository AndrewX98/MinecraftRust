/// Unsigned little-endian base-128 varint encoding (same as C++ simple-ipc)
pub fn encode_unsigned(mut value: u64, buf: &mut [u8; 10]) -> usize {
    let mut i = 0;
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf[i] = byte;
            i += 1;
            break;
        }
        buf[i] = byte | 0x80;
        i += 1;
    }
    i
}

pub fn decode_unsigned(buf: &[u8]) -> Result<(u64, usize), String> {
    match try_decode_unsigned(buf)? {
        Some(v) => Ok(v),
        None => Err("unexpected end of varint".into()),
    }
}

/// Try to decode an unsigned varint. Returns `Ok(None)` when the buffer does
/// not yet contain a complete varint (matching C++ `try_decode_unsigned`,
/// which signals "not complete" rather than erroring). Returns `Err` when the
/// encoding is malformed (more than 9 bytes for a u64).
pub fn try_decode_unsigned(buf: &[u8]) -> Result<Option<(u64, usize)>, String> {
    let mut result = 0u64;
    let mut shift = 0;
    let n = buf.len().min(9);
    for i in 0..n {
        let byte = buf[i];
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(Some((result, i + 1)));
        }
        shift += 7;
    }
    if buf.len() > 9 {
        return Err("varint too long".into());
    }
    Ok(None)
}
