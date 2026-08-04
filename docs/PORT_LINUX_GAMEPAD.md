# Port: linux-gamepad

**Status:** 0% Rust. C++ only. Depends on `libevdev` + `libudev` (both in system deps already).

## C++ to port (558 lines)

| File | Lines | Role |
|------|-------|------|
| `manifest_libs/linux-gamepad/src/linux_joystick.cpp` | 137 | `LinuxJoystick` — per-device evdev poll, button/axis/hat state |
| `manifest_libs/linux-gamepad/src/linux_joystick_manager.cpp` | 104 | `LinuxJoystickManager` — udev monitor hotplug (add/remove) |
| `manifest_libs/linux-gamepad/src/gamepad.cpp` | 48 | `Gamepad` — maps raw joystick → logical button/axis via `GamepadMapping` |
| `manifest_libs/linux-gamepad/src/gamepad_manager.cpp` | 116 | `GamepadManager` — connected/disconnected/button/axis callbacks, mapping lookup, id pool |
| `manifest_libs/linux-gamepad/src/gamepad_mapping.cpp` | 153 | `GamepadMapping` — SDL `gamecontrollerdb` string parser (`gamecontrollerdb.txt` bundled in `runtime/`) |

Plus `manifest_libs/gamewindow/joystick_manager_linux_gamepad.cpp` (149) — window↔gamepad glue (`LinuxGamepadJoystickManager`), ported with the gamewindow work.

## Interfaces (from `crates/client/include/linux-gamepad/`)

- `Joystick`: `getGUID`, `getButton(i)`, `getAxis(i)`, `getHat(i)`
- `JoystickManager`: callback lists (`onJoystickConnected/Disconnected/Button/Axis/Hat`) + `initialize()` + `poll()`
- `GamepadManager`: `onGamepadConnected/Disconnected/Button/Axis` callbacks, `addMapping`, `getMapping`; button/axis enums in `gamepad_ids.h`
- `JoystickManagerFactory::create()`

## Steps

1. Add raw FFI bindings (no new workspace crates needed — direct `extern "C"` + `dlopen`/link):
   - `libudev`: `udev_new`, `udev_monitor_new_from_netlink`, `udev_monitor_enable_receiving`, `udev_monitor_get_fd`, `udev_monitor_receive_device`, `udev_device_*` (sysname, devnode, action).
   - `libevdev`: `libevdev_new_from_fd`, `libevdev_next_event`, `libevdev_get_abs_info`, `libevdev_get_abs_max/min`, `libevdev_fetch_status_value` (ABS/KEY/EV_SYN).
2. Port `LinuxJoystick` + `LinuxJoystickManager` (udev hotplug + poll loop) → single module (e.g. `crates/client/src/gamepad/joystick.rs`).
3. Port `GamepadMapping` SDL parser (string → `Vec<Mapping>`, `guid,name,mapping,...` incl. `platform:Linux`).
4. Port `Gamepad` + `GamepadManager` with `Vec<Box<dyn Fn>>` callback lists replacing `CallbackList`.
5. Port `JoystickManager::handleMissingGamePadMapping` debug helper (dummy mapping generator) — lives in gamewindow `joystick_manager.cpp`.
6. Wire into Rust eglut window (`WindowWithLinuxJoystick` equivalent): `addWindowToGamepadManager` / `updateGamepad` on each `pollEvents`.
7. Replace `JoystickManagerFactory` / `LinuxGamepadJoystickManager::instance` callers in `window_callbacks_stub.cpp`, then delete both C++ targets from `cpp-bridge-sys/build.rs`.

## Done when

- Gamepad works without the C++ target; no `gamepad::` symbols in `nm`.
