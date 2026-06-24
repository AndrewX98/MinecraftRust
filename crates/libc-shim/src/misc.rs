#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;

extern "C" {
    #[link_name = "random"]
    fn libc_random() -> i64;
    #[link_name = "srandom"]
    fn libc_srandom(seed: u32);
    #[link_name = "rand_r"]
    fn libc_rand_r(seedp: *mut u32) -> i32;
    #[link_name = "wcscpy"]
    fn libc_wcscpy(dst: *mut u32, src: *const u32) -> *mut u32;
    #[link_name = "wcsncpy"]
    fn libc_wcsncpy(dst: *mut u32, src: *const u32, n: usize) -> *mut u32;
    #[link_name = "wcscmp"]
    fn libc_wcscmp(s1: *const u32, s2: *const u32) -> i32;
    #[link_name = "wcscat"]
    fn libc_wcscat(dst: *mut u32, src: *const u32) -> *mut u32;
    #[link_name = "mblen"]
    fn libc_mblen(s: *const c_char, n: usize) -> i32;
    #[link_name = "mbtowc"]
    fn libc_mbtowc(pwc: *mut i32, s: *const c_char, n: usize) -> i32;
    #[link_name = "wctomb"]
    fn libc_wctomb(s: *mut c_char, wc: i32) -> i32;
    #[link_name = "mbstowcs"]
    fn libc_mbstowcs(dst: *mut i32, src: *const c_char, len: usize) -> usize;
    #[link_name = "wcstombs"]
    fn libc_wcstombs(dst: *mut c_char, src: *const i32, len: usize) -> usize;
    #[link_name = "wcsrtombs"]
    fn libc_wcsrtombs(dst: *mut c_char, src: *mut *const i32, len: usize, ps: *mut std::ffi::c_void) -> usize;
    #[link_name = "mbsrtowcs"]
    fn libc_mbsrtowcs(dst: *mut i32, src: *mut *const c_char, len: usize, ps: *mut std::ffi::c_void) -> usize;
    #[link_name = "_setjmp"]
    fn libc_setjmp(env: *mut crate::jmp_buf) -> i32;
    #[link_name = "longjmp"]
    fn libc_longjmp(env: *mut crate::jmp_buf, val: i32);
    #[link_name = "__sigsetjmp"]
    fn libc_sigsetjmp(env: *mut crate::sigjmp_buf, savesigs: i32) -> i32;
    #[link_name = "siglongjmp"]
    fn libc_siglongjmp(env: *mut crate::sigjmp_buf, val: i32);
    #[link_name = "__cxa_atexit"]
    fn glibc___cxa_atexit(func: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>, arg: *mut std::ffi::c_void, dso: *mut std::ffi::c_void) -> i32;
    #[link_name = "__cxa_finalize"]
    fn glibc___cxa_finalize(d: *mut std::ffi::c_void);
}

// random
pub unsafe extern "C" fn random() -> i64 { libc_random() }
pub unsafe extern "C" fn srandom(seed: u32) { libc_srandom(seed); }
pub unsafe extern "C" fn rand() -> i32 { libc::rand() }
pub unsafe extern "C" fn srand(seed: u32) { libc::srand(seed); }
pub unsafe extern "C" fn rand_r(seedp: *mut u32) -> i32 { libc_rand_r(seedp) }

// math
pub unsafe extern "C" fn isnan(d: f64) -> i32 { if d.is_nan() { 1 } else { 0 } }
pub unsafe extern "C" fn finite(d: f64) -> i32 { if d.is_finite() { 1 } else { 0 } }

// wchar
pub unsafe extern "C" fn wcslen(s: *const u32) -> usize { libc::wcslen(s as *const libc::wchar_t) as usize }
pub unsafe extern "C" fn wcscpy(dst: *mut u32, src: *const u32) -> *mut u32 { libc_wcscpy(dst, src) }
pub unsafe extern "C" fn wcsncpy(dst: *mut u32, src: *const u32, n: usize) -> *mut u32 { libc_wcsncpy(dst, src, n) }
pub unsafe extern "C" fn wcscmp(s1: *const u32, s2: *const u32) -> i32 { libc_wcscmp(s1, s2) }
pub unsafe extern "C" fn wcscat(dst: *mut u32, src: *const u32) -> *mut u32 { libc_wcscat(dst, src) }
pub unsafe extern "C" fn mblen(s: *const c_char, n: usize) -> i32 { libc_mblen(s, n) }
pub unsafe extern "C" fn mbtowc(pwc: *mut u32, s: *const c_char, n: usize) -> i32 { libc_mbtowc(pwc as *mut i32, s, n) }
pub unsafe extern "C" fn wctomb(s: *mut c_char, wc: u32) -> i32 { libc_wctomb(s, wc as i32) }
pub unsafe extern "C" fn mbstowcs(dst: *mut u32, src: *const c_char, len: usize) -> usize { libc_mbstowcs(dst as *mut i32, src, len) }
pub unsafe extern "C" fn wcstombs(dst: *mut c_char, src: *const u32, len: usize) -> usize { libc_wcstombs(dst, src as *const i32, len) }
pub unsafe extern "C" fn wcsrtombs(dst: *mut c_char, src: *mut *const u32, len: usize, ps: *mut std::ffi::c_void) -> usize { libc_wcsrtombs(dst, src as *mut *const i32, len, ps) }
pub unsafe extern "C" fn mbsrtowcs(dst: *mut u32, src: *mut *const c_char, len: usize, ps: *mut std::ffi::c_void) -> usize { libc_mbsrtowcs(dst as *mut i32, src, len, ps) }

