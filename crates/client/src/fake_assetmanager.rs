//! Rust port of `fake_assetmanager.cpp` (the `libandroid.so` AAsset hooks).
//!
//! 1:1 port of the manifest `fake_assetmanager.cpp` (203 lines): the game
//! caches the manager pointer from `ANativeActivity.assetManager` (set from
//! `fake_assetmanager_get_instance`) and calls the `AAsset*` functions
//! through the `libandroid.so` stub registered in `mc_setup_android_hooks`.
//!
//! The `AAsset`/`AAssetDir` structs are opaque to the game (only handled via
//! pointer), so their Rust layout is free-form. `AAssetDir` uses the libc
//! `DIR*`/`readdir` iteration to match the C++ implementation exactly.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Arc, Mutex};

struct FakeAssetManager {
    root_dir: String,
}

enum AssetData {
    Vec(Vec<u8>),
    Mmap { ptr: *mut u8, len: usize },
}
// SAFETY: mmap ptr is valid for len and owned by AAsset
unsafe impl Send for AssetData {}

struct AAsset {
    data: AssetData,
    offset: i64,
}

impl AAsset {
    fn len(&self) -> usize {
        match &self.data {
            AssetData::Vec(v) => v.len(),
            AssetData::Mmap { len, .. } => *len,
        }
    }
    fn as_ptr(&self) -> *const u8 {
        match &self.data {
            AssetData::Vec(v) => v.as_ptr(),
            AssetData::Mmap { ptr, .. } => *ptr as *const u8,
        }
    }
}

const MMAP_THRESHOLD: usize = 64 * 1024;
static ASSET_CACHE: std::sync::LazyLock<Mutex<HashMap<String, Arc<Vec<u8>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

unsafe fn try_mmap(path: &str) -> Option<(*mut u8, usize)> {
    let c_path = CString::new(path).ok()?;
    let fd = libc::open(c_path.as_ptr(), libc::O_RDONLY);
    if fd < 0 {
        return None;
    }
    let mut st: libc::stat = std::mem::zeroed();
    if libc::fstat(fd, &mut st) != 0 {
        libc::close(fd);
        return None;
    }
    let len = st.st_size as usize;
    if len == 0 {
        libc::close(fd);
        return None;
    }
    let ptr = libc::mmap(
        std::ptr::null_mut(),
        len,
        libc::PROT_READ,
        libc::MAP_PRIVATE,
        fd,
        0,
    );
    libc::close(fd);
    if ptr == libc::MAP_FAILED {
        return None;
    }
    Some((ptr as *mut u8, len))
}

struct AAssetDir {
    dir: *mut libc::DIR,
    current_file_name: CString,
}

// Replaces `static AAssetManager *g_assetManager` in the C++.
static GLOBAL_ASSET_MANAGER: Mutex<Option<Box<FakeAssetManager>>> = Mutex::new(None);

fn global_instance_ptr() -> *mut FakeAssetManager {
    GLOBAL_ASSET_MANAGER
        .lock()
        .unwrap()
        .as_ref()
        .map(|b| Box::as_ref(b) as *const FakeAssetManager as *mut FakeAssetManager)
        .unwrap_or(std::ptr::null_mut())
}

// ============================================================
// AAsset_* hooks (registered into the libandroid.so symbol map)
// ============================================================

unsafe extern "C" fn AAssetManager_open(
    amgr: *mut FakeAssetManager,
    filename: *const c_char,
    _mode: c_int,
) -> *mut AAsset {
    if filename.is_null() {
        return std::ptr::null_mut();
    }
    let name = CStr::from_ptr(filename).to_string_lossy().into_owned();
    if name.is_empty() || name.starts_with('/') {
        return std::ptr::null_mut();
    }
    // dedup cache (logical name key)
    if let Some(arc) = ASSET_CACHE.lock().unwrap().get(&name).cloned() {
        if !arc.is_empty() {
            return Box::into_raw(Box::new(AAsset { data: AssetData::Vec((*arc).clone()), offset: 0 }));
        }
    }
    let full_path = format!("{}{}", (*amgr).root_dir, name);
    // mmap large files to avoid copy + page on demand
    let data = match std::fs::metadata(&full_path) {
        Ok(m) if (m.len() as usize) > MMAP_THRESHOLD => {
            if let Some((ptr, len)) = try_mmap(&full_path) {
                AssetData::Mmap { ptr, len }
            } else {
                match std::fs::read(&full_path) {
                    Ok(c) if !c.is_empty() => AssetData::Vec(c),
                    _ => return std::ptr::null_mut(),
                }
            }
        }
        Ok(_) => match std::fs::read(&full_path) {
            Ok(c) if !c.is_empty() => AssetData::Vec(c),
            _ => return std::ptr::null_mut(),
        },
        Err(_) => return std::ptr::null_mut(),
    };
    if data_len(&data) == 0 {
        if let AssetData::Mmap { ptr, len } = data {
            libc::munmap(ptr as *mut libc::c_void, len);
        }
        return std::ptr::null_mut();
    }
    // populate cache for small Vec assets
    if let AssetData::Vec(ref v) = data {
        if v.len() < 1024 * 1024 {
            let mut cache = ASSET_CACHE.lock().unwrap();
            if cache.len() > 128 {
                cache.clear();
            }
            cache.insert(name.clone(), Arc::new(v.clone()));
        }
    }
    Box::into_raw(Box::new(AAsset { data, offset: 0 }))
}

