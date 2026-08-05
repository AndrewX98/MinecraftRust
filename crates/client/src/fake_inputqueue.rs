//! Port of the C++ `FakeInputQueue` (Phase 1 of PORT_FAKE_LOOPER.md).
//!
//! Rust owns the event storage and the `libandroid.so` input hooks. The C++
//! `FakeInputQueue` class (member of `FakeLooper`) remains as a thin forwarding
//! wrapper so C++ `WindowCallbacks`/`FakeLooper` keep constructing events with
//! the C++ struct types; those get copied byte-for-byte into the Rust queues
//! here (the struct layouts are pinned by the unit tests in this module).

use std::collections::VecDeque;
use std::ffi::c_void;

// Matches android/input.h.
#[allow(dead_code)]
pub const AINPUT_EVENT_TYPE_KEY: i32 = 1;
#[allow(dead_code)]
pub const AINPUT_EVENT_TYPE_MOTION: i32 = 2;
#[allow(dead_code)]
pub const AINPUT_SOURCE_KEYBOARD: i32 = 0x101;

/// Base of both event types. Layout matches C++ `FakeInputEvent`:
/// source@0, type@4, deviceId@8 (size 12).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FakeInputEvent {
    pub source: i32,
    pub r#type: i32,
    pub device_id: i32,
}

/// Layout matches C++ `FakeKeyEvent`: base + action@12, keyCode@16, metaState@20
/// (size 24).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FakeKeyEvent {
    pub base: FakeInputEvent,
    pub action: i32,
    pub key_code: i32,
    pub meta_state: i32,
}

/// Layout matches C++ `FakeMotionEvent`: base + action@12, pointerId@16, x@20,
/// y@24, axis-slot@32 (32 bytes opaque — was `std::function`), btn@64, dy@68
/// (size 72). The axis slot is never interpreted: gamepad events with a real
/// axis lambda are delivered through `jni_support_send_motion_event` instead.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FakeMotionEvent {
    pub base: FakeInputEvent,
    pub action: i32,
    pub pointer_id: i32,
    pub x: f32,
    pub y: f32,
    pub axis_pad: [u8; 4],
    pub axis_slot: [u8; 32],
    pub btn: i32,
    pub dy: i32,
}

impl FakeInputEvent {
    #[allow(dead_code)]
    pub const fn new(source: i32, r#type: i32, device_id: i32) -> Self {
        FakeInputEvent { source, r#type, device_id }
    }
}

impl FakeKeyEvent {
    #[allow(dead_code)]
    pub const fn new(action: i32, key_code: i32, meta_state: i32) -> Self {
        FakeKeyEvent {
            base: FakeInputEvent::new(AINPUT_SOURCE_KEYBOARD, AINPUT_EVENT_TYPE_KEY, 0),
            action,
            key_code,
            meta_state,
        }
    }

    #[allow(dead_code)]
    pub const fn new_source(source: i32, device_id: i32, action: i32, key_code: i32) -> Self {
        FakeKeyEvent {
            base: FakeInputEvent::new(source, AINPUT_EVENT_TYPE_KEY, device_id),
            action,
            key_code,
            meta_state: 0,
        }
    }

    #[allow(dead_code)]
    pub const fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

impl FakeMotionEvent {
    #[allow(dead_code)]
    pub const fn new(source: i32, action: i32, pointer_id: i32, x: f32, y: f32) -> Self {
        FakeMotionEvent {
            base: FakeInputEvent::new(source, AINPUT_EVENT_TYPE_MOTION, 0),
            action,
            pointer_id,
            x,
            y,
            axis_pad: [0; 4],
            axis_slot: [0; 32],
            btn: 0,
            dy: 0,
        }
    }

    #[allow(dead_code)]
    pub const fn new_button(
        source: i32,
        action: i32,
        pointer_id: i32,
        x: f32,
        y: f32,
        btn: i32,
        dy: i32,
    ) -> Self {
        FakeMotionEvent {
            base: FakeInputEvent::new(source, AINPUT_EVENT_TYPE_MOTION, 0),
            action,
            pointer_id,
            x,
            y,
            axis_pad: [0; 4],
            axis_slot: [0; 32],
            btn,
            dy,
        }
    }

