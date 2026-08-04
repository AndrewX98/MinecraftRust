//! Linux evdev joystick + udev hotplug manager
//! (ported from `linux_joystick.cpp` / `linux_joystick_manager.cpp`).

use std::cell::RefCell;
use std::ffi::{c_char, c_ulong, CStr, CString};
use std::os::fd::RawFd;
use std::rc::Rc;

use super::ffi::*;

/// Per-abs-axis calibration info (from `struct input_absinfo`).
#[derive(Clone, Copy)]
pub struct AxisInfo {
    pub index: i32,
    pub min: i32,
    pub max: i32,
    pub flat: i32,
    pub fuzz: i32,
}

/// An event decoded from a raw evdev frame, dispatched by the manager.
#[derive(Debug)]
pub enum JoystickEvent {
    Button { id: i32, state: bool },
    Axis { id: i32, value: f32 },
    Hat { id: i32, value: i32 },
}

/// A single evdev joystick device (per-device state + fd).
pub struct LinuxJoystick {
    dev_path: String,
    edev: *mut libevdev,
    fd: RawFd,
    buttons: Vec<i32>,
    axis: Vec<AxisInfo>,
    button_values: Vec<bool>,
    axis_values: Vec<f32>,
    hat_values: Vec<i32>,
}

fn is_hat(index: usize) -> bool {
    index >= ABS_HAT0X && index <= ABS_HAT3Y
}

impl LinuxJoystick {
    pub fn new(path: &str, edev: *mut libevdev, fd: RawFd) -> Result<LinuxJoystick, String> {
        let mut buttons = vec![-1i32; KEY_CNT];
        let mut axis = vec![AxisInfo { index: -1, min: 0, max: 0, flat: 0, fuzz: 0 }; ABS_CNT];

        // EVIOCGBIT(EV_KEY, ...)
        let key_size = (nlongs(KEY_CNT) * std::mem::size_of::<c_ulong>()) as c_ulong;
        let mut key_bits = vec![0u64; nlongs(KEY_CNT)];
        let r = unsafe { libc::ioctl(fd, eviocgbit(EV_KEY, key_size), key_bits.as_mut_ptr()) };
        if r < 0 {
            return Err("Failed to get joystick keys".to_string());
        }
        let len = (r as usize) * 8;
        let mut next_id = 0i32;
        // SDL first maps buttons after BTN_JOYSTICK, then the ones before it.
        for i in BTN_JOYSTICK..len + BTN_JOYSTICK {
            if test_bit(&key_bits, i % len) {
                buttons[i] = next_id;
                next_id += 1;
            }
        }

        // EVIOCGBIT(EV_ABS, ...)
        let abs_size = (nlongs(ABS_CNT) * std::mem::size_of::<c_ulong>()) as c_ulong;
        let mut abs_bits = vec![0u64; nlongs(ABS_CNT)];
        let r = unsafe { libc::ioctl(fd, eviocgbit(EV_ABS, abs_size), abs_bits.as_mut_ptr()) };
        if r < 0 {
            return Err("Failed to get joystick abs".to_string());
        }
        let len = (r as usize) * 8;
        next_id = 0;
        for i in 0..len {
            if !test_bit(&abs_bits, i) {
                axis[i].index = -1;
                continue;
            }
            let absinfo = unsafe { libevdev_get_abs_info(edev, i as u32) };
            if absinfo.is_null() {
                continue;
            }
            let id = if is_hat(i) { 0 } else { let id = next_id; next_id += 1; id };
            let a = unsafe { *absinfo };
            axis[i] = AxisInfo { index: id, min: a.minimum, max: a.maximum, flat: a.flat, fuzz: a.fuzz };
        }
        next_id = 0;
        for i in (ABS_HAT0X..=ABS_HAT3Y).step_by(2) {
            if axis[i].index == -1 && axis[i + 1].index == -1 {
                continue;
            }
            let id = next_id;
            next_id += 1;
            axis[i].index = id;
            axis[i + 1].index = id;
        }

        Ok(LinuxJoystick {
            dev_path: path.to_string(),
            edev,
            fd,
            buttons,
            axis,
            button_values: vec![false; KEY_CNT],
            axis_values: vec![0.0; ABS_CNT],
            hat_values: vec![0; HAT_COUNT],
        })
    }

