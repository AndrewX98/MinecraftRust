use crate::utils;

pub struct MappedFileFragment {
    map_start: *mut libc::c_void,
    map_size: usize,
    data: *const u8,
    size: usize,
}

impl MappedFileFragment {
    pub fn new() -> Self {
        MappedFileFragment {
            map_start: std::ptr::null_mut(),
            map_size: 0,
            data: std::ptr::null(),
            size: 0,
        }
    }

    pub fn map(
        &mut self,
        fd: i32,
        base_offset: i64,
        elf_offset: usize,
        size: usize,
    ) -> bool {
        let offset = match utils::safe_add(base_offset, elf_offset) {
            Some(o) => o,
            None => return false,
        };

        let page_min = utils::page_start(offset);
        let end_offset = match utils::safe_add(offset, size) {
            Some(o) => o,
            None => return false,
        };
        let end_offset = match utils::safe_add(end_offset, utils::page_offset(offset)) {
            Some(o) => o,
            None => return false,
        };

        let map_size = (end_offset - page_min) as usize;

        let map_start = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                map_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE,
                fd,
                page_min,
            )
        };

        if map_start == libc::MAP_FAILED {
            return false;
        }

        self.map_start = map_start;
        self.map_size = map_size;
        self.data = unsafe { (map_start as *const u8).add(utils::page_offset(offset)) };
        self.size = size;

        true
    }

    pub fn data(&self) -> *const u8 {
        self.data
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for MappedFileFragment {
    fn drop(&mut self) {
        if !self.map_start.is_null() {
            unsafe {
                libc::munmap(self.map_start, self.map_size);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::io::AsRawFd;

    #[test]
    fn test_map_fragment() {
        let mut tmp = tempfile::tempfile().unwrap();
        let data = b"hello world this is a test file for mmap";
        tmp.write_all(data).unwrap();
        tmp.flush().unwrap();

        let mut frag = MappedFileFragment::new();
        assert!(frag.map(tmp.as_raw_fd(), 0, 0, data.len()));

        assert_eq!(frag.size(), data.len());
        unsafe {
            let slice = std::slice::from_raw_parts(frag.data(), frag.size());
            assert_eq!(slice, data);
        }
    }

    #[test]
    fn test_map_with_offset() {
        let mut tmp = tempfile::tempfile().unwrap();
        let data = b"abcdefghijklmnopqrstuvwxyz";
        tmp.write_all(data).unwrap();
        tmp.flush().unwrap();

        let mut frag = MappedFileFragment::new();
        assert!(frag.map(tmp.as_raw_fd(), 0, 5, 10));

        assert_eq!(frag.size(), 10);
        unsafe {
            let slice = std::slice::from_raw_parts(frag.data(), frag.size());
            assert_eq!(slice, b"fghijklmno");
        }
    }

    #[test]
    fn test_map_invalid_fd() {
        let mut frag = MappedFileFragment::new();
        assert!(!frag.map(-1, 0, 0, 10));
    }

    #[test]
    fn test_drop_unmaps() {
        let mut tmp = tempfile::tempfile().unwrap();
        tmp.write_all(b"test").unwrap();
        tmp.flush().unwrap();

        let mut frag = MappedFileFragment::new();
        assert!(frag.map(tmp.as_raw_fd(), 0, 0, 4));
        let _data_ptr = frag.data();
        drop(frag);
    }
}