// setjmp
pub unsafe extern "C" fn setjmp(env: *mut crate::jmp_buf) -> i32 { libc_setjmp(env) }
pub unsafe extern "C" fn longjmp(env: *mut crate::jmp_buf, val: i32) { libc_longjmp(env, val) }
pub unsafe extern "C" fn sigsetjmp(env: *mut crate::sigjmp_buf, savesigs: i32) -> i32 { libc_sigsetjmp(env, savesigs) }
pub unsafe extern "C" fn siglongjmp(env: *mut crate::sigjmp_buf, val: i32) { libc_siglongjmp(env, val) }

// cxa
#[allow(unused_variables)]
pub unsafe extern "C" fn __cxa_atexit(func: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>, arg: *mut std::ffi::c_void, dso: *mut std::ffi::c_void) -> i32 {
    glibc___cxa_atexit(func, arg, dso)
}
#[allow(unused_variables)]
pub unsafe extern "C" fn __cxa_finalize(d: *mut std::ffi::c_void) { glibc___cxa_finalize(d); }

// thread ID
pub unsafe extern "C" fn gettid() -> i32 { libc::syscall(libc::SYS_gettid) as i32 }
pub unsafe extern "C" fn getrandom(buf: *mut std::ffi::c_void, len: usize, _flags: u32) -> isize {
    libc::syscall(libc::SYS_getrandom, buf, len, 0u32) as isize
}
pub unsafe extern "C" fn arc4random_buf(buf: *mut std::ffi::c_void, len: usize) {
    getrandom(buf, len, 0);
}
pub unsafe extern "C" fn arc4random() -> u32 {
    let mut val: u32 = 0;
    getrandom(&mut val as *mut u32 as *mut std::ffi::c_void, core::mem::size_of::<u32>(), 0);
    val
}
pub unsafe extern "C" fn getentropy(buf: *mut std::ffi::c_void, len: usize) -> i32 {
    if len > 256 { return -1; }
    let mut remaining = len;
    let mut ptr = buf as *mut u8;
    while remaining > 0 {
        let chunk = if remaining > 256 { 256 } else { remaining };
        let ret = getrandom(ptr as *mut std::ffi::c_void, chunk, 0);
        if ret < 0 { return -1; }
        ptr = ptr.add(ret as usize);
        remaining -= ret as usize;
    }
    0
}

// dlfcn
pub unsafe extern "C" fn dlopen(filename: *const c_char, flag: i32) -> *mut std::ffi::c_void { libc::dlopen(filename, flag) }
pub unsafe extern "C" fn dlsym(handle: *mut std::ffi::c_void, symbol: *const c_char) -> *mut std::ffi::c_void { libc::dlsym(handle, symbol) }
pub unsafe extern "C" fn dlclose(handle: *mut std::ffi::c_void) -> i32 { libc::dlclose(handle) }
pub unsafe extern "C" fn dlerror() -> *mut c_char { libc::dlerror() }
pub unsafe extern "C" fn dladdr(addr: *const std::ffi::c_void, info: *mut libc::Dl_info) -> i32 { libc::dladdr(addr, info) }

// fnmatch
pub unsafe extern "C" fn fnmatch(pattern: *const c_char, string: *const c_char, flags: i32) -> i32 { libc::fnmatch(pattern, string, flags) }