    pub fn get_path(&self) -> &str {
        &self.dev_path
    }

    /// GUID as lowercase hex, 8 little-endian u16 fields (bustype/0/vendor/0/product/0/version/0).
    pub fn get_guid(&self) -> String {
        let mut s = String::with_capacity(16);
        let vals = [
            unsafe { libevdev_get_id_bustype(self.edev) },
            0,
            unsafe { libevdev_get_id_vendor(self.edev) },
            0,
            unsafe { libevdev_get_id_product(self.edev) },
            0,
            unsafe { libevdev_get_id_version(self.edev) },
            0,
        ];
        for v in vals {
            s.push_str(&format!("{:02x}{:02x}", v & 0xff, (v >> 8) & 0xff));
        }
        s
    }

    pub fn get_button(&self, index: i32) -> bool {
        if index < 0 || index as usize >= self.button_values.len() {
            return false;
        }
        self.button_values[index as usize]
    }

    pub fn get_axis(&self, index: i32) -> f32 {
        if index < 0 || index as usize >= self.axis_values.len() {
            return 0.0;
        }
        self.axis_values[index as usize]
    }

    pub fn get_hat(&self, index: i32) -> i32 {
        if index < 0 || index as usize >= self.hat_values.len() {
            return 0;
        }
        self.hat_values[index as usize]
    }

    /// Read pending evdev frames, updating internal state, returning decoded events.
    pub fn poll(&mut self) -> Vec<JoystickEvent> {
        let mut events = Vec::new();
        let mut e: input_event = unsafe { std::mem::zeroed() };
        loop {
            let r = unsafe { libevdev_next_event(self.edev, LIBEVDEV_READ_FLAG_NORMAL, &mut e) };
            if r == -EAGAIN {
                break;
            }
            if r != LIBEVDEV_READ_STATUS_SUCCESS {
                println!("LinuxJoystick::poll error");
                break;
            }
            if e.type_ == EV_KEY {
                if e.code as usize >= KEY_CNT {
                    continue;
                }
                let btn = self.buttons[e.code as usize];
                if btn == -1 {
                    continue;
                }
                let v = e.value != 0;
                self.button_values[btn as usize] = v;
                events.push(JoystickEvent::Button { id: btn, state: v });
            } else if e.type_ == EV_ABS && is_hat(e.code as usize) {
                let a = self.axis[e.code as usize];
                if a.index == -1 {
                    continue;
                }
                let y = (e.code & 1) != 0;
                let idx = a.index as usize;
                let mut v = self.hat_values[idx];
                // v (left, down, right, up)
                v &= !(if y { 0b0101 } else { 0b1010 });
                if e.value != 0 {
                    if y {
                        v |= if e.value > 0 { 4 } else { 1 };
                    } else {
                        v |= if e.value > 0 { 2 } else { 8 };
                    }
                }
                self.hat_values[idx] = v;
                events.push(JoystickEvent::Hat { id: a.index, value: v });
            } else if e.type_ == EV_ABS {
                if e.code as usize >= ABS_CNT {
                    continue;
                }
                let a = self.axis[e.code as usize];
                if a.index == -1 {
                    continue;
                }
                let iv = e.value - (a.min + a.max) / 2;
                let v = if iv >= 0 {
                    iv as f32 / (a.max - (a.min + a.max) / 2) as f32
                } else {
                    -(iv as f32) / (a.min - (a.min + a.max) / 2) as f32
                };
                let v = if iv.abs() < a.flat { 0.0 } else { v };
                let v = v.max(-1.0).min(1.0);
                self.axis_values[a.index as usize] = v;
                events.push(JoystickEvent::Axis { id: a.index, value: v });
            }
        }
        events
    }
}

impl Drop for LinuxJoystick {
    fn drop(&mut self) {
        if !self.edev.is_null() {
            unsafe { libevdev_free(self.edev) };
        }
        if self.fd >= 0 {
            unsafe { libc::close(self.fd) };
        }
    }
}

