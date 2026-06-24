use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::sync::OnceLock;

extern "C" {
    #[link_name = "stdin"]
    static __glibc_stdin: *mut libc::FILE;
    #[link_name = "stdout"]
    static __glibc_stdout: *mut libc::FILE;
    #[link_name = "stderr"]
    static __glibc_stderr: *mut libc::FILE;
}

struct SyncPtr(*mut BionicFile);
unsafe impl Send for SyncPtr {}
unsafe impl Sync for SyncPtr {}

const _IOEOF: i32 = 0x0020;

#[repr(C)]
pub struct BionicFile {
    _p: *const c_char,
    _r: c_int,
    _w: c_int,
    _flags: c_int,
    _file: c_int,
    wrapped: *mut libc::FILE,
    filler: [u8; 120],
}

const _ZERO_FILLER: [u8; 120] = [0u8; 120];

type VaList = *mut c_void;

static STANDARD_FILES: OnceLock<SyncPtr> = OnceLock::new();
static mut STDIN_PTR: *mut BionicFile = std::ptr::null_mut();
static mut STDOUT_PTR: *mut BionicFile = std::ptr::null_mut();
static mut STDERR_PTR: *mut BionicFile = std::ptr::null_mut();

pub unsafe fn init_standard_files() {
    if STANDARD_FILES.get().is_some() {
        return;
    }
    let arr: &mut [BionicFile; 3] =
        Box::leak(Box::new(MaybeUninit::zeroed().assume_init()));
    let base: *mut BionicFile = arr.as_mut_ptr();
    let stdin_str = c"stdin".as_ptr();
    let stdout_str = c"stdout".as_ptr();
    let stderr_str = c"stderr".as_ptr();
    (*base)._p = stdin_str;
    (*base).wrapped = __glibc_stdin;
    (*base.add(1))._p = stdout_str;
    (*base.add(1)).wrapped = __glibc_stdout;
    (*base.add(2))._p = stderr_str;
    (*base.add(2)).wrapped = __glibc_stderr;
    STDIN_PTR = base;
    STDOUT_PTR = base.add(1);
    STDERR_PTR = base.add(2);
    STANDARD_FILES.set(SyncPtr(base)).ok();
}

pub fn get_stdin_ptr_addr() -> *mut c_void {
    &raw mut STDIN_PTR as *mut *mut BionicFile as *mut c_void
}
pub fn get_stdout_ptr_addr() -> *mut c_void {
    &raw mut STDOUT_PTR as *mut *mut BionicFile as *mut c_void
}
pub fn get_stderr_ptr_addr() -> *mut c_void {
    &raw mut STDERR_PTR as *mut *mut BionicFile as *mut c_void
}
pub fn get_sf_addr() -> *mut c_void {
    STANDARD_FILES.get().unwrap().0 as *mut c_void
}

fn sf_base() -> *mut BionicFile {
    STANDARD_FILES.get().unwrap().0
}

fn is_standard_file(stream: *mut BionicFile) -> bool {
    let sf = sf_base();
    stream == sf || stream == unsafe { sf.add(1) } || stream == unsafe { sf.add(2) }
}

fn wrap_file(host_file: *mut libc::FILE) -> *mut BionicFile {
    if host_file.is_null() {
        return std::ptr::null_mut();
    }
    let bf = Box::new(BionicFile {
        _p: c"Internal".as_ptr(),
        _r: 0,
        _w: 0,
        _flags: 0,
        _file: unsafe { libc::fileno(host_file) },
        wrapped: host_file,
        filler: _ZERO_FILLER,
    });
    Box::into_raw(bf)
}

fn update_feof(stream: *mut BionicFile) {
    unsafe {
        let wrapped = (*stream).wrapped;
        (*stream)._flags = if libc::feof(wrapped) != 0 {
            _IOEOF
        } else {
            0
        };
    }
}

// Phase 1: Auto-arg-rewritten category — simple unwrap