    #[allow(dead_code)]
    pub const fn default() -> Self {
        Self::new(0, 0, 0, 0.0, 0.0)
    }
}

/// Rust-owned storage. Mirrors the C++ deques (preallocated 100, key checked
/// before motion on get).
pub struct FakeInputQueue {
    key_events: VecDeque<FakeKeyEvent>,
    motion_events: VecDeque<FakeMotionEvent>,
}

impl FakeInputQueue {
    pub fn new() -> Self {
        FakeInputQueue {
            key_events: VecDeque::with_capacity(100),
            motion_events: VecDeque::with_capacity(100),
        }
    }

    pub fn has_events(&self) -> bool {
        !self.key_events.is_empty() || !self.motion_events.is_empty()
    }

    /// Returns 0 and stores the front-event pointer in `out_event`, or -1 when
    /// empty (leaving `out_event` untouched).
    fn get_event(&mut self, out_event: *mut *mut c_void) -> i32 {
        if out_event.is_null() {
            return -1;
        }
        if let Some(front) = self.key_events.front_mut() {
            unsafe { *out_event = front as *mut FakeKeyEvent as *mut c_void };
            return 0;
        }
        if let Some(front) = self.motion_events.front_mut() {
            unsafe { *out_event = front as *mut FakeMotionEvent as *mut c_void };
            return 0;
        }
        -1
    }

    /// Pops the front event if `event` is its address; returns false otherwise.
    fn finish_event(&mut self, event: *mut c_void) -> bool {
        if event.is_null() {
            return false;
        }
        if let Some(front) = self.key_events.front() {
            if front as *const FakeKeyEvent as *const c_void == event {
                self.key_events.pop_front();
                return true;
            }
        }
        if let Some(front) = self.motion_events.front() {
            if front as *const FakeMotionEvent as *const c_void == event {
                self.motion_events.pop_front();
                return true;
            }
        }
        false
    }

    fn add_key_event(&mut self, event: FakeKeyEvent) {
        self.key_events.push_back(event);
    }

    fn add_motion_event(&mut self, event: FakeMotionEvent) {
        self.motion_events.push_back(event);
    }
}

// C++ FFI accessors (defined in fake_inputqueue_stub.cpp) plus the android-hook
// registration helper shared with fake_looper.rs.
extern "C" {
    fn mc_register_android_hook(map: *mut c_void, name: *const i8, fn_ptr: *mut c_void);
    /// Returns the Rust `FakeInputQueue*` held by the C++ wrapper.
    fn fake_input_queue_get_rust(queue: *mut c_void) -> *mut c_void;
}

// ============================================================
// C++-consumed FFI (thin wrapper methods + mc_setup_android_hooks)
// ============================================================

#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_create() -> *mut FakeInputQueue {
    Box::into_raw(Box::new(FakeInputQueue::new()))
}

#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_destroy(queue: *mut FakeInputQueue) {
    if !queue.is_null() {
        drop(Box::from_raw(queue));
    }
}

#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_has_events(queue: *mut FakeInputQueue) -> bool {
    if queue.is_null() {
        return false;
    }
    unsafe { (&*queue).has_events() }
}

#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_get_event(
    queue: *mut FakeInputQueue,
    out_event: *mut *mut c_void,
) -> i32 {
    if queue.is_null() {
        return -1;
    }
    unsafe { (&mut *queue).get_event(out_event) }
}

/// Returns 0 on success, nonzero if `event` was not the front of the queue
/// (the C++ wrapper throws `std::runtime_error` on that).
#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_finish_event(
    queue: *mut FakeInputQueue,
    event: *mut c_void,
) -> i32 {
    if queue.is_null() {
        return 1;
    }
    if unsafe { (&mut *queue).finish_event(event) } { 0 } else { 1 }
}

#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_add_key_event(
    queue: *mut FakeInputQueue,
    event: *const FakeKeyEvent,
) {
    if queue.is_null() || event.is_null() {
        return;
    }
    unsafe { (&mut *queue).add_key_event(*event) };
}

