#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;
use crate::errno::__errno;

pub unsafe extern "C" fn open(path: *const c_char, flags: i32) -> i32 { libc::open(path, flags) }
pub unsafe extern "C" fn open64(path: *const c_char, flags: i32) -> i32 { libc::open64(path, flags) }
pub unsafe extern "C" fn close(fd: i32) -> i32 {
    let r = libc::close(fd);
    if r != 0 { *__errno() = *libc::__errno_location(); }
    r
}
pub unsafe extern "C" fn read(fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize {
    let r = libc::read(fd, buf, count);
    if r < 0 { *__errno() = *libc::__errno_location(); }
    r
}
pub unsafe extern "C" fn write(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
    let r = libc::write(fd, buf, count);
    if r < 0 { *__errno() = *libc::__errno_location(); }
    r
}
pub unsafe extern "C" fn pread(fd: i32, buf: *mut std::ffi::c_void, count: usize, offset: i64) -> isize { libc::pread(fd, buf, count, offset) }
pub unsafe extern "C" fn pwrite(fd: i32, buf: *const std::ffi::c_void, count: usize, offset: i64) -> isize { libc::pwrite(fd, buf, count, offset) }
pub unsafe extern "C" fn lseek(fd: i32, offset: i64, whence: i32) -> i64 { libc::lseek(fd, offset, whence) }
pub unsafe extern "C" fn lseek64(fd: i32, offset: i64, whence: i32) -> i64 { libc::lseek64(fd, offset, whence) }
pub unsafe extern "C" fn dup(fd: i32) -> i32 { libc::dup(fd) }
pub unsafe extern "C" fn dup2(oldfd: i32, newfd: i32) -> i32 { libc::dup2(oldfd, newfd) }
pub unsafe extern "C" fn pipe(pipefd: *mut i32) -> i32 { libc::pipe(pipefd) }
pub unsafe extern "C" fn access(path: *const c_char, mode: i32) -> i32 { libc::access(path, mode) }
pub unsafe extern "C" fn unlink(path: *const c_char) -> i32 { libc::unlink(path) }
pub unsafe extern "C" fn unlinkat(dirfd: i32, pathname: *const c_char, flags: i32) -> i32 { libc::unlinkat(dirfd, pathname, flags) }
pub unsafe extern "C" fn rmdir(path: *const c_char) -> i32 { libc::rmdir(path) }
pub unsafe extern "C" fn mkdir(path: *const c_char, mode: u32) -> i32 { libc::mkdir(path, mode) }
pub unsafe extern "C" fn link(oldpath: *const c_char, newpath: *const c_char) -> i32 { libc::link(oldpath, newpath) }
pub unsafe extern "C" fn symlink(target: *const c_char, linkpath: *const c_char) -> i32 { libc::symlink(target, linkpath) }
pub unsafe extern "C" fn readlink(path: *const c_char, buf: *mut c_char, bufsiz: usize) -> isize { libc::readlink(path, buf, bufsiz) }
pub unsafe extern "C" fn chdir(path: *const c_char) -> i32 { libc::chdir(path) }
pub unsafe extern "C" fn fchdir(fd: i32) -> i32 { libc::fchdir(fd) }
pub unsafe extern "C" fn getcwd(buf: *mut c_char, size: usize) -> *mut c_char { libc::getcwd(buf, size) }
pub unsafe extern "C" fn chown(path: *const c_char, owner: u32, group: u32) -> i32 { libc::chown(path, owner, group) }
pub unsafe extern "C" fn fchown(fd: i32, owner: u32, group: u32) -> i32 { libc::fchown(fd, owner, group) }
pub unsafe extern "C" fn lchown(path: *const c_char, owner: u32, group: u32) -> i32 { libc::lchown(path, owner, group) }
pub unsafe extern "C" fn getuid() -> u32 { libc::getuid() }
pub unsafe extern "C" fn getgid() -> u32 { libc::getgid() }
pub unsafe extern "C" fn geteuid() -> u32 { libc::geteuid() }
pub unsafe extern "C" fn getegid() -> u32 { libc::getegid() }
pub unsafe extern "C" fn getpid() -> i32 { libc::getpid() }
pub unsafe extern "C" fn getppid() -> i32 { libc::getppid() }
pub unsafe extern "C" fn getpgrp() -> i32 { libc::getpgrp() }
pub unsafe extern "C" fn fork() -> i32 { libc::fork() }
pub unsafe extern "C" fn vfork() -> i32 { libc::vfork() }
pub unsafe extern "C" fn execv(path: *const c_char, argv: *const *const c_char) -> i32 { libc::execv(path, argv) }
pub unsafe extern "C" fn execvp(file: *const c_char, argv: *const *const c_char) -> i32 { libc::execvp(file, argv) }
pub unsafe extern "C" fn execl(path: *const c_char, arg: *const c_char) -> i32 { libc::execl(path, arg) }
pub unsafe extern "C" fn execle(path: *const c_char, arg: *const c_char) -> i32 { libc::execle(path, arg) }
pub unsafe extern "C" fn execlp(file: *const c_char, arg: *const c_char) -> i32 { libc::execlp(file, arg) }
pub unsafe extern "C" fn isatty(fd: i32) -> i32 { libc::isatty(fd) }
pub unsafe extern "C" fn alarm(seconds: u32) -> u32 { libc::alarm(seconds) }
pub unsafe extern "C" fn sleep(seconds: u32) -> u32 { libc::sleep(seconds) }
pub unsafe extern "C" fn usleep(usec: u32) -> i32 { libc::usleep(usec) }
pub unsafe extern "C" fn pause() -> i32 { libc::pause() }
pub unsafe extern "C" fn fsync(fd: i32) -> i32 { libc::fsync(fd) }
pub unsafe extern "C" fn fdatasync(fd: i32) -> i32 { libc::fdatasync(fd) }
pub unsafe extern "C" fn sync() { libc::sync(); }
pub unsafe extern "C" fn gethostname(name: *mut c_char, len: usize) -> i32 { libc::gethostname(name, len) }
pub unsafe extern "C" fn sethostname(name: *const c_char, len: usize) -> i32 { libc::sethostname(name, len) }
pub unsafe extern "C" fn getpagesize() -> i32 { libc::sysconf(libc::_SC_PAGESIZE) as i32 }
pub unsafe extern "C" fn truncate(path: *const c_char, length: i64) -> i32 { libc::truncate(path, length) }
pub unsafe extern "C" fn ftruncate(fd: i32, length: i64) -> i32 { libc::ftruncate(fd, length) }
pub unsafe extern "C" fn nice(inc: i32) -> i32 { libc::nice(inc) }
pub unsafe extern "C" fn getdtablesize() -> i32 { libc::getdtablesize() }

pub unsafe extern "C" fn writev(fd: i32, iov: *const libc::iovec, iovcnt: i32) -> isize { libc::writev(fd, iov, iovcnt) }
pub unsafe extern "C" fn openat(dirfd: i32, path: *const c_char, flags: i32) -> i32 { libc::openat(dirfd, path, flags) }
pub unsafe extern "C" fn __read_chk(fd: i32, buf: *mut std::ffi::c_void, count: usize, _buf_len: usize) -> isize {
    let r = libc::read(fd, buf, count);
    if r < 0 { *__errno() = *libc::__errno_location(); }
    r
}
pub unsafe extern "C" fn __write_chk(fd: i32, buf: *const std::ffi::c_void, count: usize, _buf_len: usize) -> isize {
    let r = libc::write(fd, buf, count);
    if r < 0 { *__errno() = *libc::__errno_location(); }
    r
}
pub unsafe extern "C" fn __open_2(path: *const c_char, flags: i32) -> i32 { libc::open(path, flags) }
