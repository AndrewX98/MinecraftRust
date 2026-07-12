#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reloc {
    pub r_offset: usize,
    pub r_info: usize,
    pub r_addend: isize,
}

pub const RELOCATION_GROUPED_BY_INFO_FLAG: usize = 1;
pub const RELOCATION_GROUPED_BY_OFFSET_DELTA_FLAG: usize = 2;
pub const RELOCATION_GROUPED_BY_ADDEND_FLAG: usize = 4;
pub const RELOCATION_GROUP_HAS_ADDEND_FLAG: usize = 8;

pub struct Sleb128Decoder<'a> {
    current: *const u8,
    end: *const u8,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> Sleb128Decoder<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Sleb128Decoder {
            current: buffer.as_ptr(),
            end: unsafe { buffer.as_ptr().add(buffer.len()) },
            _marker: std::marker::PhantomData,
        }
    }

    pub fn pop_front(&mut self) -> usize {
        let mut value: usize = 0;
        let size = std::mem::size_of::<usize>() * 8;
        let mut shift: usize = 0;
        let mut byte: u8;

        loop {
            if self.current >= self.end {
                panic!("sleb128_decoder ran out of bounds");
            }
            byte = unsafe { *self.current };
            self.current = unsafe { self.current.add(1) };
            value |= ((byte & 127) as usize) << shift;
            shift += 7;
            if byte & 128 == 0 {
                break;
            }
        }

        if shift < size && (byte & 64) != 0 {
            value |= (1usize << shift).wrapping_neg();
        }

        value
    }
}

pub fn for_all_packed_relocs<F>(mut decoder: Sleb128Decoder, mut callback: F) -> bool
where
    F: FnMut(Reloc) -> bool,
{
    let num_relocs = decoder.pop_front();

    let mut reloc = Reloc {
        r_offset: decoder.pop_front(),
        r_info: 0,
        r_addend: 0,
    };

    let mut idx: usize = 0;
    while idx < num_relocs {
        let group_size = decoder.pop_front();
        let group_flags = decoder.pop_front();

        let mut group_r_offset_delta: usize = 0;
        if group_flags & RELOCATION_GROUPED_BY_OFFSET_DELTA_FLAG != 0 {
            group_r_offset_delta = decoder.pop_front();
        }
        if group_flags & RELOCATION_GROUPED_BY_INFO_FLAG != 0 {
            reloc.r_info = decoder.pop_front();
        }

        let group_flags_reloc = group_flags
            & (RELOCATION_GROUP_HAS_ADDEND_FLAG | RELOCATION_GROUPED_BY_ADDEND_FLAG);
        if group_flags_reloc == RELOCATION_GROUP_HAS_ADDEND_FLAG {
            // Each relocation has its own addend (popped inside loop)
        } else if group_flags_reloc
            == (RELOCATION_GROUP_HAS_ADDEND_FLAG | RELOCATION_GROUPED_BY_ADDEND_FLAG)
        {
            let addend_delta = decoder.pop_front();
            reloc.r_addend = reloc.r_addend.wrapping_add(addend_delta as isize);
        } else {
            reloc.r_addend = 0;
        }

        for _i in 0..group_size {
            if group_flags & RELOCATION_GROUPED_BY_OFFSET_DELTA_FLAG != 0 {
                reloc.r_offset = reloc.r_offset.wrapping_add(group_r_offset_delta);
            } else {
                reloc.r_offset = reloc.r_offset.wrapping_add(decoder.pop_front());
            }
            if group_flags & RELOCATION_GROUPED_BY_INFO_FLAG == 0 {
                reloc.r_info = decoder.pop_front();
            }
            if group_flags_reloc == RELOCATION_GROUP_HAS_ADDEND_FLAG {
                let addend = decoder.pop_front();
                reloc.r_addend = reloc.r_addend.wrapping_add(addend as isize);
            }
            if !callback(reloc) {
                return false;
            }
        }

        idx += group_size;
    }

    true
}