#[no_mangle]
pub unsafe extern "C" fn mc_fake_input_queue_add_motion_event(
    queue: *mut FakeInputQueue,
    event: *const FakeMotionEvent,
) {
    if queue.is_null() || event.is_null() {
        return;
    }
    unsafe { (&mut *queue).add_motion_event(*event) };
}

// ============================================================
// libandroid.so hooks (registered by mc_register_fake_input_queue_hooks)
// ============================================================

fn rust_queue_of(queue: *mut c_void) -> *mut FakeInputQueue {
    if queue.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { fake_input_queue_get_rust(queue) as *mut FakeInputQueue }
}

unsafe extern "C" fn hook_input_queue_get_event(
    queue: *mut c_void,
    out_event: *mut *mut c_void,
) -> i32 {
    mc_fake_input_queue_get_event(rust_queue_of(queue), out_event)
}

unsafe extern "C" fn hook_input_queue_finish_event(queue: *mut c_void, event: *mut c_void, _handled: i32) {
    mc_fake_input_queue_finish_event(rust_queue_of(queue), event);
}

unsafe extern "C" fn hook_input_queue_pre_dispatch_event(_queue: *mut c_void, _event: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn hook_event_get_source(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeInputEvent)).source }
}

unsafe extern "C" fn hook_event_get_type(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeInputEvent)).r#type }
}

unsafe extern "C" fn hook_event_get_device_id(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeInputEvent)).device_id }
}

unsafe extern "C" fn hook_key_get_action(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeKeyEvent)).action }
}

unsafe extern "C" fn hook_key_get_key_code(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeKeyEvent)).key_code }
}

unsafe extern "C" fn hook_key_get_repeat_count(_event: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn hook_key_get_meta_state(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeKeyEvent)).meta_state }
}

unsafe extern "C" fn hook_motion_get_action(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeMotionEvent)).action }
}

unsafe extern "C" fn hook_motion_get_pointer_count(_event: *mut c_void) -> i32 {
    1
}

unsafe extern "C" fn hook_motion_get_button_state(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeMotionEvent)).btn }
}

unsafe extern "C" fn hook_motion_get_pointer_id(event: *mut c_void) -> i32 {
    if event.is_null() {
        return 0;
    }
    unsafe { (*(event as *const FakeMotionEvent)).pointer_id }
}

