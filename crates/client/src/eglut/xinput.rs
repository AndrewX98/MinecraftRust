use std::ffi::{c_int, c_void};

pub const XIKeyPress: i32 = 1;
pub const XIButtonPress: i32 = 4;
pub const XIButtonRelease: i32 = 5;
pub const XIMotion: i32 = 6;
pub const XIFocusIn: i32 = 7;
pub const XIFocusOut: i32 = 8;
pub const XIEnter: i32 = 9;
pub const XILeave: i32 = 10;
pub const XITouchBegin: i32 = 18;
pub const XITouchUpdate: i32 = 19;
pub const XITouchEnd: i32 = 20;
pub const XIPropertyEvent: i32 = 16;

pub const XIEventMasks: i32 = 2;
pub const XIMasterPointer: i32 = 1;
pub const XIDeviceEnabled: i32 = 9;
pub const XIDeviceDisabled: i32 = 10;

pub const XISend: u8 = 1;
pub const XIModifier: u8 = 2;

pub const XIAllDevices: i32 = 0;

pub const XIGetFocus: u32 = 4;
pub const XIRawEvent: i32 = 14;

pub const XIRawMotion: i32 = 17;
pub const XIAllMasterDevices: i32 = 1;
pub const XI_KeyClass: i32 = 0;
pub const XI_ButtonClass: i32 = 1;
pub const XI_ValuatorClass: i32 = 2;
pub const XI_ScrollClass: i32 = 3;
pub const XI_TouchClass: i32 = 8;

pub const XI_RawKeyPress: i32 = 2;
pub const XI_RawKeyRelease: i32 = 3;
pub const XI_RawButtonPress: i32 = 11;
pub const XI_RawButtonRelease: i32 = 12;
pub const XI_RawMotion: i32 = 13;

pub const XISetFocus: u32 = 8;

#[repr(C)]
pub struct XIDeviceEvent {
    pub type_: c_int, pub serial: u64, pub send_event: bool, pub display: *mut c_void,
    pub extension: c_int, pub evtype: c_int, pub time_: u64,
    pub deviceid: c_int, pub sourceid: c_int, pub detail: c_int,
    pub root: u64, pub event: u64, pub child: u64,
    pub root_x: f64, pub root_y: f64, pub event_x: f64, pub event_y: f64,
    pub flags: c_int, pub buttons: *mut c_void, pub valuators: *mut c_void,
    pub mods: *mut c_void, pub group: *mut c_void,
}

#[repr(C)]
pub struct XIRawEvent {
    pub type_: c_int, pub serial: u64, pub send_event: bool, pub display: *mut c_void,
    pub extension: c_int, pub evtype: c_int, pub time_: u64,
    pub deviceid: c_int, pub sourceid: c_int, pub detail: c_int,
    pub flags: u32, pub valuators: *mut c_void, pub raw_values: *mut f64,
    pub num_values: c_int,
}

#[repr(C)]
pub struct XIEventMask {
    pub deviceid: c_int, pub mask_len: c_int, pub mask: *mut u8,
}

#[repr(C)]
pub struct XIAnyClassInfo {
    pub type_: c_int, pub sourceid: c_int,
}

#[repr(C)]
pub struct XIValuatorClassInfo {
    pub type_: c_int, pub sourceid: c_int, pub number: c_int, pub label: u64,
    pub min: f64, pub max: f64, pub value: f64, pub resolution: c_int, pub mode: c_int,
}

pub struct XInputRuntime {
    pub lib_xi2: Option<*mut libc::c_void>,
    pub xi2_available: bool,
    pub xi_opcode: c_int,
    pub xisuppress: Option<unsafe extern "C" fn(display: *mut c_void, win: u64, mask: c_int, /* XEvent* */ event: *mut c_void) -> c_int>,
    pub xiselect_events: Option<unsafe extern "C" fn(display: *mut c_void, win: u64, mask: *mut XIEventMask, num_masks: c_int) -> c_int>,
    pub xiquery_device: Option<unsafe extern "C" fn(display: *mut c_void, deviceid: c_int, ndevices: *mut c_int, avail: *mut *mut c_void) -> *mut c_void>,
    pub xifree_device: Option<unsafe extern "C" fn(data: *mut c_void)>,
    pub xiquery_extension: Option<unsafe extern "C" fn(display: *mut c_void, opcode: *mut c_int, event: *mut c_int, error: *mut c_int) -> bool>,
    pub xiget_property: Option<unsafe extern "C" fn(display: *mut c_void, deviceid: c_int, property: u64, offset: u64, length: u64, delete: bool, type_: u64, type_ret: *mut u64, format_ret: *mut c_int, nitems_ret: *mut u64, bytes_after_ret: *mut u64, data_ret: *mut *mut u8) -> c_int>,
    pub xiseti_focus: Option<unsafe extern "C" fn(display: *mut c_void, deviceid: c_int, win: u64, time_: u64)>,
    pub xigeti_focus: Option<unsafe extern "C" fn(display: *mut c_void, deviceid: c_int, focus_ret: *mut u64, time_ret: *mut u64) -> c_int>,
}

pub static mut XINPUT_RT: Option<XInputRuntime> = None;

pub unsafe fn raw_motion_to_relative(dx: *mut f64, dy: *mut f64) {
    if let Some(rt) = &XINPUT_RT {
        if !rt.xi2_available { return; }
    }
    // stubbed: XInput2 raw motion relative movement disabled
}
