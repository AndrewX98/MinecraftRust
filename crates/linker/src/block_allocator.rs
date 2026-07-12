const ALLOCATE_SIZE: usize = 4096 * 100;

fn round_up(size: usize, multiplier: usize) -> usize {
    (size + (multiplier - 1)) & !(multiplier - 1)
}

#[repr(C)]
struct Page {
    next: *mut Page,
    bytes: [u8; ALLOCATE_SIZE - 16],
}

#[repr(C)]
struct FreeBlockInfo {
    next_block: *mut std::ffi::c_void,
    num_free_blocks: usize,
}

pub struct LinkerBlockAllocator {
    block_size: usize,
    page_list: *mut Page,
    free_block_list: *mut std::ffi::c_void,
    allocated: usize,
}

unsafe impl Send for LinkerBlockAllocator {}

impl LinkerBlockAllocator {
    pub fn new(block_size: usize) -> Self {
        let min_size = std::mem::size_of::<FreeBlockInfo>();
        let block_size = if block_size < min_size { min_size } else { block_size };
        let block_size = round_up(block_size, 16);
        LinkerBlockAllocator {
            block_size,
            page_list: std::ptr::null_mut(),
            free_block_list: std::ptr::null_mut(),
            allocated: 0,
        }
    }

    pub fn alloc(&mut self) -> *mut std::ffi::c_void {
        if self.free_block_list.is_null() {
            self.create_new_page();
        }

        let block_info = self.free_block_list as *mut FreeBlockInfo;
        unsafe {
            if (*block_info).num_free_blocks > 1 {
                let next = (block_info as *mut u8).add(self.block_size) as *mut FreeBlockInfo;
                (*next).next_block = (*block_info).next_block;
                (*next).num_free_blocks = (*block_info).num_free_blocks - 1;
                self.free_block_list = next as *mut std::ffi::c_void;
            } else {
                self.free_block_list = (*block_info).next_block;
            }
            std::ptr::write_bytes(block_info as *mut u8, 0, self.block_size);
        }
        self.allocated += 1;
        block_info as *mut std::ffi::c_void
    }

    pub fn free(&mut self, block: *mut std::ffi::c_void) {
        if block.is_null() {
            return;
        }

        let page = self.find_page(block);
        if page.is_null() {
            panic!("free: block not found in any page");
        }

        let offset = unsafe { (block as usize) - (&raw const (*page).bytes) as usize };
        if offset % self.block_size != 0 {
            panic!("free: block not aligned");
        }

        unsafe {
            std::ptr::write_bytes(block as *mut u8, 0, self.block_size);
        }

        let block_info = block as *mut FreeBlockInfo;
        unsafe {
            (*block_info).next_block = self.free_block_list;
            (*block_info).num_free_blocks = 1;
        }
        self.free_block_list = block_info as *mut std::ffi::c_void;
        self.allocated -= 1;
    }

    pub fn protect_all(&self, _prot: i32) {
    }

    pub fn purge(&mut self) {
        if self.allocated != 0 {
            return;
        }

        let mut page = self.page_list;
        while !page.is_null() {
            unsafe {
                let next = (*page).next;
                libc::munmap(page as *mut libc::c_void, ALLOCATE_SIZE);
                page = next;
            }
        }
        self.page_list = std::ptr::null_mut();
        self.free_block_list = std::ptr::null_mut();
    }

    fn create_new_page(&mut self) {
        let page = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                ALLOCATE_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if page == libc::MAP_FAILED {
            panic!("LinkerBlockAllocator: mmap failed (OOM)");
        }

        let page = page as *mut Page;
        unsafe {
            let first_block = &raw mut (*page).bytes as *mut u8 as *mut FreeBlockInfo;
            (*first_block).next_block = self.free_block_list;
            let bytes_len = ALLOCATE_SIZE - std::mem::size_of::<*mut Page>();
            (*first_block).num_free_blocks = bytes_len / self.block_size;

            self.free_block_list = first_block as *mut std::ffi::c_void;
            (*page).next = self.page_list;
            self.page_list = page;
        }
    }

    fn find_page(&self, block: *mut std::ffi::c_void) -> *mut Page {
        if block.is_null() {
            panic!("find_page: null block");
        }

        let mut page = self.page_list;
        while !page.is_null() {
            unsafe {
                let page_ptr = page as *const u8;
                let page_end = page_ptr.add(ALLOCATE_SIZE);
                let block_ptr = block as *const u8;
                let page_start = page_ptr.add(std::mem::size_of::<*mut Page>());
                if block_ptr >= page_start && block_ptr < page_end {
                    return page;
                }
            }
            unsafe {
                page = (*page).next;
            }
        }
        panic!("find_page: block not found");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_free() {
        let mut alloc = LinkerBlockAllocator::new(64);
        let p1 = alloc.alloc();
        assert!(!p1.is_null());
        let p2 = alloc.alloc();
        assert!(!p2.is_null());
        assert_ne!(p1, p2);
        alloc.free(p1);
        alloc.free(p2);
    }

    #[test]
    fn test_alloc_reuses_freed() {
        let mut alloc = LinkerBlockAllocator::new(32);
        let p1 = alloc.alloc();
        alloc.free(p1);
        let p2 = alloc.alloc();
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_alloc_zeroes_memory() {
        let mut alloc = LinkerBlockAllocator::new(64);
        let p = alloc.alloc();
        unsafe {
            let bytes = std::slice::from_raw_parts(p as *const u8, 64);
            assert!(bytes.iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn test_purge() {
        let mut alloc = LinkerBlockAllocator::new(32);
        let p = alloc.alloc();
        alloc.free(p);
        alloc.purge();
        let p2 = alloc.alloc();
        assert!(!p2.is_null());
    }

    #[test]
    fn test_multiple_allocations() {
        let mut alloc = LinkerBlockAllocator::new(16);
        let mut ptrs = Vec::new();
        for _ in 0..100 {
            ptrs.push(alloc.alloc());
        }
        for &p in &ptrs {
            alloc.free(p);
        }
        alloc.purge();
    }
}
