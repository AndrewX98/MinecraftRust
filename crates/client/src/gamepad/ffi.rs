//! Raw FFI bindings for libudev and libevdev (both linked in client/build.rs).

use std::ffi::{c_char, c_int, c_uint, c_ulong, c_ushort};

// --- Opaque udev types ---
pub struct udev {
    _private: [u8; 0],
}
pub struct udev_monitor {
    _private: [u8; 0],
}
pub struct udev_enumerate {
    _private: [u8; 0],
}
pub struct udev_list_entry {
    _private: [u8; 0],
}
pub struct udev_device {
    _private: [u8; 0],
}

// --- Opaque libevdev type ---
pub struct libevdev {
    _private: [u8; 0],
}

/// `struct input_absinfo` (linux/input.h) — six 32-bit fields.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct input_absinfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

/// `struct input_event` (linux/input.h) on x86_64:
/// `struct timeval` (two longs) + u16 type + u16 code + i32 value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct input_event {
    pub time: libc::timeval,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

// Event types (linux/input-event-codes.h)
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;

// Key / abs ranges
pub const KEY_CNT: usize = 0x300; // KEY_MAX + 1 = 0x2ff + 1
pub const BTN_JOYSTICK: usize = 0x120;
pub const ABS_CNT: usize = 0x40; // ABS_MAX + 1 = 0x3f + 1
pub const ABS_HAT0X: usize = 0x10;
pub const ABS_HAT3Y: usize = 0x17;
pub const HAT_COUNT: usize = (ABS_HAT3Y + 1 - ABS_HAT0X) / 2;

// libevdev
pub const LIBEVDEV_READ_FLAG_NORMAL: c_uint = 2;
pub const LIBEVDEV_READ_STATUS_SUCCESS: c_int = 0;
pub const EAGAIN: c_int = 11;

const IOC_READ: c_ulong = 2;
const IOCTL_TYPE: c_ulong = b'E' as c_ulong;

/// `EVIOCGBIT(ev, len)` — `_IOC(_IOC_READ, 'E', 0x20 + (ev), len)`.
pub fn eviocgbit(ev: u16, size: c_ulong) -> c_ulong {
    ((IOC_READ << 30) | (size << 16) | (IOCTL_TYPE << 8) | (0x20 + ev as c_ulong)) as c_ulong
}

/// Number of `unsigned long` words needed for `x` bits (matches C `NLONGS`).
pub fn nlongs(x: usize) -> usize {
    let long_bits = std::mem::size_of::<c_ulong>() * 8;
    (x - 1) / long_bits + 1
}

extern "C" {
    // libudev
    pub fn udev_new() -> *mut udev;
    pub fn udev_unref(udev: *mut udev);
    pub fn udev_monitor_new_from_netlink(udev: *mut udev, name: *const c_char) -> *mut udev_monitor;
    pub fn udev_monitor_unref(udev_monitor: *mut udev_monitor);
    pub fn udev_monitor_filter_add_match_subsystem_devtype(
        udev_monitor: *mut udev_monitor,
        subsystem: *const c_char,
        devtype: *const c_char,
    ) -> c_int;
    pub fn udev_monitor_enable_receiving(udev_monitor: *mut udev_monitor) -> c_int;
    pub fn udev_monitor_get_fd(udev_monitor: *mut udev_monitor) -> c_int;
    pub fn udev_monitor_receive_device(udev_monitor: *mut udev_monitor) -> *mut udev_device;
    pub fn udev_enumerate_new(udev: *mut udev) -> *mut udev_enumerate;
    pub fn udev_enumerate_unref(enumerate: *mut udev_enumerate);
    pub fn udev_enumerate_add_match_subsystem(
        enumerate: *mut udev_enumerate,
        subsystem: *const c_char,
    ) -> c_int;
    pub fn udev_enumerate_scan_devices(enumerate: *mut udev_enumerate) -> c_int;
    pub fn udev_enumerate_get_list_entry(enumerate: *mut udev_enumerate) -> *mut udev_list_entry;
    pub fn udev_list_entry_get_name(list_entry: *mut udev_list_entry) -> *const c_char;
    pub fn udev_list_entry_get_next(list_entry: *mut udev_list_entry) -> *mut udev_list_entry;
    pub fn udev_device_new_from_syspath(udev: *mut udev, syspath: *const c_char) -> *mut udev_device;
    pub fn udev_device_unref(dev: *mut udev_device);
    pub fn udev_device_get_property_value(dev: *mut udev_device, key: *const c_char) -> *const c_char;
    pub fn udev_device_get_devnode(dev: *mut udev_device) -> *const c_char;
    pub fn udev_device_get_action(dev: *mut udev_device) -> *const c_char;

    // libevdev
    pub fn libevdev_new_from_fd(fd: c_int, dev: *mut *mut libevdev) -> c_int;
    pub fn libevdev_free(dev: *mut libevdev);
    pub fn libevdev_get_fd(dev: *const libevdev) -> c_int;
    pub fn libevdev_next_event(
        dev: *mut libevdev,
        flags: c_uint,
        ev: *mut input_event,
    ) -> c_int;
    pub fn libevdev_get_abs_info(dev: *const libevdev, code: c_uint) -> *const input_absinfo;
    pub fn libevdev_get_id_bustype(dev: *const libevdev) -> c_ushort;
    pub fn libevdev_get_id_vendor(dev: *const libevdev) -> c_ushort;
    pub fn libevdev_get_id_product(dev: *const libevdev) -> c_ushort;
    pub fn libevdev_get_id_version(dev: *const libevdev) -> c_ushort;
}

#[allow(dead_code)]
pub fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

/// Read a bit at `index` from a `Vec<c_ulong>` bit array (LSB-first per word).
pub fn test_bit(bits: &[c_ulong], index: usize) -> bool {
    let long_bits = std::mem::size_of::<c_ulong>() * 8;
    let word = index / long_bits;
    if word >= bits.len() {
        return false;
    }
    (bits[word] & (1u64 << (index % long_bits))) != 0
}
