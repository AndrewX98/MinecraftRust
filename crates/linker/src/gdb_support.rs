use std::sync::Mutex;

#[repr(C)]
pub struct LinkMap {
    pub l_addr: usize,
    pub l_name: *const i8,
    pub l_ld: *mut u8,
    pub l_next: *mut LinkMap,
    pub l_prev: *mut LinkMap,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RDebugState {
    RtConsistent = 0,
    RtAdd = 1,
    RtDelete = 2,
}

#[repr(C)]
pub struct RDebug {
    pub r_version: i32,
    pub r_map: *mut LinkMap,
    pub r_brk: usize,
    pub r_state: RDebugState,
    pub r_ldbase: usize,
}

unsafe impl Send for LinkMap {}
unsafe impl Sync for LinkMap {}
unsafe impl Send for RDebug {}
unsafe impl Sync for RDebug {}

#[no_mangle]
pub extern "C" fn rtld_db_dlactivity() {}

static R_DEBUG_TAIL: Mutex<usize> = Mutex::new(0);
static R_DEBUG_MUTEX: Mutex<()> = Mutex::new(());

fn init_r_debug() -> RDebug {
    RDebug {
        r_version: 1,
        r_map: std::ptr::null_mut(),
        r_brk: rtld_db_dlactivity as usize,
        r_state: RDebugState::RtConsistent,
        r_ldbase: 0,
    }
}

static R_DEBUG: std::sync::LazyLock<Mutex<RDebug>> =
    std::sync::LazyLock::new(|| Mutex::new(init_r_debug()));

pub fn with_r_debug_mut<F: FnOnce(&mut RDebug)>(f: F) {
    let mut guard = R_DEBUG.lock().unwrap();
    f(&mut guard);
}

fn tail_ptr() -> *mut LinkMap {
    let v = R_DEBUG_TAIL.lock().unwrap();
    *v as *mut LinkMap
}

fn set_tail(map: *mut LinkMap) {
    *R_DEBUG_TAIL.lock().unwrap() = map as usize;
}

pub fn insert_link_map_into_debug_map(map: *mut LinkMap) {
    unsafe {
        let tail = tail_ptr();
        if !tail.is_null() {
            (*tail).l_next = map;
            (*map).l_prev = tail;
            (*map).l_next = std::ptr::null_mut();
        } else {
            with_r_debug_mut(|rdebug| {
                rdebug.r_map = map;
            });
            (*map).l_prev = std::ptr::null_mut();
            (*map).l_next = std::ptr::null_mut();
        }
        set_tail(map);
    }
}

pub fn remove_link_map_from_debug_map(map: *mut LinkMap) {
    unsafe {
        let tail = tail_ptr();
        if tail == map {
            set_tail((*map).l_prev);
        }
        if !(*map).l_prev.is_null() {
            (*(*map).l_prev).l_next = (*map).l_next;
        }
        if !(*map).l_next.is_null() {
            (*(*map).l_next).l_prev = (*map).l_prev;
        }
    }
}

pub fn notify_gdb_of_load(map: *mut LinkMap) {
    let _lock = R_DEBUG_MUTEX.lock().unwrap();
    with_r_debug_mut(|rdebug| rdebug.r_state = RDebugState::RtAdd);
    rtld_db_dlactivity();
    insert_link_map_into_debug_map(map);
    with_r_debug_mut(|rdebug| rdebug.r_state = RDebugState::RtConsistent);
    rtld_db_dlactivity();
}

pub fn notify_gdb_of_unload(map: *mut LinkMap) {
    let _lock = R_DEBUG_MUTEX.lock().unwrap();
    with_r_debug_mut(|rdebug| rdebug.r_state = RDebugState::RtDelete);
    rtld_db_dlactivity();
    remove_link_map_from_debug_map(map);
    with_r_debug_mut(|rdebug| rdebug.r_state = RDebugState::RtConsistent);
    rtld_db_dlactivity();
}

pub fn notify_gdb_of_libraries() {
    with_r_debug_mut(|rdebug| rdebug.r_state = RDebugState::RtAdd);
    rtld_db_dlactivity();
    with_r_debug_mut(|rdebug| rdebug.r_state = RDebugState::RtConsistent);
    rtld_db_dlactivity();
}

pub const K_LOG_ERRORS: u32 = 1;
pub const K_LOG_DLOPEN: u32 = 2;
pub const K_LOG_DLSYM: u32 = 4;

pub static G_GREYLIST_DISABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub struct LinkerLogger {
    flags: u32,
}

impl LinkerLogger {
    pub const fn new() -> Self {
        Self { flags: 0 }
    }

