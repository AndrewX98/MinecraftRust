const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(input: &[u8], padded: bool) -> String {
    let n = input.len();
    let block_count = (n + 2) / 3;
    let output_len = if padded {
        block_count * 4
    } else {
        (n / 3) * 4 + if n % 3 == 0 { 0 } else { 1 + n % 3 }
    };
    let mut out = String::with_capacity(output_len);

    for i in 0..block_count {
        let i0 = input[i * 3];
        let i1 = if n > i * 3 + 1 { input[i * 3 + 1] } else { 0 };
        let i2 = if n > i * 3 + 2 { input[i * 3 + 2] } else { 0 };
        let n0 = (i0 >> 2) & 0x3f;
        let n1 = ((i0 & 3) << 4) | ((i1 >> 4) & 0xf);
        let n2 = ((i1 & 0xf) << 2) | ((i2 >> 6) & 3);
        let n3 = i2 & 0x3f;

        out.push(TABLE[n0 as usize] as char);
        out.push(TABLE[n1 as usize] as char);
        if i != block_count - 1 {
            out.push(TABLE[n2 as usize] as char);
            out.push(TABLE[n3 as usize] as char);
        } else {
            if n > i * 3 + 1 {
                out.push(TABLE[n2 as usize] as char);
            } else if padded {
                out.push('=');
            }
            if n > i * 3 + 2 {
                out.push(TABLE[n3 as usize] as char);
            } else if padded {
                out.push('=');
            }
        }
    }
    out
}

pub fn decode(input: &str, skip_chars: &[char]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut i = [0u8; 4];
    let mut p = 0usize;
    let bytes = input.as_bytes();

    while p < bytes.len() {
        for n in 0..4 {
            loop {
                if p >= bytes.len() {
                    if n < 2 {
                        return Err("Unexpected end of input".into());
                    }
                    i[n] = 64;
                    break;
                }
                let c = bytes[p];
                let val = match c {
                    b'A'..=b'Z' => c - b'A',
                    b'a'..=b'z' => c - b'a' + 26,
                    b'0'..=b'9' => c - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    b'=' => 64,
                    _ => 255,
                };
                if val != 255 {
                    if (val == 64 && n < 2) || (n == 3 && i[n - 1] == 64 && val != 64) {
                        return Err("Invalid '=' character".into());
                    }
                    i[n] = val;
                    p += 1;
                    break;
                }
                if !skip_chars.contains(&(c as char)) {
                    return Err(format!("Invalid character at position {}", p));
                }
                p += 1;
            }
        }
        output.push((i[0] << 2) | ((i[1] >> 4) & 3));
        if i[2] != 64 {
            output.push(((i[1] & 0xf) << 4) | ((i[2] >> 2) & 0xf));
        }
        if i[2] != 64 && i[3] != 64 {
            output.push(((i[2] & 3) << 6) | i[3]);
        }
    }
    Ok(output)
}