// epoll
pub unsafe extern "C" fn epoll_create(size: i32) -> i32 { libc::epoll_create(size) }
pub unsafe extern "C" fn epoll_create1(flags: i32) -> i32 { libc::epoll_create1(flags) }
pub unsafe extern "C" fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut libc::epoll_event) -> i32 { libc::epoll_ctl(epfd, op, fd, event) }
pub unsafe extern "C" fn epoll_wait(epfd: i32, events: *mut libc::epoll_event, maxevents: i32, timeout: i32) -> i32 { libc::epoll_wait(epfd, events, maxevents, timeout) }

// eventfd
pub unsafe extern "C" fn eventfd(initval: u32, flags: i32) -> i32 { libc::eventfd(initval, flags) }

// semaphore
pub unsafe extern "C" fn sem_init(sem: *mut libc::sem_t, pshared: i32, value: u32) -> i32 { libc::sem_init(sem, pshared, value) }
pub unsafe extern "C" fn sem_destroy(sem: *mut libc::sem_t) -> i32 { libc::sem_destroy(sem) }
pub unsafe extern "C" fn sem_wait(sem: *mut libc::sem_t) -> i32 { libc::sem_wait(sem) }
pub unsafe extern "C" fn sem_timedwait(sem: *mut libc::sem_t, abs_timeout: *const libc::timespec) -> i32 { libc::sem_timedwait(sem, abs_timeout) }
pub unsafe extern "C" fn sem_post(sem: *mut libc::sem_t) -> i32 { libc::sem_post(sem) }

// sysconf
pub unsafe extern "C" fn sysconf(name: i32) -> i64 {
    let host_id = match name {
        0x0000 => libc::_SC_ARG_MAX, 0x0001 => libc::_SC_BC_BASE_MAX, 0x0002 => libc::_SC_BC_DIM_MAX,
        0x0003 => libc::_SC_BC_SCALE_MAX, 0x0004 => libc::_SC_BC_STRING_MAX, 0x0005 => libc::_SC_CHILD_MAX,
        0x0006 => libc::_SC_CLK_TCK, 0x0007 => libc::_SC_COLL_WEIGHTS_MAX, 0x0008 => libc::_SC_EXPR_NEST_MAX,
        0x0009 => libc::_SC_LINE_MAX, 0x000a => libc::_SC_NGROUPS_MAX, 0x000b => libc::_SC_OPEN_MAX,
        0x000c => libc::_SC_PASS_MAX, 0x000d => libc::_SC_2_C_BIND, 0x000e => libc::_SC_2_C_DEV,
        0x0010 => libc::_SC_2_CHAR_TERM, 0x0011 => libc::_SC_2_FORT_DEV, 0x0012 => libc::_SC_2_FORT_RUN,
        0x0013 => libc::_SC_2_LOCALEDEF, 0x0014 => libc::_SC_2_SW_DEV, 0x0015 => libc::_SC_2_UPE,
        0x0016 => libc::_SC_2_VERSION, 0x0017 => libc::_SC_JOB_CONTROL, 0x0018 => libc::_SC_SAVED_IDS,
        0x0019 => libc::_SC_VERSION, 0x001a => libc::_SC_RE_DUP_MAX, 0x001b => libc::_SC_STREAM_MAX,
        0x001c => libc::_SC_TZNAME_MAX, 0x001d => libc::_SC_XOPEN_CRYPT, 0x001e => libc::_SC_XOPEN_ENH_I18N,
        0x001f => libc::_SC_XOPEN_SHM, 0x0020 => libc::_SC_XOPEN_VERSION, 0x0022 => libc::_SC_XOPEN_REALTIME,
        0x0023 => libc::_SC_XOPEN_REALTIME_THREADS, 0x0024 => libc::_SC_XOPEN_LEGACY,
        0x0025 => libc::_SC_ATEXIT_MAX, 0x0026 => libc::_SC_IOV_MAX, 0x0027 => libc::_SC_PAGESIZE,
        0x0028 => libc::_SC_PAGE_SIZE, 0x0029 => libc::_SC_XOPEN_UNIX, 0x002e => libc::_SC_AIO_LISTIO_MAX,
        0x002f => libc::_SC_AIO_MAX, 0x0030 => libc::_SC_AIO_PRIO_DELTA_MAX, 0x0031 => libc::_SC_DELAYTIMER_MAX,
        0x0032 => libc::_SC_MQ_OPEN_MAX, 0x0033 => libc::_SC_MQ_PRIO_MAX, 0x0034 => libc::_SC_RTSIG_MAX,
        0x0035 => libc::_SC_SEM_NSEMS_MAX, 0x0036 => libc::_SC_SEM_VALUE_MAX, 0x0037 => libc::_SC_SIGQUEUE_MAX,
        0x0038 => libc::_SC_TIMER_MAX, 0x0039 => libc::_SC_ASYNCHRONOUS_IO, 0x003a => libc::_SC_FSYNC,
        0x003b => libc::_SC_MAPPED_FILES, 0x003c => libc::_SC_MEMLOCK, 0x003d => libc::_SC_MEMLOCK_RANGE,
        0x003e => libc::_SC_MEMORY_PROTECTION, 0x003f => libc::_SC_MESSAGE_PASSING,
        0x0040 => libc::_SC_PRIORITIZED_IO, 0x0041 => libc::_SC_PRIORITY_SCHEDULING,
        0x0042 => libc::_SC_REALTIME_SIGNALS, 0x0043 => libc::_SC_SEMAPHORES,
        0x0044 => libc::_SC_SHARED_MEMORY_OBJECTS, 0x0045 => libc::_SC_SYNCHRONIZED_IO,
        0x0046 => libc::_SC_TIMERS, 0x0047 => libc::_SC_GETGR_R_SIZE_MAX, 0x0048 => libc::_SC_GETPW_R_SIZE_MAX,
        0x0049 => libc::_SC_LOGIN_NAME_MAX, 0x004a => libc::_SC_THREAD_DESTRUCTOR_ITERATIONS,
        0x004b => libc::_SC_THREAD_KEYS_MAX, 0x004c => libc::_SC_THREAD_STACK_MIN,
        0x004d => libc::_SC_THREAD_THREADS_MAX, 0x004e => libc::_SC_TTY_NAME_MAX,
        0x004f => libc::_SC_THREADS, 0x0050 => libc::_SC_THREAD_ATTR_STACKADDR,
        0x0051 => libc::_SC_THREAD_ATTR_STACKSIZE, 0x0052 => libc::_SC_THREAD_PRIORITY_SCHEDULING,
        0x0053 => libc::_SC_THREAD_PRIO_INHERIT, 0x0054 => libc::_SC_THREAD_PRIO_PROTECT,
        0x0055 => libc::_SC_THREAD_SAFE_FUNCTIONS, 0x0060 => libc::_SC_NPROCESSORS_CONF,
        0x0061 => libc::_SC_NPROCESSORS_ONLN, 0x0062 => libc::_SC_PHYS_PAGES, 0x0063 => libc::_SC_AVPHYS_PAGES,
        0x0064 => libc::_SC_MONOTONIC_CLOCK, _ => return -1,
    };
    libc::sysconf(host_id)
}

