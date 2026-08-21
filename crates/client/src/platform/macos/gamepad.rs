//! macOS placeholder for the Linux `gamepad` module (udev/evdev FFI).
//! Cocoa GameController support arrives with the Phase 4 windowing backend.

#![allow(unused)]

/// No-op on macOS; the Linux path loads SDL gamecontrollerdb.txt mappings.
pub fn load_mappings_from_file(_path: &str) {}