pub unsafe extern "C" fn clearerr(stream: *mut BionicFile) {
    libc::clearerr((*stream).wrapped);
}
pub unsafe extern "C" fn feof(stream: *mut BionicFile) -> i32 {
    libc::feof((*stream).wrapped)
}
pub unsafe extern "C" fn ferror(stream: *mut BionicFile) -> i32 {
    libc::ferror((*stream).wrapped)
}
pub unsafe extern "C" fn fflush(stream: *mut BionicFile) -> i32 {
    libc::fflush((*stream).wrapped)
}
pub unsafe extern "C" fn fgetc(stream: *mut BionicFile) -> i32 {
    libc::fgetc((*stream).wrapped)
}
pub unsafe extern "C" fn fgets(
    s: *mut c_char,
    size: i32,
    stream: *mut BionicFile,
) -> *mut c_char {
    libc::fgets(s, size, (*stream).wrapped)
}
pub unsafe extern "C" fn fputc(c: i32, stream: *mut BionicFile) -> i32 {
    libc::fputc(c, (*stream).wrapped)
}
pub unsafe extern "C" fn fputs(s: *const c_char, stream: *mut BionicFile) -> i32 {
    libc::fputs(s, (*stream).wrapped)
}
pub unsafe extern "C" fn fwrite(
    buf: *const c_void,
    size: usize,
    count: usize,
    stream: *mut BionicFile,
) -> usize {
    libc::fwrite(buf, size, count, (*stream).wrapped)
}
pub unsafe extern "C" fn getc(stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn getc(stream: *mut libc::FILE) -> i32;
    }
    getc((*stream).wrapped)
}
pub unsafe extern "C" fn getc_unlocked(stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn getc_unlocked(stream: *mut libc::FILE) -> i32;
    }
    getc_unlocked((*stream).wrapped)
}
pub unsafe extern "C" fn putc(c: i32, stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn putc(c: i32, stream: *mut libc::FILE) -> i32;
    }
    putc(c, (*stream).wrapped)
}
pub unsafe extern "C" fn putc_unlocked(c: i32, stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn putc_unlocked(c: i32, stream: *mut libc::FILE) -> i32;
    }
    putc_unlocked(c, (*stream).wrapped)
}
pub unsafe extern "C" fn rewind(stream: *mut BionicFile) {
    libc::rewind((*stream).wrapped);
}
pub unsafe extern "C" fn setbuf(stream: *mut BionicFile, buf: *mut c_char) {
    libc::setbuf((*stream).wrapped, buf);
}
pub unsafe extern "C" fn setbuffer(
    stream: *mut BionicFile,
    buf: *mut c_char,
    size: usize,
) {
    extern "C" {
        fn setbuffer(stream: *mut libc::FILE, buf: *mut c_char, size: usize);
    }
    setbuffer((*stream).wrapped, buf, size);
}
pub unsafe extern "C" fn setlinebuf(stream: *mut BionicFile) {
    extern "C" {
        fn setlinebuf(stream: *mut libc::FILE);
    }
    setlinebuf((*stream).wrapped);
}
pub unsafe extern "C" fn ungetc(c: i32, stream: *mut BionicFile) -> i32 {
    libc::ungetc(c, (*stream).wrapped)
}
pub unsafe extern "C" fn fileno(stream: *mut BionicFile) -> i32 {
    libc::fileno((*stream).wrapped)
}
pub unsafe extern "C" fn flockfile(stream: *mut BionicFile) {
    extern "C" {
        fn flockfile(stream: *mut libc::FILE);
    }
    flockfile((*stream).wrapped);
}
pub unsafe extern "C" fn ftrylockfile(stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn ftrylockfile(stream: *mut libc::FILE) -> i32;
    }
    ftrylockfile((*stream).wrapped)
}
pub unsafe extern "C" fn funlockfile(stream: *mut BionicFile) {
    extern "C" {
        fn funlockfile(stream: *mut libc::FILE);
    }
    funlockfile((*stream).wrapped);
}
pub unsafe extern "C" fn getw(stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn getw(stream: *mut libc::FILE) -> i32;
    }
    getw((*stream).wrapped)
}
pub unsafe extern "C" fn putw(w: i32, stream: *mut BionicFile) -> i32 {
    extern "C" {
        fn putw(w: i32, stream: *mut libc::FILE) -> i32;
    }
    putw(w, (*stream).wrapped)
}
pub unsafe extern "C" fn fseek(
    stream: *mut BionicFile,
    offset: i64,
    whence: i32,
) -> i32 {
    libc::fseek((*stream).wrapped, offset, whence)
}
pub unsafe extern "C" fn ftell(stream: *mut BionicFile) -> i64 {
    libc::ftell((*stream).wrapped)
}
pub unsafe extern "C" fn getdelim(
    lineptr: *mut *mut c_char,
    n: *mut usize,
    delim: i32,
    stream: *mut BionicFile,
) -> isize {
    extern "C" {
        fn getdelim(
            lineptr: *mut *mut c_char,
            n: *mut usize,
            delim: i32,
            stream: *mut libc::FILE,
        ) -> isize;
    }
    getdelim(lineptr, n, delim, (*stream).wrapped)
}
pub unsafe extern "C" fn getline(
    lineptr: *mut *mut c_char,
    n: *mut usize,
    stream: *mut BionicFile,
) -> isize {
    extern "C" {
        fn getline(
            lineptr: *mut *mut c_char,
            n: *mut usize,
            stream: *mut libc::FILE,
        ) -> isize;
    }
    getline(lineptr, n, (*stream).wrapped)
}