/// Callback lists subscribed by `GamepadManager` (replaces `CallbackList`).
pub struct JoystickCallbacks {
    pub on_connected: Vec<Box<dyn Fn(Rc<RefCell<LinuxJoystick>>)>>,
    pub on_disconnected: Vec<Box<dyn Fn(Rc<RefCell<LinuxJoystick>>)>>,
    pub on_button: Vec<Box<dyn Fn(Rc<RefCell<LinuxJoystick>>, i32, bool)>>,
    pub on_axis: Vec<Box<dyn Fn(Rc<RefCell<LinuxJoystick>>, i32, f32)>>,
    pub on_hat: Vec<Box<dyn Fn(Rc<RefCell<LinuxJoystick>>, i32, i32)>>,
}

impl Default for JoystickCallbacks {
    fn default() -> Self {
        JoystickCallbacks {
            on_connected: Vec::new(),
            on_disconnected: Vec::new(),
            on_button: Vec::new(),
            on_axis: Vec::new(),
            on_hat: Vec::new(),
        }
    }
}

/// udev hotplug manager owning all joystick devices.
pub struct LinuxJoystickManager {
    pub callbacks: JoystickCallbacks,
    udev: *mut udev,
    udev_monitor: *mut udev_monitor,
    udev_monitor_fd: RawFd,
    joysticks: Vec<Rc<RefCell<LinuxJoystick>>>,
}

impl Default for LinuxJoystickManager {
    fn default() -> Self {
        LinuxJoystickManager {
            callbacks: JoystickCallbacks::default(),
            udev: std::ptr::null_mut(),
            udev_monitor: std::ptr::null_mut(),
            udev_monitor_fd: -1,
            joysticks: Vec::new(),
        }
    }
}

impl Drop for LinuxJoystickManager {
    fn drop(&mut self) {
        if !self.udev_monitor.is_null() {
            unsafe { udev_monitor_unref(self.udev_monitor) };
        }
        if !self.udev.is_null() {
            unsafe { udev_unref(self.udev) };
        }
    }
}

impl LinuxJoystickManager {
    pub fn initialize(&mut self) {
        if self.udev.is_null() {
            self.udev = unsafe { udev_new() };
            if self.udev.is_null() {
                log::error!("gamepad: failed to initialize udev");
                return;
            }
        }

        self.udev_monitor = unsafe { udev_monitor_new_from_netlink(self.udev, c"udev".as_ptr()) };
        unsafe {
            udev_monitor_filter_add_match_subsystem_devtype(self.udev_monitor, c"input".as_ptr(), std::ptr::null());
            udev_monitor_enable_receiving(self.udev_monitor);
        }
        self.udev_monitor_fd = unsafe { udev_monitor_get_fd(self.udev_monitor) };

        let enumerate = unsafe { udev_enumerate_new(self.udev) };
        unsafe {
            udev_enumerate_add_match_subsystem(enumerate, c"input".as_ptr());
            udev_enumerate_scan_devices(enumerate);
        }
        let mut entry = unsafe { udev_enumerate_get_list_entry(enumerate) };
        while !entry.is_null() {
            let path = unsafe { udev_list_entry_get_name(entry) };
            let dev = unsafe { udev_device_new_from_syspath(self.udev, path) };
            if !dev.is_null() {
                self.on_device_added(dev);
                unsafe { udev_device_unref(dev) };
            }
            entry = unsafe { udev_list_entry_get_next(entry) };
        }
        unsafe { udev_enumerate_unref(enumerate) };
    }

    pub fn poll(&mut self) {
        self.poll_hotplug();

        let joysticks: Vec<Rc<RefCell<LinuxJoystick>>> = self.joysticks.clone();
        for js in &joysticks {
            let events = js.borrow_mut().poll();
            for ev in events {
                let rc = js.clone();
                match ev {
                    JoystickEvent::Button { id, state } => self.fire_button(rc, id, state),
                    JoystickEvent::Axis { id, value } => self.fire_axis(rc, id, value),
                    JoystickEvent::Hat { id, value } => self.fire_hat(rc, id, value),
                }
            }
        }
    }