fn data_len(d: &AssetData) -> usize {
    match d { AssetData::Vec(v) => v.len(), AssetData::Mmap { len, .. } => *len }
}

unsafe extern "C" fn AAssetManager_openDir(
    amgr: *mut FakeAssetManager,
    dirname: *const c_char,
) -> *mut AAssetDir {
    if dirname.is_null() {
        return std::ptr::null_mut();
    }
    let name = CStr::from_ptr(dirname).to_string_lossy().into_owned();
    if name.is_empty() || name.starts_with('/') {
        return std::ptr::null_mut();
    }
    let full_path = format!("{}{}", (*amgr).root_dir, name);
    let c_path = CString::new(full_path).unwrap();
    let d = libc::opendir(c_path.as_ptr());
    if d.is_null() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(AAssetDir {
        dir: d,
        current_file_name: CString::new("").unwrap(),
    }))
}

unsafe extern "C" fn AAsset_close(asset: *mut AAsset) {
    if !asset.is_null() {
        let b = Box::from_raw(asset);
        if let AssetData::Mmap { ptr, len } = b.data {
            libc::munmap(ptr as *mut libc::c_void, len);
        }
        // Vec dropped normally
    }
}

unsafe extern "C" fn AAsset_isAllocated(_asset: *mut AAsset) -> c_int {
    1
}

unsafe extern "C" fn AAsset_read(asset: *mut AAsset, buf: *mut c_void, count: usize) -> isize {
    if asset.is_null() {
        return 0;
    }
    let a = &mut *asset;
    let total = a.len() as i64;
    if a.offset > total {
        return 0;
    }
    let max_len = total - a.offset;
    let mut count = count as i64;
    if count > max_len {
        count = max_len;
    }
    if count == 0 {
        return 0;
    }
    std::ptr::copy_nonoverlapping(
        a.as_ptr().add(a.offset as usize),
        buf as *mut u8,
        count as usize,
    );
    a.offset += count;
    count as isize
}

unsafe extern "C" fn AAsset_seek64(asset: *mut AAsset, offset: i64, whence: c_int) -> i64 {
    let a = &mut *asset;
    let cur_pos = a.offset;
    let max_pos = a.len() as i64;
    let new_offset = match whence {
        libc::SEEK_SET => offset,
        libc::SEEK_CUR => cur_pos + offset,
        libc::SEEK_END => max_pos + offset,
        _ => return -1,
    };
    if new_offset < 0 || new_offset > max_pos {
        return -1;
    }
    a.offset = new_offset;
    new_offset
}

unsafe extern "C" fn AAsset_seek(asset: *mut AAsset, offset: i64, whence: c_int) -> i64 {
    AAsset_seek64(asset, offset, whence)
}

unsafe extern "C" fn AAsset_getLength64(asset: *mut AAsset) -> i64 {
    (*asset).len() as i64
}

unsafe extern "C" fn AAsset_getLength(asset: *mut AAsset) -> i64 {
    AAsset_getLength64(asset)
}

unsafe extern "C" fn AAsset_getRemainingLength64(asset: *mut AAsset) -> i64 {
    let a = &*asset;
    (a.len() as i64 - a.offset).max(0)
}

unsafe extern "C" fn AAsset_getRemainingLength(asset: *mut AAsset) -> i64 {
    AAsset_getRemainingLength64(asset)
}

unsafe extern "C" fn AAsset_getBuffer(asset: *mut AAsset) -> *const c_void {
    (*asset).as_ptr() as *const c_void
}

