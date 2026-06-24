#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    StartNamespace,
    EndNamespace,
    StartElement,
    EndElement,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ChunkHeader {
    ty: u16,
    header_size: u16,
    size: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct StringPoolHeader {
    header: ChunkHeader,
    string_count: u32,
    style_count: u32,
    flags: u32,
    strings_start: u32,
    styles_start: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ResValue {
    pub size: u16,
    pub res0: u8,
    pub data_type: u8,
    pub data: u32,
}

impl ResValue {
    pub fn is_integer(&self) -> bool {
        self.data_type >= 0x10 && self.data_type <= 0x1f
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct XmlNodeChunkHeader {
    header: ChunkHeader,
    line_number: u32,
    comment: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NamespaceChunkData {
    prefix: u32,
    url: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ElementChunkData {
    ns: u32,
    name: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct StartElementChunkData {
    element: ElementChunkData,
    attribute_start: u16,
    attribute_size: u16,
    attribute_count: u16,
    id_index: u16,
    class_index: u16,
    style_index: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct AttributeChunkData {
    ns: u32,
    name: u32,
    raw_value: u32,
    typed_value: ResValue,
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

pub struct StringPool<'a> {
    data: &'a [u8],
    header_offset: usize,
    utf8: bool,
}

impl<'a> StringPool<'a> {
    fn new(data: &'a [u8], header_offset: usize) -> Self {
        let flags = read_u32(data, header_offset + 12);
        let utf8 = (flags & (1 << 8)) != 0;
        StringPool {
            data,
            header_offset,
            utf8,
        }
    }

    fn string_count(&self) -> u32 {
        read_u32(self.data, self.header_offset + 8)
    }

    fn strings_start(&self) -> u32 {
        read_u32(self.data, self.header_offset + 16)
    }

    fn get_string_at_offset(&self, offset: usize) -> String {
        if self.utf8 {
            self.decode_utf8(offset)
        } else {
            self.decode_utf16(offset)
        }
    }

    fn decode_utf8(&self, offset: usize) -> String {
        let data = &self.data[offset..];
        let (_, skip1) = decode_length_utf8(data);
        let (_, skip2) = decode_length_utf8(&data[skip1..]);
        let total_skip = skip1 + skip2;
        let mut end = total_skip;
        while end < data.len() && data[end] != 0 {
            end += 1;
        }
        String::from_utf8_lossy(&data[total_skip..end]).to_string()
    }

    fn decode_utf16(&self, offset: usize) -> String {
        let data = self.data;
        let slice = &data[offset..];
        if slice.len() < 4 {
            return String::new();
        }

        let mut skip = 0;
        let _len_val = read_u16(slice, skip);
        skip += 2;
        let _len = if _len_val == 0x7fff {
            let v = read_u32(slice, skip) as usize;
            skip += 4;
            if v > 0 { v - 1 } else { 0 }
        } else {
            let v = _len_val as usize;
            if v > 0 { v - 1 } else { 0 }
        };

        let mut s = String::new();
        let mut i = skip;
        while i + 1 < slice.len() {
            let code_unit = read_u16(slice, i);
            if code_unit == 0 {
                break;
            }
            if code_unit >= 0xd800 && code_unit <= 0xdbff {
                if i + 3 < slice.len() {
                    let low = read_u16(slice, i + 2);
                    if low >= 0xdc00 && low <= 0xdfff {
                        let cp =
                            0x10000 + ((code_unit as u32 - 0xd800) << 10) | (low as u32 - 0xdc00);
                        if let Some(c) = char::from_u32(cp) {
                            s.push(c);
                        }
                        i += 4;
                        continue;
                    }
                }
            }
            if let Some(c) = char::from_u32(code_unit as u32) {
                s.push(c);
            }
            i += 2;
        }
        s
    }

    pub fn get_string(&self, index: u32) -> String {
        if index == 0xffff_ffff {
            return String::new();
        }
        let strings_start = self.strings_start() as usize;
        let string_offset_offset = self.header_offset + 28 + (index as usize) * 4;
        if string_offset_offset + 4 > self.data.len() {
            return String::new();
        }
        let string_offset = read_u32(self.data, string_offset_offset) as usize;
        self.get_string_at_offset(strings_start + string_offset)
    }
}

fn decode_length_utf8(data: &[u8]) -> (usize, usize) {
    if data.is_empty() {
        return (0, 0);
    }
    let first = data[0] as usize;
    if first & 0x80 != 0 {
        let len = ((first & 0x7f) << 8) | (data.get(1).copied().unwrap_or(0) as usize);
        (len, 2)
    } else {
        (first, 1)
    }
}

pub struct AXMLFile<'a> {
    data: &'a [u8],
    string_pool: StringPool<'a>,
    document_range: ChunkRange<'a>,
}

impl<'a> AXMLFile<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, &'a str> {
        if data.len() < 8 {
            return Err("File too small");
        }
        let ty = read_u16(data, 0);
        if ty != 3 {
            return Err("Not an AXML file (expected type 3)");
        }
        let header_size = read_u16(data, 2);
        let total_size = read_u32(data, 4) as usize;

        if total_size > data.len() {
            return Err("File truncated");
        }

        let data = &data[..total_size];

        let mut header_offset = header_size as usize;
        let mut string_pool = StringPool::new(data, 0);

        if header_offset + 8 <= data.len() {
            let child_type = read_u16(data, header_offset);
            if child_type == 1 {
                string_pool = StringPool::new(data, header_offset);
                let pool_size = read_u32(data, header_offset + 4) as usize;
                header_offset += pool_size;
            }
        }

        let doc_range = ChunkRange {
            data,
            begin: header_offset,
            end: total_size,
        };

        Ok(AXMLFile {
            data,
            string_pool,
            document_range: doc_range,
        })
    }

    pub fn string_pool(&self) -> &StringPool<'a> {
        &self.string_pool
    }

    pub fn document_range(&self) -> &ChunkRange<'a> {
        &self.document_range
    }

    pub fn parser(&'a self) -> AXMLParser<'a> {
        AXMLParser {
            data: self.data,
            range: &self.document_range,
            string_pool: &self.string_pool,
            pos: self.document_range.begin,
            first_el: true,
            event_type: EventType::StartElement,
        }
    }
}

pub struct ChunkRange<'a> {
    data: &'a [u8],
    begin: usize,
    end: usize,
}

impl<'a> ChunkRange<'a> {
    pub fn is_empty(&self) -> bool {
        self.begin >= self.end
    }
}

fn read_chunk_header(data: &[u8], offset: usize) -> Option<(u16, u16, u32)> {
    if offset + 8 > data.len() {
        return None;
    }
    Some((
        read_u16(data, offset),
        read_u16(data, offset + 2),
        read_u32(data, offset + 4),
    ))
}

pub struct AXMLParser<'a> {
    data: &'a [u8],
    range: &'a ChunkRange<'a>,
    string_pool: &'a StringPool<'a>,
    pos: usize,
    first_el: bool,
    event_type: EventType,
}

impl<'a> AXMLParser<'a> {
    fn increment_iterator(&mut self) {
        if self.pos >= self.range.end {
            return;
        }
        let (ty, _header_size, size) = match read_chunk_header(self.data, self.pos) {
            Some(h) => h,
            None => return,
        };
        if size == 0 {
            self.pos = self.range.end;
            return;
        }
        if ty == 0x103 {
            self.event_type = EventType::EndElement;
            self.pos += size as usize;
            return;
        }
        if ty == 0x101 {
            self.event_type = EventType::EndNamespace;
            self.pos += size as usize;
            return;
        }
        self.pos += size as usize;
        self.increment_iterator();
    }

    pub fn next(&mut self) -> bool {
        if self.first_el {
            self.first_el = false;
        } else {
            self.increment_iterator();
        }
        if self.pos >= self.range.end {
            return false;
        }
        let (ty, _, _) = match read_chunk_header(self.data, self.pos) {
            Some(h) => h,
            None => return false,
        };
        self.event_type = if ty == 0x100 {
            EventType::StartNamespace
        } else {
            EventType::StartElement
        };
        true
    }

    pub fn event_type(&self) -> EventType {
        self.event_type
    }

    fn get_node_offset(&self) -> usize {
        let (_, header_size, _) = read_chunk_header(self.data, self.pos).unwrap();
        self.pos + header_size as usize
    }

    pub fn get_namespace_prefix(&self) -> String {
        let offset = self.get_node_offset();
        let prefix = read_u32(self.data, offset);
        self.string_pool.get_string(prefix)
    }

    pub fn get_namespace_url(&self) -> String {
        let offset = self.get_node_offset();
        let url = read_u32(self.data, offset + 4);
        self.string_pool.get_string(url)
    }

    pub fn get_element_ns(&self) -> String {
        let offset = self.get_node_offset();
        let ns = read_u32(self.data, offset);
        self.string_pool.get_string(ns)
    }

    pub fn get_element_name(&self) -> String {
        let offset = self.get_node_offset();
        let name = read_u32(self.data, offset + 4);
        self.string_pool.get_string(name)
    }

    pub fn get_element_attribute_count(&self) -> usize {
        let offset = self.get_node_offset();
        read_u16(self.data, offset + 12) as usize
    }

    pub fn get_element_attribute_ns(&self, i: usize) -> String {
        let attr = self.get_element_attribute(i);
        self.string_pool.get_string(attr.ns)
    }

    pub fn get_element_attribute_name(&self, i: usize) -> String {
        let attr = self.get_element_attribute(i);
        self.string_pool.get_string(attr.name)
    }

    pub fn get_element_attribute_raw_value(&self, i: usize) -> String {
        let attr = self.get_element_attribute(i);
        self.string_pool.get_string(attr.raw_value)
    }

    pub fn get_element_attribute_typed_value(&self, i: usize) -> ResValue {
        self.get_element_attribute(i).typed_value
    }

    fn get_element_attribute(&self, i: usize) -> AttributeChunkData {
        let offset = self.get_node_offset();
        // StartElementChunkData: elem_ns(4) + elem_name(4) + attr_start(2) + attr_size(2) + attr_count(2) + id_index(2) + class_index(2) + style_index(2) = 20 bytes
        let attr_start = read_u16(self.data, offset + 8) as usize;
        let attr_offset = offset + attr_start + i * 20; // AttributeChunkData is 20 bytes
        AttributeChunkData {
            ns: read_u32(self.data, attr_offset),
            name: read_u32(self.data, attr_offset + 4),
            raw_value: read_u32(self.data, attr_offset + 8),
            typed_value: ResValue {
                size: read_u16(self.data, attr_offset + 12),
                res0: self.data[attr_offset + 14],
                data_type: self.data[attr_offset + 15],
                data: read_u32(self.data, attr_offset + 16),
            },
        }
    }
}
