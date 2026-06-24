#![allow(non_camel_case_types, unused)]

use std::ffi::{c_char, c_void};

type va_list = *mut c_void;

extern "C" {
    #[link_name = "vfprintf"]
    fn libc_vfprintf(stream: *mut libc::FILE, fmt: *const c_char, ap: va_list) -> i32;
    #[link_name = "vsprintf"]
    fn libc_vsprintf(buf: *mut c_char, fmt: *const c_char, ap: va_list) -> i32;
    #[link_name = "vsnprintf"]
    fn libc_vsnprintf(buf: *mut c_char, size: usize, fmt: *const c_char, ap: va_list) -> i32;
    #[link_name = "fscanf"]
    fn libc_fscanf(stream: *mut libc::FILE, fmt: *const c_char, ...) -> i32;
    #[link_name = "scanf"]
    fn libc_scanf(fmt: *const c_char, ...) -> i32;
    #[link_name = "sscanf"]
    fn libc_sscanf(s: *const c_char, fmt: *const c_char, ...) -> i32;
    #[link_name = "printf"]
    fn libc_printf(fmt: *const c_char, ...) -> i32;
    #[link_name = "fprintf"]
    fn libc_fprintf(stream: *mut libc::FILE, fmt: *const c_char, ...) -> i32;
    #[link_name = "sprintf"]
    fn libc_sprintf(buf: *mut c_char, fmt: *const c_char, ...) -> i32;
    #[link_name = "snprintf"]
    fn libc_snprintf(buf: *mut c_char, size: usize, fmt: *const c_char, ...) -> i32;
    #[link_name = "asprintf"]
    fn libc_asprintf(s: *mut *mut c_char, fmt: *const c_char, ...) -> i32;
    #[link_name = "vprintf"]
    fn libc_vprintf(fmt: *const c_char, ap: va_list) -> i32;
}