unsafe extern "C" fn hook_motion_get_history_size(_event: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn hook_motion_get_x(event: *mut c_void, _pointer_index: usize) -> f32 {
    if event.is_null() {
        return 0.0;
    }
    unsafe { (*(event as *const FakeMotionEvent)).x }
}

unsafe extern "C" fn hook_motion_get_y(event: *mut c_void, _pointer_index: usize) -> f32 {
    if event.is_null() {
        return 0.0;
    }
    unsafe { (*(event as *const FakeMotionEvent)).y }
}

unsafe extern "C" fn hook_motion_get_raw_x(event: *mut c_void, pointer_index: usize) -> f32 {
    hook_motion_get_x(event, pointer_index)
}

unsafe extern "C" fn hook_motion_get_raw_y(event: *mut c_void, pointer_index: usize) -> f32 {
    hook_motion_get_y(event, pointer_index)
}

/// `axisFunction` is opaque here and never set in this build (gamepad motion
/// events go through `jni_support_send_motion_event`), so fall back to `dy` —
/// matching the current C++ behavior for the events that reach the queue.
unsafe extern "C" fn hook_motion_get_axis_value(
    event: *mut c_void,
    _axis: i32,
    _pointer_index: usize,
) -> f32 {
    if event.is_null() {
        return 0.0;
    }
    let dy = unsafe { (*(event as *const FakeMotionEvent)).dy };
    if dy != 0 {
        dy as f32
    } else {
        0.0
    }
}

/// Register all `libandroid.so` input hooks (replaces C++
/// `FakeInputQueue::initHybrisHooks`).
#[no_mangle]
pub unsafe extern "C" fn mc_register_fake_input_queue_hooks(map: *mut c_void) {
    unsafe {
        mc_register_android_hook(map, c"AInputQueue_getEvent".as_ptr(), hook_input_queue_get_event as *mut c_void);
        mc_register_android_hook(map, c"AInputQueue_finishEvent".as_ptr(), hook_input_queue_finish_event as *mut c_void);
        mc_register_android_hook(map, c"AInputQueue_preDispatchEvent".as_ptr(), hook_input_queue_pre_dispatch_event as *mut c_void);
        mc_register_android_hook(map, c"AInputEvent_getSource".as_ptr(), hook_event_get_source as *mut c_void);
        mc_register_android_hook(map, c"AInputEvent_getType".as_ptr(), hook_event_get_type as *mut c_void);
        mc_register_android_hook(map, c"AInputEvent_getDeviceId".as_ptr(), hook_event_get_device_id as *mut c_void);
        mc_register_android_hook(map, c"AKeyEvent_getAction".as_ptr(), hook_key_get_action as *mut c_void);
        mc_register_android_hook(map, c"AKeyEvent_getKeyCode".as_ptr(), hook_key_get_key_code as *mut c_void);
        mc_register_android_hook(map, c"AKeyEvent_getRepeatCount".as_ptr(), hook_key_get_repeat_count as *mut c_void);
        mc_register_android_hook(map, c"AKeyEvent_getMetaState".as_ptr(), hook_key_get_meta_state as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getAction".as_ptr(), hook_motion_get_action as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getPointerCount".as_ptr(), hook_motion_get_pointer_count as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getButtonState".as_ptr(), hook_motion_get_button_state as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getPointerId".as_ptr(), hook_motion_get_pointer_id as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getHistorySize".as_ptr(), hook_motion_get_history_size as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getX".as_ptr(), hook_motion_get_x as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getY".as_ptr(), hook_motion_get_y as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getRawX".as_ptr(), hook_motion_get_raw_x as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getRawY".as_ptr(), hook_motion_get_raw_y as *mut c_void);
        mc_register_android_hook(map, c"AMotionEvent_getAxisValue".as_ptr(), hook_motion_get_axis_value as *mut c_void);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layout_matches_cpp() {
        assert_eq!(std::mem::size_of::<FakeInputEvent>(), 12);
        assert_eq!(std::mem::offset_of!(FakeInputEvent, source), 0);
        assert_eq!(std::mem::offset_of!(FakeInputEvent, r#type), 4);
        assert_eq!(std::mem::offset_of!(FakeInputEvent, device_id), 8);

        assert_eq!(std::mem::size_of::<FakeKeyEvent>(), 24);
        assert_eq!(std::mem::offset_of!(FakeKeyEvent, action), 12);
        assert_eq!(std::mem::offset_of!(FakeKeyEvent, key_code), 16);
        assert_eq!(std::mem::offset_of!(FakeKeyEvent, meta_state), 20);

        assert_eq!(std::mem::size_of::<FakeMotionEvent>(), 72);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, action), 12);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, pointer_id), 16);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, x), 20);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, y), 24);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, axis_slot), 32);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, btn), 64);
        assert_eq!(std::mem::offset_of!(FakeMotionEvent, dy), 68);
    }

    #[test]
    fn queue_add_get_finish_ordering() {
        unsafe {
            let q = mc_fake_input_queue_create();
            assert!(!q.is_null());
            let mut key = FakeKeyEvent::new(5, 67, 2);
            key.base.source = AINPUT_SOURCE_KEYBOARD;
            let motion = FakeMotionEvent::new_button(0, 3, 0, 1.0, 2.0, 7, 42);
            mc_fake_input_queue_add_key_event(q, &key);
            mc_fake_input_queue_add_motion_event(q, &motion);
            assert!(mc_fake_input_queue_has_events(q));

            let mut out: *mut c_void = std::ptr::null_mut();
            assert_eq!(mc_fake_input_queue_get_event(q, &mut out), 0);
            let kev = out as *const FakeKeyEvent;
            assert_eq!((*kev).base.r#type, AINPUT_EVENT_TYPE_KEY);
            assert_eq!((*kev).action, 5);
            assert_eq!((*kev).key_code, 67);
            assert_eq!((*kev).meta_state, 2);
            assert_eq!(mc_fake_input_queue_finish_event(q, out), 0);

            assert_eq!(mc_fake_input_queue_get_event(q, &mut out), 0);
            let mev = out as *const FakeMotionEvent;
            assert_eq!((*mev).base.r#type, AINPUT_EVENT_TYPE_MOTION);
            assert_eq!((*mev).action, 3);
            assert_eq!((*mev).btn, 7);
            assert_eq!((*mev).dy, 42);
            assert_eq!(mc_fake_input_queue_finish_event(q, out), 0);

            assert!(!mc_fake_input_queue_has_events(q));
            mc_fake_input_queue_destroy(q);
        }
    }

    #[test]
    fn empty_queue_returns_minus_one() {
        unsafe {
            let q = mc_fake_input_queue_create();
            assert!(!q.is_null());
            let mut out: *mut c_void = std::ptr::null_mut();
            assert_eq!(mc_fake_input_queue_get_event(q, &mut out), -1);
            assert!(out.is_null());
            mc_fake_input_queue_destroy(q);
        }
    }

    #[test]
    fn finish_mismatch_returns_error() {
        unsafe {
            let q = mc_fake_input_queue_create();
            let key = FakeKeyEvent::new(1, 2, 0);
            mc_fake_input_queue_add_key_event(q, &key);
            let mut out: *mut c_void = std::ptr::null_mut();
            assert_eq!(mc_fake_input_queue_get_event(q, &mut out), 0);
            assert_ne!(mc_fake_input_queue_finish_event(q, std::ptr::null_mut()), 0);
            assert_eq!(mc_fake_input_queue_finish_event(q, out), 0);
            mc_fake_input_queue_destroy(q);
        }
    }

    #[test]
    fn axis_falls_back_to_dy() {
        unsafe {
            let mut motion = FakeMotionEvent::default();
            motion.dy = 37;
            let ptr = &motion as *const FakeMotionEvent as *mut c_void;
            assert_eq!(hook_motion_get_axis_value(ptr, 0, 0), 37.0);
            motion.dy = 0;
            assert_eq!(hook_motion_get_axis_value(ptr, 0, 0), 0.0);
        }
    }

    #[test]
    fn accessor_hooks_read_fields() {
        unsafe {
            let key = FakeKeyEvent::new_source(AINPUT_SOURCE_KEYBOARD, 3, 1, 68);
            let kp = &key as *const FakeKeyEvent as *mut c_void;
            assert_eq!(hook_event_get_source(kp), AINPUT_SOURCE_KEYBOARD);
            assert_eq!(hook_event_get_type(kp), AINPUT_EVENT_TYPE_KEY);
            assert_eq!(hook_event_get_device_id(kp), 3);
            assert_eq!(hook_key_get_action(kp), 1);
            assert_eq!(hook_key_get_key_code(kp), 68);
            assert_eq!(hook_key_get_repeat_count(kp), 0);

            let mut motion = FakeMotionEvent::new_button(AINPUT_SOURCE_KEYBOARD, 7, 9, 11.0, 13.0, 5, 0);
            motion.base.source = 0;
            let mp = &motion as *const FakeMotionEvent as *mut c_void;
            assert_eq!(hook_motion_get_action(mp), 7);
            assert_eq!(hook_motion_get_pointer_id(mp), 9);
            assert_eq!(hook_motion_get_x(mp, 0), 11.0);
            assert_eq!(hook_motion_get_y(mp, 3), 13.0);
            assert_eq!(hook_motion_get_raw_x(mp, 0), 11.0);
            assert_eq!(hook_motion_get_raw_y(mp, 1), 13.0);
            assert_eq!(hook_motion_get_pointer_count(mp), 1);
            assert_eq!(hook_motion_get_button_state(mp), 5);
            assert_eq!(hook_motion_get_history_size(mp), 0);
        }
    }
}