    pub fn reset_state(&mut self) {
        G_GREYLIST_DISABLED.store(false, std::sync::atomic::Ordering::Relaxed);
        self.flags = 0;
    }

    pub fn log(&self, msg: &str) {
        log::info!("linker: {}", msg);
    }

    pub fn is_enabled(&self, log_type: u32) -> bool {
        (self.flags & log_type) != 0
    }
}

pub static G_LINKER_LOGGER: std::sync::LazyLock<Mutex<LinkerLogger>> =
    std::sync::LazyLock::new(|| Mutex::new(LinkerLogger::new()));

pub fn parse_property(value: &str) -> u32 {
    if value.is_empty() {
        return 0;
    }
    let options = crate::base_strings::split(value, ",");
    let mut flags = 0u32;
    for o in &options {
        match o.as_str() {
            "dlerror" => flags |= K_LOG_ERRORS,
            "dlopen" => flags |= K_LOG_DLOPEN,
            "dlsym" => flags |= K_LOG_DLSYM,
            _ => log::warn!("Ignoring unknown debug.ld option \"{}\"", o),
        }
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize GDB tests that share global R_DEBUG / R_DEBUG_TAIL state.
    fn with_clean_gdb_state<F: FnOnce()>(f: F) {
        static GDB_TEST_MUTEX: std::sync::LazyLock<std::sync::Mutex<()>> =
            std::sync::LazyLock::new(|| std::sync::Mutex::new(()));
        let _lock = GDB_TEST_MUTEX.lock().unwrap();
        *R_DEBUG_TAIL.lock().unwrap() = 0;
        with_r_debug_mut(|rdebug| rdebug.r_map = std::ptr::null_mut());
        f();
    }

    #[test]
    fn test_link_map_insert_remove() {
        with_clean_gdb_state(|| {
            let mut map1 = LinkMap {
                l_addr: 0x1000,
                l_name: std::ptr::null(),
                l_ld: std::ptr::null_mut(),
                l_next: std::ptr::null_mut(),
                l_prev: std::ptr::null_mut(),
            };
            let mut map2 = LinkMap {
                l_addr: 0x2000,
                l_name: std::ptr::null(),
                l_ld: std::ptr::null_mut(),
                l_next: std::ptr::null_mut(),
                l_prev: std::ptr::null_mut(),
            };

            let p1: *mut LinkMap = &mut map1;
            let p2: *mut LinkMap = &mut map2;

            insert_link_map_into_debug_map(p1);
            insert_link_map_into_debug_map(p2);

            unsafe {
                assert!(!map1.l_next.is_null());
                assert!(!map2.l_prev.is_null());
                assert_eq!(map2.l_prev, p1);
            }

            remove_link_map_from_debug_map(p1);
            assert!(map2.l_prev.is_null());
        });
    }

    #[test]
    fn test_r_debug_initial_state() {
        with_clean_gdb_state(|| {
            with_r_debug_mut(|rdebug| {
                assert_eq!(rdebug.r_version, 1);
                assert_eq!(rdebug.r_state, RDebugState::RtConsistent);
                assert_ne!(rdebug.r_brk, 0);
            });
        });
    }

    #[test]
    fn test_notify_gdb_libraries_no_crash() {
        // Run in a fresh LazyLock context: access the init to ensure it's created,
        // then notify and check state.
        notify_gdb_of_libraries();
        with_clean_gdb_state(|| {
            notify_gdb_of_libraries();
            with_r_debug_mut(|rdebug| {
                assert_eq!(rdebug.r_state, RDebugState::RtConsistent);
            });
        });
    }

    #[test]
    fn test_parse_property() {
        assert_eq!(parse_property(""), 0);
        assert_eq!(parse_property("dlerror"), K_LOG_ERRORS);
        assert_eq!(parse_property("dlopen"), K_LOG_DLOPEN);
        assert_eq!(parse_property("dlsym"), K_LOG_DLSYM);
        assert_eq!(
            parse_property("dlerror,dlopen"),
            K_LOG_ERRORS | K_LOG_DLOPEN
        );
    }

    #[test]
    fn test_linker_logger() {
        let mut logger = LinkerLogger::new();
        assert!(!logger.is_enabled(K_LOG_ERRORS));

        logger.reset_state();
        assert!(!logger.is_enabled(K_LOG_ERRORS));
    }
}