// system_properties (stubs)
pub unsafe extern "C" fn __system_property_find(_name: *const c_char) -> *mut std::ffi::c_void { std::ptr::null_mut() }
pub unsafe extern "C" fn __system_property_get(_name: *const c_char, value: *mut c_char) -> i32 { *value = 0; 0 }
pub unsafe extern "C" fn __system_property_read_callback(_pi: *const std::ffi::c_void, callback: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char, *const c_char, u32)>, cookie: *mut std::ffi::c_void) {
    if let Some(cb) = callback {
        static EMPTY: [u8; 1] = [0];
        cb(cookie, EMPTY.as_ptr() as *const c_char, EMPTY.as_ptr() as *const c_char, 0);
    }
}

// misc forwards
pub unsafe extern "C" fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32 { libc::waitpid(pid, status, options) }
pub unsafe extern "C" fn chmod(path: *const c_char, mode: u32) -> i32 { libc::chmod(path, mode) }
pub unsafe extern "C" fn fchmod(fd: i32, mode: u32) -> i32 { libc::fchmod(fd, mode) }
pub unsafe extern "C" fn fchmodat(dirfd: i32, path: *const c_char, mode: u32, flags: i32) -> i32 { libc::fchmodat(dirfd, path, mode, flags) }
pub unsafe extern "C" fn umask(mask: u32) -> u32 { libc::umask(mask) }
pub unsafe extern "C" fn select(nfds: i32, readfds: *mut libc::fd_set, writefds: *mut libc::fd_set, exceptfds: *mut libc::fd_set, timeout: *mut libc::timeval) -> i32 { libc::select(nfds, readfds, writefds, exceptfds, timeout) }
pub unsafe extern "C" fn ioctl(fd: i32, request: u64) -> i32 { libc::ioctl(fd, request) }
pub unsafe extern "C" fn fcntl(fd: i32, cmd: i32) -> i32 { libc::fcntl(fd, cmd) }
pub unsafe extern "C" fn poll(fds: *mut libc::pollfd, nfds: u64, timeout: i32) -> i32 { libc::poll(fds, nfds as libc::nfds_t, timeout) }
pub unsafe extern "C" fn getrlimit(resource: u32, rlim: *mut libc::rlimit) -> i32 { libc::getrlimit(resource, rlim) }
pub unsafe extern "C" fn setrlimit(resource: u32, rlim: *const libc::rlimit) -> i32 { libc::setrlimit(resource, rlim) }
pub unsafe extern "C" fn getrusage(who: i32, usage: *mut libc::rusage) -> i32 { libc::getrusage(who, usage) }
pub unsafe extern "C" fn getpriority(which: u32, who: u32) -> i32 { libc::getpriority(which, who) }
pub unsafe extern "C" fn setpriority(which: u32, who: u32, prio: i32) -> i32 { libc::setpriority(which, who, prio) }
pub unsafe extern "C" fn sched_yield() -> i32 { libc::sched_yield() }
pub unsafe extern "C" fn sched_get_priority_min(policy: i32) -> i32 { libc::sched_get_priority_min(policy) }
pub unsafe extern "C" fn sched_get_priority_max(policy: i32) -> i32 { libc::sched_get_priority_max(policy) }
pub unsafe extern "C" fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const libc::cpu_set_t) -> i32 { libc::sched_setaffinity(pid, cpusetsize, mask) }
pub unsafe extern "C" fn sched_getaffinity(pid: i32, cpusetsize: usize, mask: *mut libc::cpu_set_t) -> i32 { libc::sched_getaffinity(pid, cpusetsize, mask) }
pub unsafe extern "C" fn openlog(ident: *const c_char, option: i32, facility: i32) { libc::openlog(ident, option, facility); }
pub unsafe extern "C" fn closelog() { libc::closelog(); }
pub unsafe extern "C" fn syslog(priority: i32, fmt: *const c_char) { libc::syslog(priority, fmt); }
pub unsafe extern "C" fn uname(buf: *mut libc::utsname) -> i32 { libc::uname(buf) }
pub unsafe extern "C" fn prctl(option: i32, a2: usize, a3: usize, a4: usize, a5: usize) -> i32 { libc::prctl(option, a2, a3, a4, a5) }
pub unsafe extern "C" fn lockf(fd: i32, cmd: i32, len: i64) -> i32 { libc::lockf(fd, cmd, len) }
pub unsafe extern "C" fn swab(src: *const std::ffi::c_void, dst: *mut std::ffi::c_void, nbytes: isize) {
    extern "C" { fn swab(src: *const std::ffi::c_void, dst: *mut std::ffi::c_void, nbytes: isize); }
    swab(src, dst, nbytes)
}
pub unsafe extern "C" fn pathconf(path: *const c_char, name: i32) -> i64 { libc::pathconf(path, name) }
pub unsafe extern "C" fn getauxval(type_: u64) -> usize { libc::getauxval(type_) as usize }
pub unsafe extern "C" fn tcgetattr(fd: i32, termios: *mut libc::termios) -> i32 { libc::tcgetattr(fd, termios) }
pub unsafe extern "C" fn tcsetattr(fd: i32, opt: i32, termios: *const libc::termios) -> i32 { libc::tcsetattr(fd, opt, termios) }
pub unsafe extern "C" fn getpwuid_r(uid: u32, pwd: *mut libc::passwd, buf: *mut c_char, buflen: usize, result: *mut *mut libc::passwd) -> i32 { libc::getpwuid_r(uid, pwd, buf, buflen, result) }
pub unsafe extern "C" fn __register_atfork(prepare: Option<unsafe extern "C" fn()>, parent: Option<unsafe extern "C" fn()>, child: Option<unsafe extern "C" fn()>, _dso: *mut std::ffi::c_void) -> i32 { libc::pthread_atfork(prepare, parent, child) }
pub unsafe extern "C" fn getifaddrs(ifap: *mut *mut std::ffi::c_void) -> i32 {
    extern "C" { fn getifaddrs(ifap: *mut *mut std::ffi::c_void) -> i32; }
    getifaddrs(ifap)
}
pub unsafe extern "C" fn freeifaddrs(ifa: *mut std::ffi::c_void) {
    extern "C" { fn freeifaddrs(ifa: *mut std::ffi::c_void); }
    freeifaddrs(ifa)
}

