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
    let mut result = 0u64;
    let mut shift = 0;
    for (i, &byte) in buf.iter().enumerate() {
        if i >= 9 {
            return Err("varint too long".into());
        }
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err("unexpected end of varint".into())
}