    fn fire_connected(&self, js: Rc<RefCell<LinuxJoystick>>) {
        for cb in &self.callbacks.on_connected {
            cb(js.clone());
        }
    }

    fn fire_disconnected(&self, js: Rc<RefCell<LinuxJoystick>>) {
        for cb in &self.callbacks.on_disconnected {
            cb(js.clone());
        }
    }

    fn fire_button(&self, js: Rc<RefCell<LinuxJoystick>>, button: i32, state: bool) {
        for cb in &self.callbacks.on_button {
            cb(js.clone(), button, state);
        }
    }

    fn fire_axis(&self, js: Rc<RefCell<LinuxJoystick>>, axis: i32, value: f32) {
        for cb in &self.callbacks.on_axis {
            cb(js.clone(), axis, value);
        }
    }

    fn fire_hat(&self, js: Rc<RefCell<LinuxJoystick>>, hat: i32, value: i32) {
        for cb in &self.callbacks.on_hat {
            cb(js.clone(), hat, value);
        }
    }

    fn poll_hotplug(&mut self) {
        if self.udev_monitor.is_null() {
            return;
        }
        loop {
            let mut tv = libc::timeval { tv_sec: 0, tv_usec: 0 };
            let mut fds: libc::fd_set = unsafe { std::mem::zeroed() };
            unsafe {
                libc::FD_ZERO(&mut fds);
                libc::FD_SET(self.udev_monitor_fd, &mut fds);
            }
            let r = unsafe {
                libc::select(
                    self.udev_monitor_fd + 1,
                    &mut fds,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut tv,
                )
            };
            if r <= 0 || !unsafe { libc::FD_ISSET(self.udev_monitor_fd, &fds) } {
                break;
            }
            let dev = unsafe { udev_monitor_receive_device(self.udev_monitor) };
            if dev.is_null() {
                break;
            }
            let action = unsafe { udev_device_get_action(dev) };
            let action = if action.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(action).to_string_lossy().into_owned() }
            };
            if action == "add" {
                self.on_device_added(dev);
            } else if action == "remove" {
                self.on_device_removed(dev);
            }
            unsafe { udev_device_unref(dev) };
        }
    }

    fn on_device_added(&mut self, dev: *mut udev_device) {
        let val = unsafe { udev_device_get_property_value(dev, c"ID_INPUT_JOYSTICK".as_ptr()) };
        if val.is_null() {
            return;
        }
        let is_joystick = unsafe { CStr::from_ptr(val).to_string_lossy() } == "1";
        if !is_joystick {
            return;
        }
        let devnode = unsafe { udev_device_get_devnode(dev) };
        if devnode.is_null() {
            return;
        }
        let path = unsafe { CStr::from_ptr(devnode).to_string_lossy().into_owned() };

        let cpath = match CString::new(path.clone()) {
            Ok(p) => p,
            Err(_) => return,
        };
        let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            return;
        }
        let mut edev: *mut libevdev = std::ptr::null_mut();
        let err = unsafe { libevdev_new_from_fd(fd, &mut edev) };
        if err != 0 {
            println!("libevdev_new_from_fd error {} ({})", err, path);
            unsafe { libc::close(fd) };
            return;
        }

        match LinuxJoystick::new(&path, edev, fd) {
            Ok(js) => {
                let rc = Rc::new(RefCell::new(js));
                self.fire_connected(rc.clone());
                self.joysticks.push(rc);
            }
            Err(e) => {
                log::warn!("gamepad: failed to open {}: {}", path, e);
                unsafe { libevdev_free(edev) };
                unsafe { libc::close(fd) };
            }
        }
    }

    fn on_device_removed(&mut self, dev: *mut udev_device) {
        let devnode = unsafe { udev_device_get_devnode(dev) };
        if devnode.is_null() {
            return;
        }
        let path = unsafe { CStr::from_ptr(devnode).to_string_lossy() };
        for i in 0..self.joysticks.len() {
            if self.joysticks[i].borrow().dev_path == path.as_ref() {
                let js = self.joysticks.remove(i);
                self.fire_disconnected(js);
                return;
            }
        }
    }
}