pub unsafe extern "C" fn fopen(path: *const c_char, mode: *const c_char) -> *mut libc::FILE { libc::fopen(path, mode) }
pub unsafe extern "C" fn fclose(stream: *mut libc::FILE) -> i32 { libc::fclose(stream) }
pub unsafe extern "C" fn fread(buf: *mut c_void, size: usize, count: usize, stream: *mut libc::FILE) -> usize { libc::fread(buf, size, count, stream) }
pub unsafe extern "C" fn fwrite(buf: *const c_void, size: usize, count: usize, stream: *mut libc::FILE) -> usize { libc::fwrite(buf, size, count, stream) }
pub unsafe extern "C" fn printf(fmt: *const c_char) -> i32 { libc_printf(fmt) }
pub unsafe extern "C" fn fprintf(stream: *mut libc::FILE, fmt: *const c_char) -> i32 { libc_fprintf(stream, fmt) }
pub unsafe extern "C" fn sprintf(buf: *mut c_char, fmt: *const c_char) -> i32 { libc_sprintf(buf, fmt) }
pub unsafe extern "C" fn snprintf(buf: *mut c_char, size: usize, fmt: *const c_char) -> i32 { libc_snprintf(buf, size, fmt) }
pub unsafe extern "C" fn vfprintf(stream: *mut libc::FILE, fmt: *const c_char, ap: va_list) -> i32 { libc_vfprintf(stream, fmt, ap) }
pub unsafe extern "C" fn vsprintf(buf: *mut c_char, fmt: *const c_char, ap: va_list) -> i32 { libc_vsprintf(buf, fmt, ap) }
pub unsafe extern "C" fn vsnprintf(buf: *mut c_char, size: usize, fmt: *const c_char, ap: va_list) -> i32 { libc_vsnprintf(buf, size, fmt, ap) }
pub unsafe extern "C" fn puts(s: *const c_char) -> i32 { libc::puts(s) }
pub unsafe extern "C" fn fputs(s: *const c_char, stream: *mut libc::FILE) -> i32 { libc::fputs(s, stream) }
pub unsafe extern "C" fn fgets(s: *mut c_char, size: i32, stream: *mut libc::FILE) -> *mut c_char { libc::fgets(s, size, stream) }
pub unsafe extern "C" fn getchar() -> i32 { libc::getchar() }
pub unsafe extern "C" fn putchar(c: i32) -> i32 { libc::putchar(c) }
pub unsafe extern "C" fn fflush(stream: *mut libc::FILE) -> i32 { libc::fflush(stream) }
pub unsafe extern "C" fn fflush_unlocked(stream: *mut libc::FILE) -> i32 {
    extern "C" { fn fflush_unlocked(stream: *mut libc::FILE) -> i32; }
    fflush_unlocked(stream)
}
pub unsafe extern "C" fn feof(stream: *mut libc::FILE) -> i32 { libc::feof(stream) }
pub unsafe extern "C" fn ferror(stream: *mut libc::FILE) -> i32 { libc::ferror(stream) }
pub unsafe extern "C" fn clearerr(stream: *mut libc::FILE) { libc::clearerr(stream); }
pub unsafe extern "C" fn remove(path: *const c_char) -> i32 { libc::remove(path) }
pub unsafe extern "C" fn rename(old: *const c_char, new: *const c_char) -> i32 { libc::rename(old, new) }
pub unsafe extern "C" fn tmpfile() -> *mut libc::FILE { libc::tmpfile() }
pub unsafe extern "C" fn fdopen(fd: i32, mode: *const c_char) -> *mut libc::FILE { libc::fdopen(fd, mode) }
pub unsafe extern "C" fn freopen(path: *const c_char, mode: *const c_char, stream: *mut libc::FILE) -> *mut libc::FILE { libc::freopen(path, mode, stream) }
pub unsafe extern "C" fn fgetc(stream: *mut libc::FILE) -> i32 { libc::fgetc(stream) }
pub unsafe extern "C" fn fputc(c: i32, stream: *mut libc::FILE) -> i32 { libc::fputc(c, stream) }
pub unsafe extern "C" fn vfscanf(stream: *mut libc::FILE, fmt: *const c_char, ap: *mut c_void) -> i32 {
    extern "C" { fn vfscanf(stream: *mut libc::FILE, fmt: *const c_char, ap: *mut c_void) -> i32; }
    vfscanf(stream, fmt, ap)
}
pub unsafe extern "C" fn fscanf(stream: *mut libc::FILE, fmt: *const c_char) -> i32 { libc_fscanf(stream, fmt) }
pub unsafe extern "C" fn scanf(fmt: *const c_char) -> i32 { libc_scanf(fmt) }
pub unsafe extern "C" fn sscanf(s: *const c_char, fmt: *const c_char) -> i32 { libc_sscanf(s, fmt) }
pub unsafe extern "C" fn fseek(stream: *mut libc::FILE, offset: i64, whence: i32) -> i32 { libc::fseek(stream, offset, whence) }
pub unsafe extern "C" fn ftell(stream: *mut libc::FILE) -> i64 { libc::ftell(stream) }
pub unsafe extern "C" fn fseeko(stream: *mut libc::FILE, offset: i64, whence: i32) -> i32 { libc::fseeko(stream, offset, whence) }
pub unsafe extern "C" fn ftello(stream: *mut libc::FILE) -> i64 { libc::ftello(stream) }
pub unsafe extern "C" fn rewind(stream: *mut libc::FILE) { libc::rewind(stream); }
pub unsafe extern "C" fn fileno(stream: *mut libc::FILE) -> i32 { libc::fileno(stream) }
pub unsafe extern "C" fn flockfile(stream: *mut libc::FILE) {
    extern "C" { fn flockfile(stream: *mut libc::FILE); }
    flockfile(stream)
}
pub unsafe extern "C" fn funlockfile(stream: *mut libc::FILE) {
    extern "C" { fn funlockfile(stream: *mut libc::FILE); }
    funlockfile(stream)
}
pub unsafe extern "C" fn setbuf(stream: *mut libc::FILE, buf: *mut c_char) { libc::setbuf(stream, buf); }
pub unsafe extern "C" fn setvbuf(stream: *mut libc::FILE, buf: *mut c_char, mode: i32, size: usize) -> i32 { libc::setvbuf(stream, buf, mode, size) }
pub unsafe extern "C" fn setbuffer(stream: *mut libc::FILE, buf: *mut c_char, size: usize) {
    extern "C" { fn setbuffer(stream: *mut libc::FILE, buf: *mut c_char, size: usize); }
    setbuffer(stream, buf, size)
}
pub unsafe extern "C" fn setlinebuf(stream: *mut libc::FILE) {
    extern "C" { fn setlinebuf(stream: *mut libc::FILE); }
    setlinebuf(stream)
}
pub unsafe extern "C" fn perror(s: *const c_char) { libc::perror(s) }
pub unsafe extern "C" fn vasprintf(s: *mut *mut c_char, fmt: *const c_char, ap: *mut c_void) -> i32 {
    extern "C" { fn vasprintf(s: *mut *mut c_char, fmt: *const c_char, ap: *mut c_void) -> i32; }
    vasprintf(s, fmt, ap)
}
pub unsafe extern "C" fn vscanf(fmt: *const c_char, ap: *mut c_void) -> i32 {
    extern "C" { fn vscanf(fmt: *const c_char, ap: *mut c_void) -> i32; }
    vscanf(fmt, ap)
}
pub unsafe extern "C" fn vsscanf(s: *const c_char, fmt: *const c_char, ap: *mut c_void) -> i32 {
    extern "C" { fn vsscanf(s: *const c_char, fmt: *const c_char, ap: *mut c_void) -> i32; }
    vsscanf(s, fmt, ap)
}
pub unsafe extern "C" fn popen(command: *const c_char, type_: *const c_char) -> *mut libc::FILE { libc::popen(command, type_) }
pub unsafe extern "C" fn pclose(stream: *mut libc::FILE) -> i32 { libc::pclose(stream) }
pub unsafe extern "C" fn asprintf(s: *mut *mut c_char, fmt: *const c_char) -> i32 { libc_asprintf(s, fmt) }
pub unsafe extern "C" fn vprintf(fmt: *const c_char, ap: va_list) -> i32 { libc_vprintf(fmt, ap) }
pub unsafe extern "C" fn __vsprintf_chk(buf: *mut c_char, _flags: i32, _dst_len: usize, fmt: *const c_char, ap: va_list) -> i32 { libc_vsprintf(buf, fmt, ap) }
pub unsafe extern "C" fn __snprintf_chk(buf: *mut c_char, size: usize, _flags: i32, _dst_len: usize, fmt: *const c_char) -> i32 { libc_snprintf(buf, size, fmt) }
pub unsafe extern "C" fn __vsnprintf_chk(buf: *mut c_char, size: usize, _flags: i32, _dst_len: usize, fmt: *const c_char, ap: va_list) -> i32 { libc_vsnprintf(buf, size, fmt, ap) }
pub unsafe extern "C" fn __fgets_chk(s: *mut c_char, size: i32, _buf_len: usize, stream: *mut libc::FILE) -> *mut c_char { libc::fgets(s, size, stream) }