unsafe extern "C" fn AAssetDir_close(asset_dir: *mut AAssetDir) {
    if !asset_dir.is_null() {
        let d = &mut *asset_dir;
        if !d.dir.is_null() {
            libc::closedir(d.dir);
        }
        drop(Box::from_raw(asset_dir));
    }
}

unsafe extern "C" fn AAssetDir_rewind(asset_dir: *mut AAssetDir) {
    if !asset_dir.is_null() {
        libc::rewinddir((*asset_dir).dir);
    }
}

unsafe extern "C" fn AAssetDir_getNextFileName(asset_dir: *mut AAssetDir) -> *const c_char {
    if asset_dir.is_null() {
        return std::ptr::null();
    }
    let d = &mut *asset_dir;
    let ent = libc::readdir(d.dir);
    if ent.is_null() {
        return std::ptr::null();
    }
    let name = CStr::from_ptr((*ent).d_name.as_ptr())
        .to_string_lossy()
        .into_owned();
    if name == "." || name == ".." {
        return AAssetDir_getNextFileName(asset_dir);
    }
    d.current_file_name = CString::new(name).unwrap();
    d.current_file_name.as_ptr()
}

unsafe extern "C" fn AAssetManager_fromJava(
    _env: *mut c_void,
    _asset_manager_obj: *mut c_void,
) -> *mut c_void {
    global_instance_ptr() as *mut c_void
}

// ============================================================
// Instance accessors (called from Rust startup.rs / jni_support.rs)
// ============================================================

/// Replaces the C++ `extern "C" void* fake_assetmanager_get_instance()`.
#[no_mangle]
pub unsafe extern "C" fn fake_assetmanager_get_instance() -> *mut c_void {
    global_instance_ptr() as *mut c_void
}

/// Replaces the C++ `extern "C" void fake_assetmanager_create_and_set_global(const char*)`.
#[no_mangle]
pub unsafe extern "C" fn fake_assetmanager_create_and_set_global(root_dir: *const c_char) {
    if root_dir.is_null() {
        return;
    }
    let mut dir = CStr::from_ptr(root_dir).to_string_lossy().into_owned();
    if !dir.is_empty() && !dir.ends_with('/') {
        dir.push('/');
    }
    let mgr = FakeAssetManager { root_dir: dir };
    *GLOBAL_ASSET_MANAGER.lock().unwrap() = Some(Box::new(mgr));
}

// ============================================================
// Hook registration (replaces FakeAssetManager::initHybrisHooks)
// ============================================================

/// Insert all `libandroid.so` asset hooks into the symbol map. Called from
/// `startup::setup_android_hooks` (the C++ `mc_register_android_hook` bridge and
/// the `*mut c_void` map argument are gone — the Rust map is passed directly).
pub unsafe fn mc_register_fake_asset_manager_hooks(map: &mut HashMap<String, *mut c_void>) {
    map.insert("AAssetManager_open".to_string(), AAssetManager_open as *mut c_void);
    map.insert("AAssetManager_openDir".to_string(), AAssetManager_openDir as *mut c_void);
    map.insert("AAssetManager_fromJava".to_string(), AAssetManager_fromJava as *mut c_void);
    map.insert("AAsset_close".to_string(), AAsset_close as *mut c_void);
    map.insert("AAsset_isAllocated".to_string(), AAsset_isAllocated as *mut c_void);
    map.insert("AAsset_read".to_string(), AAsset_read as *mut c_void);
    map.insert("AAsset_seek64".to_string(), AAsset_seek64 as *mut c_void);
    map.insert("AAsset_seek".to_string(), AAsset_seek as *mut c_void);
    map.insert("AAsset_getLength64".to_string(), AAsset_getLength64 as *mut c_void);
    map.insert("AAsset_getLength".to_string(), AAsset_getLength as *mut c_void);
    map.insert("AAsset_getRemainingLength64".to_string(), AAsset_getRemainingLength64 as *mut c_void);
    map.insert("AAsset_getRemainingLength".to_string(), AAsset_getRemainingLength as *mut c_void);
    map.insert("AAsset_getBuffer".to_string(), AAsset_getBuffer as *mut c_void);
    map.insert("AAssetDir_close".to_string(), AAssetDir_close as *mut c_void);
    map.insert("AAssetDir_rewind".to_string(), AAssetDir_rewind as *mut c_void);
    map.insert("AAssetDir_getNextFileName".to_string(), AAssetDir_getNextFileName as *mut c_void);
}