// drand48 family
pub unsafe extern "C" fn drand48() -> f64 {
    extern "C" { fn drand48() -> f64; }
    drand48()
}
pub unsafe extern "C" fn erand48(xsubi: *mut u16) -> f64 {
    extern "C" { fn erand48(xsubi: *mut u16) -> f64; }
    erand48(xsubi)
}
pub unsafe extern "C" fn lrand48() -> i64 {
    extern "C" { fn lrand48() -> i64; }
    lrand48()
}
pub unsafe extern "C" fn nrand48(xsubi: *mut u16) -> i64 {
    extern "C" { fn nrand48(xsubi: *mut u16) -> i64; }
    nrand48(xsubi)
}
pub unsafe extern "C" fn mrand48() -> i64 {
    extern "C" { fn mrand48() -> i64; }
    mrand48()
}
pub unsafe extern "C" fn jrand48(xsubi: *mut u16) -> i64 {
    extern "C" { fn jrand48(xsubi: *mut u16) -> i64; }
    jrand48(xsubi)
}
pub unsafe extern "C" fn srand48(seedval: i64) {
    extern "C" { fn srand48(seedval: i64); }
    srand48(seedval)
}
pub unsafe extern "C" fn seed48(seed16v: *mut u16) -> *mut u16 {
    extern "C" { fn seed48(seed16v: *mut u16) -> *mut u16; }
    seed48(seed16v)
}
pub unsafe extern "C" fn lcong48(param: *mut u16) {
    extern "C" { fn lcong48(param: *mut u16); }
    lcong48(param)
}
pub unsafe extern "C" fn initstate(seed: u32, state: *mut c_char, n: usize) -> *mut c_char {
    extern "C" { fn initstate(seed: u32, state: *mut c_char, n: usize) -> *mut c_char; }
    initstate(seed, state, n)
}
pub unsafe extern "C" fn setstate(state: *mut c_char) -> *mut c_char {
    extern "C" { fn setstate(state: *mut c_char) -> *mut c_char; }
    setstate(state)
}

