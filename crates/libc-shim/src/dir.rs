#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;
use std::ffi::CStr;

extern "C" {
    #[link_name = "scandir"]
    fn libc_scandir(dirp: *const c_char, namelist: *mut *mut *mut libc::dirent, filter: Option<unsafe extern "C" fn(*const libc::dirent) -> i32>, compar: Option<unsafe extern "C" fn(*const *const libc::dirent, *const *const libc::dirent) -> i32>) -> i32;
    #[link_name = "alphasort"]
    fn libc_alphasort(a: *const *const libc::dirent, b: *const *const libc::dirent) -> i32;
    #[link_name = "versionsort"]
    fn libc_versionsort(a: *const *const libc::dirent, b: *const *const libc::dirent) -> i32;
}

pub unsafe extern "C" fn opendir(path: *const c_char) -> *mut libc::DIR { libc::opendir(path) }
pub unsafe extern "C" fn closedir(dirp: *mut libc::DIR) -> i32 { libc::closedir(dirp) }
pub unsafe extern "C" fn readdir(dirp: *mut libc::DIR) -> *mut libc::dirent { libc::readdir(dirp) }
pub unsafe extern "C" fn readdir_r(dirp: *mut libc::DIR, entry: *mut libc::dirent, result: *mut *mut libc::dirent) -> i32 { libc::readdir_r(dirp, entry, result) }
pub unsafe extern "C" fn rewinddir(dirp: *mut libc::DIR) { libc::rewinddir(dirp); }
pub unsafe extern "C" fn seekdir(dirp: *mut libc::DIR, loc: i64) { libc::seekdir(dirp, loc); }
pub unsafe extern "C" fn telldir(dirp: *mut libc::DIR) -> i64 { libc::telldir(dirp) }
pub unsafe extern "C" fn scandir(dirp: *const c_char, namelist: *mut *mut *mut libc::dirent, filter: Option<unsafe extern "C" fn(*const libc::dirent) -> i32>, compar: Option<unsafe extern "C" fn(*const *const libc::dirent, *const *const libc::dirent) -> i32>) -> i32 {
    libc_scandir(dirp, namelist, filter, compar)
}
pub unsafe extern "C" fn alphasort(a: *const *const libc::dirent, b: *const *const libc::dirent) -> i32 { libc_alphasort(a, b) }
#[cfg(not(target_os = "macos"))]
pub unsafe extern "C" fn versionsort(a: *const *const libc::dirent, b: *const *const libc::dirent) -> i32 { libc_versionsort(a, b) }
// glibc-only; approximate strverscmp over d_name
#[cfg(target_os = "macos")]
pub unsafe extern "C" fn versionsort(a: *const *const libc::dirent, b: *const *const libc::dirent) -> i32 {
    let na = CStr::from_ptr((**a).d_name.as_ptr()).to_bytes();
    let nb = CStr::from_ptr((**b).d_name.as_ptr()).to_bytes();
    let mut i = 0;
    while i < na.len() && i < nb.len() {
        let (ca, cb) = (na[i], nb[i]);
        if ca.is_ascii_digit() && cb.is_ascii_digit() && ca != cb {
            // compare the full digit runs numerically
            let da: u64 = na[i..].iter().take_while(|c| c.is_ascii_digit()).fold(0u64, |acc, c| acc * 10 + (c - b'0') as u64);
            let db: u64 = nb[i..].iter().take_while(|c| c.is_ascii_digit()).fold(0u64, |acc, c| acc * 10 + (c - b'0') as u64);
            return match da.cmp(&db) { std::cmp::Ordering::Less => -1, std::cmp::Ordering::Greater => 1, std::cmp::Ordering::Equal => 0 };
        }
        if ca != cb { return ca as i32 - cb as i32; }
        i += 1;
    }
    (na.len() as i32) - (nb.len() as i32)
}
pub unsafe extern "C" fn mkdtemp(template: *mut c_char) -> *mut c_char { libc::mkdtemp(template) }
pub unsafe extern "C" fn fdopendir(fd: i32) -> *mut libc::DIR { libc::fdopendir(fd) }
pub unsafe extern "C" fn dirfd(dirp: *mut libc::DIR) -> i32 { libc::dirfd(dirp) }