/// Encode a positive usize as SLEB128 (for test helpers and non-negative values only).
/// Ensures the last byte has bit 6 clear to avoid sign extension.
#[cfg(test)]
fn encode_sleb128(values: &[usize]) -> Vec<u8> {
    let mut buf = Vec::new();
    for &v in values {
        let mut val = v;
        loop {
            let mut byte = (val & 0x7f) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
                buf.push(byte);
            } else {
                if byte & 0x40 != 0 {
                    byte |= 0x80;
                    buf.push(byte);
                    buf.push(0x00);
                } else {
                    buf.push(byte);
                }
                break;
            }
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sleb128_zero() {
        let data = encode_sleb128(&[0]);
        let mut dec = Sleb128Decoder::new(&data);
        assert_eq!(dec.pop_front(), 0);
    }

    #[test]
    fn test_sleb128_small_values() {
        let data = encode_sleb128(&[1, 42, 63]);
        let mut dec = Sleb128Decoder::new(&data);
        assert_eq!(dec.pop_front(), 1);
        assert_eq!(dec.pop_front(), 42);
        assert_eq!(dec.pop_front(), 63);
    }

    #[test]
    fn test_sleb128_values_requiring_two_bytes() {
        let data = encode_sleb128(&[64, 127, 128, 255, 300]);
        let mut dec = Sleb128Decoder::new(&data);
        assert_eq!(dec.pop_front(), 64);
        assert_eq!(dec.pop_front(), 127);
        assert_eq!(dec.pop_front(), 128);
        assert_eq!(dec.pop_front(), 255);
        assert_eq!(dec.pop_front(), 300);
    }

    #[test]
    fn test_sleb128_larger_values() {
        let data = encode_sleb128(&[0x1000, 0x2000, 0x3fff, 0x12345678]);
        let mut dec = Sleb128Decoder::new(&data);
        assert_eq!(dec.pop_front(), 0x1000);
        assert_eq!(dec.pop_front(), 0x2000);
        assert_eq!(dec.pop_front(), 0x3fff);
        assert_eq!(dec.pop_front(), 0x12345678);
    }

    #[test]
    fn test_single_reloc_no_grouping() {
        let data = encode_sleb128(&[1,       // num_relocs
                                    0x1000,  // r_offset
                                    1,       // group_size = 1
                                    0,       // group_flags = 0
                                    0,       // offset delta (element 0)
                                    42]);    // r_info (element 0)
        let decoder = Sleb128Decoder::new(&data);
        let mut results = Vec::new();
        let ok = for_all_packed_relocs(decoder, |reloc| {
            results.push(reloc);
            true
        });
        assert!(ok);
        assert_eq!(results.len(), 1);
        // r_offset is cumulative: initial + delta applied to each element
        assert_eq!(results[0].r_offset, 0x1000);
        assert_eq!(results[0].r_info, 42);
        assert_eq!(results[0].r_addend, 0);
    }

    #[test]
    fn test_multiple_relocs_grouped_by_offset_delta() {
        let data = encode_sleb128(&[3,       // num_relocs = 3
                                    0x1000,  // r_offset
                                    3,       // group_size = 3
                                    RELOCATION_GROUPED_BY_OFFSET_DELTA_FLAG,
                                    0x10,   // offset delta
                                    42,     // r_info (element 0)
                                    43,     // r_info (element 1)
                                    44]);   // r_info (element 2)
        let decoder = Sleb128Decoder::new(&data);
        let mut results = Vec::new();
        let ok = for_all_packed_relocs(decoder, |reloc| {
            results.push(reloc);
            true
        });
        assert!(ok);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].r_offset, 0x1010);
        assert_eq!(results[1].r_offset, 0x1020);
        assert_eq!(results[2].r_offset, 0x1030);
        assert_eq!(results[0].r_info, 42);
        assert_eq!(results[1].r_info, 43);
        assert_eq!(results[2].r_info, 44);
    }

    #[test]
    fn test_relocs_grouped_by_info() {
        let data = encode_sleb128(&[2,       // num_relocs = 2
                                    0x1000,  // r_offset
                                    2,       // group_size = 2
                                    RELOCATION_GROUPED_BY_INFO_FLAG,
                                    42,     // r_info (shared)
                                    0x200,  // offset delta (element 0)
                                    0x300]); // offset delta (element 1)
        let decoder = Sleb128Decoder::new(&data);
        let mut results = Vec::new();
        let ok = for_all_packed_relocs(decoder, |reloc| {
            results.push(reloc);
            true
        });
        assert!(ok);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].r_offset, 0x1200);
        assert_eq!(results[0].r_info, 42);
        assert_eq!(results[1].r_offset, 0x1500);
        assert_eq!(results[1].r_info, 42);
    }

    #[test]
    fn test_has_addend_per_element() {
        let data = encode_sleb128(&[2,       // num_relocs = 2
                                    0x1000,  // r_offset
                                    2,       // group_size = 2
                                    RELOCATION_GROUP_HAS_ADDEND_FLAG,
                                    0x10,   // offset delta (element 0)
                                    42,     // r_info (element 0)
                                    5,      // addend (element 0)
                                    0x20,   // offset delta (element 1)
                                    43,     // r_info (element 1)
                                    7]);    // addend (element 1)
        let decoder = Sleb128Decoder::new(&data);
        let mut results = Vec::new();
        let ok = for_all_packed_relocs(decoder, |reloc| {
            results.push(reloc);
            true
        });
        assert!(ok);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].r_offset, 0x1010);
        assert_eq!(results[0].r_addend, 5);
        assert_eq!(results[0].r_info, 42);
        assert_eq!(results[1].r_offset, 0x1030);
        assert_eq!(results[1].r_addend, 12);
        assert_eq!(results[1].r_info, 43);
    }

    #[test]
    fn test_addend_grouped() {
        let data = encode_sleb128(&[2,       // num_relocs = 2
                                    0x1000,  // r_offset
                                    2,       // group_size = 2
                                    RELOCATION_GROUP_HAS_ADDEND_FLAG
                                        | RELOCATION_GROUPED_BY_ADDEND_FLAG,
                                    0x100,  // base addend (grouped by addend)
                                    0x10,   // offset delta (element 0)
                                    42,     // r_info (element 0)
                                    0x10,   // offset delta (element 1)
                                    43]);   // r_info (element 1)
        let decoder = Sleb128Decoder::new(&data);
        let mut results = Vec::new();
        let ok = for_all_packed_relocs(decoder, |reloc| {
            results.push(reloc);
            true
        });
        assert!(ok);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].r_offset, 0x1010);
        assert_eq!(results[0].r_addend, 0x100);
        assert_eq!(results[0].r_info, 42);
        assert_eq!(results[1].r_offset, 0x1020);
        assert_eq!(results[1].r_addend, 0x100);
        assert_eq!(results[1].r_info, 43);
    }

    #[test]
    fn test_stop_early() {
        let data = encode_sleb128(&[5,       // num_relocs = 5
                                    0x1000,  // r_offset
                                    5,       // group_size = 5
                                    RELOCATION_GROUPED_BY_OFFSET_DELTA_FLAG,
                                    0x10,   // offset delta
                                    1,      // r_info (element 0)
                                    2,      // r_info (element 1)
                                    3]);    // r_info (element 2)
        let decoder = Sleb128Decoder::new(&data);
        let mut count = 0;
        let ok = for_all_packed_relocs(decoder, |_reloc| {
            count += 1;
            count < 3
        });
        assert!(!ok);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_zero_relocs() {
        // Format always has r_offset even with 0 relocs
        let data = encode_sleb128(&[0, 0]); // num_relocs=0, r_offset=0
        let decoder = Sleb128Decoder::new(&data);
        let mut count = 0;
        let ok = for_all_packed_relocs(decoder, |_reloc| {
            count += 1;
            true
        });
        assert!(ok);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_all_flags() {
        let data = encode_sleb128(&[4,       // num_relocs = 4
                                    0x2000,  // r_offset
                                    4,       // group_size = 4
                                    RELOCATION_GROUPED_BY_OFFSET_DELTA_FLAG
                                        | RELOCATION_GROUPED_BY_INFO_FLAG
                                        | RELOCATION_GROUP_HAS_ADDEND_FLAG,
                                    0x100,  // offset delta (grouped)
                                    99,     // r_info (shared)
                                    10,     // addend (element 0)
                                    20,     // addend (element 1)
                                    30,     // addend (element 2)
                                    40]);   // addend (element 3)
        let decoder = Sleb128Decoder::new(&data);
        let mut results = Vec::new();
        let ok = for_all_packed_relocs(decoder, |reloc| {
            results.push(reloc);
            true
        });
        assert!(ok);
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].r_offset, 0x2100);
        assert_eq!(results[1].r_offset, 0x2200);
        assert_eq!(results[2].r_offset, 0x2300);
        assert_eq!(results[3].r_offset, 0x2400);
        assert_eq!(results[0].r_info, 99);
        assert_eq!(results[1].r_info, 99);
        assert_eq!(results[2].r_info, 99);
        assert_eq!(results[3].r_info, 99);
        // r_addend is cumulative (uses +=)
        assert_eq!(results[0].r_addend, 10);
        assert_eq!(results[1].r_addend, 30);
        assert_eq!(results[2].r_addend, 60);
        assert_eq!(results[3].r_addend, 100);
    }
}