// compiler-rt
pub unsafe extern "C" fn __divdi3(a: i64, b: i64) -> i64 { a / b }
pub unsafe extern "C" fn __udivdi3(a: u64, b: u64) -> u64 { a / b }
pub unsafe extern "C" fn __umoddi3(a: u64, b: u64) -> u64 { a % b }

// mallinfo
pub unsafe extern "C" fn mallinfo() -> libc::mallinfo { std::mem::zeroed() }

// FD_CHK
pub unsafe extern "C" fn __FD_CLR_chk(fd: i32, set: *mut libc::fd_set, _nfds: usize) { libc::FD_CLR(fd, set); }
pub unsafe extern "C" fn __FD_ISSET_chk(fd: i32, set: *const libc::fd_set, _nfds: usize) -> i32 { libc::FD_ISSET(fd, set) as i32 }
pub unsafe extern "C" fn __FD_SET_chk(fd: i32, set: *mut libc::fd_set, _nfds: usize) { libc::FD_SET(fd, set); }

// wchar strto*
pub unsafe extern "C" fn wcstol(s: *const u32, endptr: *mut *mut u32, base: i32) -> i64 {
    extern "C" { fn wcstol(s: *const u32, endptr: *mut *mut u32, base: i32) -> i64; }
    wcstol(s, endptr, base)
}
pub unsafe extern "C" fn wcstoul(s: *const u32, endptr: *mut *mut u32, base: i32) -> u64 {
    extern "C" { fn wcstoul(s: *const u32, endptr: *mut *mut u32, base: i32) -> u64; }
    wcstoul(s, endptr, base)
}
pub unsafe extern "C" fn wcstoll(s: *const u32, endptr: *mut *mut u32, base: i32) -> i64 {
    extern "C" { fn wcstoll(s: *const u32, endptr: *mut *mut u32, base: i32) -> i64; }
    wcstoll(s, endptr, base)
}
pub unsafe extern "C" fn wcstoull(s: *const u32, endptr: *mut *mut u32, base: i32) -> u64 {
    extern "C" { fn wcstoull(s: *const u32, endptr: *mut *mut u32, base: i32) -> u64; }
    wcstoull(s, endptr, base)
}
pub unsafe extern "C" fn wcstof(s: *const u32, endptr: *mut *mut u32) -> f32 {
    extern "C" { fn wcstof(s: *const u32, endptr: *mut *mut u32) -> f32; }
    wcstof(s, endptr)
}
pub unsafe extern "C" fn wcstod(s: *const u32, endptr: *mut *mut u32) -> f64 {
    extern "C" { fn wcstod(s: *const u32, endptr: *mut *mut u32) -> f64; }
    wcstod(s, endptr)
}
pub unsafe extern "C" fn wcstold(s: *const u32, endptr: *mut *mut u32) -> u128 {
    extern "C" { fn wcstold(s: *const u32, endptr: *mut *mut u32) -> u128; }
    wcstold(s, endptr)
}