pub unsafe extern "C" fn getwc(stream: *mut BionicFile) -> u32 {
    extern "C" {
        fn getwc(stream: *mut libc::FILE) -> u32;
    }
    getwc((*stream).wrapped)
}
pub unsafe extern "C" fn ungetwc(wc: u32, stream: *mut BionicFile) -> u32 {
    extern "C" {
        fn ungetwc(wc: u32, stream: *mut libc::FILE) -> u32;
    }
    ungetwc(wc, (*stream).wrapped)
}
pub unsafe extern "C" fn putwc(wc: u32, stream: *mut BionicFile) -> u32 {
    extern "C" {
        fn putwc(wc: u32, stream: *mut libc::FILE) -> u32;
    }
    putwc(wc, (*stream).wrapped)
}

// vfprintf / vfscanf — not variadic (takes va_list), pure Rust

pub unsafe extern "C" fn vfprintf(
    stream: *mut BionicFile,
    fmt: *const c_char,
    ap: VaList,
) -> i32 {
    extern "C" {
        fn vfprintf(stream: *mut libc::FILE, fmt: *const c_char, ap: VaList) -> i32;
    }
    vfprintf((*stream).wrapped, fmt, ap)
}
pub unsafe extern "C" fn vfscanf(
    stream: *mut BionicFile,
    fmt: *const c_char,
    ap: VaList,
) -> i32 {
    extern "C" {
        fn vfscanf(stream: *mut libc::FILE, fmt: *const c_char, ap: VaList) -> i32;
    }
    let wrapped = (*stream).wrapped;
    let ret = vfscanf(wrapped, fmt, ap);
    update_feof(stream);
    ret
}

// Phase 2: Wrapping/unwrapping functions

pub unsafe extern "C" fn fopen(
    path: *const c_char,
    mode: *const c_char,
) -> *mut BionicFile {
    wrap_file(libc::fopen(path, mode))
}
pub unsafe extern "C" fn fdopen(fd: i32, mode: *const c_char) -> *mut BionicFile {
    wrap_file(libc::fdopen(fd, mode))
}
pub unsafe extern "C" fn freopen(
    path: *const c_char,
    mode: *const c_char,
    stream: *mut BionicFile,
) -> *mut BionicFile {
    wrap_file(libc::freopen(path, mode, (*stream).wrapped))
}
pub unsafe extern "C" fn freopen64(
    path: *const c_char,
    mode: *const c_char,
    stream: *mut BionicFile,
) -> *mut BionicFile {
    freopen(path, mode, stream)
}
pub unsafe extern "C" fn tmpfile() -> *mut BionicFile {
    wrap_file(libc::tmpfile())
}
pub unsafe extern "C" fn popen(
    command: *const c_char,
    type_: *const c_char,
) -> *mut BionicFile {
    wrap_file(libc::popen(command, type_))
}
pub unsafe extern "C" fn fclose(stream: *mut BionicFile) -> i32 {
    let wrapped = (*stream).wrapped;
    let ret = libc::fclose(wrapped);
    if !is_standard_file(stream) {
        drop(Box::from_raw(stream));
    }
    ret
}
pub unsafe extern "C" fn pclose(stream: *mut BionicFile) -> i32 {
    let wrapped = (*stream).wrapped;
    let ret = libc::pclose(wrapped);
    drop(Box::from_raw(stream));
    ret
}

// Phase 3: Special logic functions

pub unsafe extern "C" fn fread(
    buf: *mut c_void,
    size: usize,
    count: usize,
    stream: *mut BionicFile,
) -> usize {
    let wrapped = (*stream).wrapped;
    let ret = libc::fread(buf, size, count, wrapped);
    update_feof(stream);
    ret
}

pub unsafe extern "C" fn fseeko(
    stream: *mut BionicFile,
    offset: i64,
    whence: i32,
) -> i32 {
    libc::fseeko((*stream).wrapped, offset, whence)
}
pub unsafe extern "C" fn ftello(stream: *mut BionicFile) -> i64 {
    libc::ftello((*stream).wrapped)
}

pub unsafe extern "C" fn __fgets_chk(
    dst: *mut c_char,
    len: i32,
    stream: *mut BionicFile,
    max_len: usize,
) -> *mut c_char {
    if len < 0 || (len as usize) > max_len {
        std::process::abort();
    }
    libc::fgets(dst, len, (*stream).wrapped)
}

// setvbuf is a stub (empty) — matching C++ behavior, since AutoArgRewritten(::setvbuf) crashes
pub unsafe extern "C" fn setvbuf(
    _stream: *mut BionicFile,
    _buf: *mut c_char,
    _mode: i32,
    _size: usize,
) -> i32 {
    0
}

// fputwc — real implementation (was stub in misc.rs)
pub unsafe extern "C" fn fputwc(wc: u32, stream: *mut BionicFile) -> u32 {
    extern "C" {
        fn fputwc(wc: u32, stream: *mut libc::FILE) -> u32;
    }
    fputwc(wc, (*stream).wrapped)
}

// Data symbol statics
pub static __isthreaded: c_int = 1;