// wchar mem*
pub unsafe extern "C" fn wmemchr(s: *const u32, c: u32, n: usize) -> *mut u32 {
    extern "C" { fn wmemchr(s: *const u32, c: u32, n: usize) -> *mut u32; }
    wmemchr(s, c, n)
}
pub unsafe extern "C" fn wmemcmp(s1: *const u32, s2: *const u32, n: usize) -> i32 {
    extern "C" { fn wmemcmp(s1: *const u32, s2: *const u32, n: usize) -> i32; }
    wmemcmp(s1, s2, n)
}
pub unsafe extern "C" fn wmemcpy(dst: *mut u32, src: *const u32, n: usize) -> *mut u32 {
    extern "C" { fn wmemcpy(dst: *mut u32, src: *const u32, n: usize) -> *mut u32; }
    wmemcpy(dst, src, n)
}
pub unsafe extern "C" fn wmemset(s: *mut u32, c: u32, n: usize) -> *mut u32 {
    extern "C" { fn wmemset(s: *mut u32, c: u32, n: usize) -> *mut u32; }
    wmemset(s, c, n)
}
pub unsafe extern "C" fn wmemmove(dst: *mut u32, src: *const u32, n: usize) -> *mut u32 {
    extern "C" { fn wmemmove(dst: *mut u32, src: *const u32, n: usize) -> *mut u32; }
    wmemmove(dst, src, n)
}

// wchar conversions
pub unsafe extern "C" fn wctob(wc: u32) -> i32 {
    extern "C" { fn wctob(wc: u32) -> i32; }
    wctob(wc)
}
pub unsafe extern "C" fn btowc(c: i32) -> u32 {
    extern "C" { fn btowc(c: i32) -> u32; }
    btowc(c)
}
pub unsafe extern "C" fn wctype(charset: *const c_char) -> usize {
    extern "C" { fn wctype(charset: *const c_char) -> usize; }
    wctype(charset)
}

// Stub: fputwc — FILE* function, return WEOF (-1) instead of aborting like C++ shim does
pub unsafe extern "C" fn fputwc(_wc: i32, _stream: *mut std::ffi::c_void) -> i32 { -1 }

// system_properties additional stubs
pub unsafe extern "C" fn __system_property_foreach(_name: *const c_char, _callback: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char, *const c_char, u32)>, _cookie: *mut std::ffi::c_void) -> i32 { 0 }
pub unsafe extern "C" fn __system_property_read(_pi: *const std::ffi::c_void, _name: *mut *const c_char, _value: *mut *const c_char) -> i32 { 0 }

// wide character type classification
pub unsafe extern "C" fn iswspace(wc: u32) -> i32 {
    extern "C" { fn iswspace(wc: u32) -> i32; }
    iswspace(wc)
}
pub unsafe extern "C" fn iswctype(wc: u32, desc: usize) -> i32 {
    extern "C" { fn iswctype(wc: u32, desc: usize) -> i32; }
    iswctype(wc, desc)
}
pub unsafe extern "C" fn towlower(wc: u32) -> u32 {
    extern "C" { fn towlower(wc: u32) -> u32; }
    towlower(wc)
}
pub unsafe extern "C" fn towupper(wc: u32) -> u32 {
    extern "C" { fn towupper(wc: u32) -> u32; }
    towupper(wc)
}
pub unsafe extern "C" fn iswlower(wc: u32) -> i32 {
    extern "C" { fn iswlower(wc: u32) -> i32; }
    iswlower(wc)
}
pub unsafe extern "C" fn iswprint(wc: u32) -> i32 {
    extern "C" { fn iswprint(wc: u32) -> i32; }
    iswprint(wc)
}
pub unsafe extern "C" fn iswblank(wc: u32) -> i32 {
    extern "C" { fn iswblank(wc: u32) -> i32; }
    iswblank(wc)
}
pub unsafe extern "C" fn iswcntrl(wc: u32) -> i32 {
    extern "C" { fn iswcntrl(wc: u32) -> i32; }
    iswcntrl(wc)
}
pub unsafe extern "C" fn iswupper(wc: u32) -> i32 {
    extern "C" { fn iswupper(wc: u32) -> i32; }
    iswupper(wc)
}
pub unsafe extern "C" fn iswalpha(wc: u32) -> i32 {
    extern "C" { fn iswalpha(wc: u32) -> i32; }
    iswalpha(wc)
}
pub unsafe extern "C" fn iswdigit(wc: u32) -> i32 {
    extern "C" { fn iswdigit(wc: u32) -> i32; }
    iswdigit(wc)
}
pub unsafe extern "C" fn iswpunct(wc: u32) -> i32 {
    extern "C" { fn iswpunct(wc: u32) -> i32; }
    iswpunct(wc)
}
pub unsafe extern "C" fn iswxdigit(wc: u32) -> i32 {
    extern "C" { fn iswxdigit(wc: u32) -> i32; }
    iswxdigit(wc)
}

// locale-aware wide character type (ignore locale param, use C locale)
pub unsafe extern "C" fn towlower_l(wc: u32, _l: *mut std::ffi::c_void) -> u32 { towlower(wc) }
pub unsafe extern "C" fn towupper_l(wc: u32, _l: *mut std::ffi::c_void) -> u32 { towupper(wc) }
pub unsafe extern "C" fn iswlower_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswlower(wc) }
pub unsafe extern "C" fn iswprint_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswprint(wc) }
pub unsafe extern "C" fn iswblank_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswblank(wc) }
pub unsafe extern "C" fn iswcntrl_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswcntrl(wc) }
pub unsafe extern "C" fn iswupper_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswupper(wc) }
pub unsafe extern "C" fn iswalpha_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswalpha(wc) }
pub unsafe extern "C" fn iswdigit_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswdigit(wc) }
pub unsafe extern "C" fn iswpunct_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswpunct(wc) }
pub unsafe extern "C" fn iswxdigit_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswxdigit(wc) }
pub unsafe extern "C" fn iswspace_l(wc: u32, _l: *mut std::ffi::c_void) -> i32 { iswspace(wc) }

// more wchar
pub unsafe extern "C" fn wcrtomb(s: *mut c_char, wc: u32, ps: *mut std::ffi::c_void) -> usize {
    extern "C" { fn wcrtomb(s: *mut c_char, wc: u32, ps: *mut std::ffi::c_void) -> usize; }
    wcrtomb(s, wc, ps)
}
pub unsafe extern "C" fn mbrtowc(pwc: *mut u32, s: *const c_char, n: usize, ps: *mut std::ffi::c_void) -> usize {
    extern "C" { fn mbrtowc(pwc: *mut u32, s: *const c_char, n: usize, ps: *mut std::ffi::c_void) -> usize; }
    mbrtowc(pwc, s, n, ps)
}
pub unsafe extern "C" fn wcscoll(s1: *const u32, s2: *const u32) -> i32 {
    extern "C" { fn wcscoll(s1: *const u32, s2: *const u32) -> i32; }
    wcscoll(s1, s2)
}
pub unsafe extern "C" fn wcsxfrm(dst: *mut u32, src: *const u32, n: usize) -> usize {
    extern "C" { fn wcsxfrm(dst: *mut u32, src: *const u32, n: usize) -> usize; }
    wcsxfrm(dst, src, n)
}
pub unsafe extern "C" fn mbsnrtowcs(dst: *mut u32, src: *mut *const c_char, nmc: usize, len: usize, ps: *mut std::ffi::c_void) -> usize {
    extern "C" { fn mbsnrtowcs(dst: *mut u32, src: *mut *const c_char, nmc: usize, len: usize, ps: *mut std::ffi::c_void) -> usize; }
    mbsnrtowcs(dst, src, nmc, len, ps)
}
pub unsafe extern "C" fn wcsnrtombs(dst: *mut c_char, src: *mut *const u32, nwc: usize, len: usize, ps: *mut std::ffi::c_void) -> usize {
    extern "C" { fn wcsnrtombs(dst: *mut c_char, src: *mut *const u32, nwc: usize, len: usize, ps: *mut std::ffi::c_void) -> usize; }
    wcsnrtombs(dst, src, nwc, len, ps)
}
pub unsafe extern "C" fn wcscoll_l(s1: *const u32, s2: *const u32, _l: *mut std::ffi::c_void) -> i32 { wcscoll(s1, s2) }
pub unsafe extern "C" fn wcsxfrm_l(dst: *mut u32, src: *const u32, n: usize, _l: *mut std::ffi::c_void) -> usize { wcsxfrm(dst, src, n) }
pub unsafe extern "C" fn mbrlen(s: *const c_char, n: usize, ps: *mut std::ffi::c_void) -> usize {
    extern "C" { fn mbrlen(s: *const c_char, n: usize, ps: *mut std::ffi::c_void) -> usize; }
    mbrlen(s, n, ps)
}
pub unsafe extern "C" fn wcsftime(s: *mut u32, maxsize: usize, fmt: *const u32, tm: *const libc::tm) -> usize {
    extern "C" { fn wcsftime(s: *mut u32, maxsize: usize, fmt: *const u32, tm: *const libc::tm) -> usize; }
    wcsftime(s, maxsize, fmt, tm)
}
