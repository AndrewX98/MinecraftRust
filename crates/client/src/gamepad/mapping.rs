//! SDL `gamecontrollerdb` string parser (ported from `gamepad_mapping.cpp`).

use std::collections::HashMap;

use super::ids::{GamepadAxis, GamepadButton};

/// Raw source of a mapping entry.
#[derive(Clone, Debug)]
pub enum MapFrom {
    Button { id: i32 },
    Axis { id: i32, min: f32, max: f32 },
    Hat { id: i32, mask: i32 },
}

/// Logical (gamepad-level) target of a mapping entry.
#[derive(Clone, Debug)]
pub enum MapTo {
    Button { id: GamepadButton },
    Axis { id: GamepadAxis, min: f32, max: f32 },
}

/// One `target:source` mapping pair.
#[derive(Clone, Debug)]
pub struct Mapping {
    pub from: MapFrom,
    pub to: MapTo,
}

/// A parsed controller mapping line: `guid,name,mapping,...`.
#[derive(Clone, Debug)]
pub struct GamepadMapping {
    pub guid: String,
    pub name: String,
    pub mappings: Vec<Mapping>,
}

impl Default for GamepadMapping {
    fn default() -> Self {
        GamepadMapping { guid: String::new(), name: String::new(), mappings: Vec::new() }
    }
}

fn find_e(s: &str, c: char, start: usize) -> Result<usize, String> {
    match s[start..].find(c) {
        Some(p) => Ok(start + p),
        None => Err("Invalid mapping".to_string()),
    }
}

/// `strtol(..., 10)` equivalent starting at byte offset `start`.
fn parse_int(s: &str, start: usize) -> Result<(i32, usize), String> {
    let bytes = s.as_bytes();
    let mut i = start;
    if i >= bytes.len() {
        return Err("Invalid integer".to_string());
    }
    let neg = if bytes[i] == b'-' { i += 1; true } else { false };
    let dstart = i;
    let mut val: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        val = val * 10 + (bytes[i] - b'0') as i64;
        i += 1;
    }
    if i == dstart {
        return Err("Invalid integer".to_string());
    }
    Ok(((if neg { -val } else { val }) as i32, i))
}

fn known_buttons() -> HashMap<&'static str, GamepadButton> {
    let mut m = HashMap::new();
    m.insert("a", GamepadButton::A);
    m.insert("b", GamepadButton::B);
    m.insert("x", GamepadButton::X);
    m.insert("y", GamepadButton::Y);
    m.insert("leftshoulder", GamepadButton::LB);
    m.insert("rightshoulder", GamepadButton::RB);
    m.insert("back", GamepadButton::Back);
    m.insert("start", GamepadButton::Start);
    m.insert("guide", GamepadButton::Guide);
    m.insert("leftstick", GamepadButton::LeftStick);
    m.insert("rightstick", GamepadButton::RightStick);
    m.insert("dpup", GamepadButton::DpadUp);
    m.insert("dpright", GamepadButton::DpadRight);
    m.insert("dpdown", GamepadButton::DpadDown);
    m.insert("dpleft", GamepadButton::DpadLeft);
    m
}

fn known_axes() -> HashMap<&'static str, GamepadAxis> {
    let mut m = HashMap::new();
    m.insert("leftx", GamepadAxis::LeftX);
    m.insert("lefty", GamepadAxis::LeftY);
    m.insert("rightx", GamepadAxis::RightX);
    m.insert("righty", GamepadAxis::RightY);
    m.insert("lefttrigger", GamepadAxis::LeftTrigger);
    m.insert("righttrigger", GamepadAxis::RightTrigger);
    m
}

impl GamepadMapping {
    /// Parse a single `gamecontrollerdb.txt` line. Throws `Err` on malformed input
    /// (unknown `platform:...` and similar entries are skipped, not errors).
    pub fn parse(&mut self, mapping: &str) -> Result<(), String> {
        let buttons = known_buttons();
        let axes = known_axes();

        let iof = find_e(mapping, ',', 0)?;
        self.guid = mapping[..iof].to_string();
        let iof = iof + 1;
        let iof2 = find_e(mapping, ',', iof)?;
        self.name = mapping[iof..iof2].to_string();
        let mut iof = iof2;

        let bytes = mapping.as_bytes();
        while iof != usize::MAX && iof + 1 < mapping.len() {
            let mut c = Mapping { from: MapFrom::Button { id: 0 }, to: MapTo::Button { id: GamepadButton::A } };

            iof += 1;
            // parse target
            let mut to_mod = 0u8;
            if bytes[iof] == b'-' || bytes[iof] == b'+' {
                to_mod = bytes[iof];
                iof += 1;
            }
            let iof2 = find_e(mapping, ':', iof)?;
            let from = &mapping[iof..iof2];
            if let Some(btn) = buttons.get(from) {
                c.to = MapTo::Button { id: *btn };
            } else if let Some(axis) = axes.get(from) {
                if *axis == GamepadAxis::LeftTrigger || *axis == GamepadAxis::RightTrigger {
                    c.to = MapTo::Axis { id: *axis, min: 0.0, max: 1.0 };
                } else if to_mod == 0 {
                    c.to = MapTo::Axis { id: *axis, min: -1.0, max: 1.0 };
                } else {
                    c.to = MapTo::Axis {
                        id: *axis,
                        min: 0.0,
                        max: if to_mod == b'+' { 1.0 } else { -1.0 },
                    };
                }
            } else {
                iof = match mapping[iof + 1..].find(',') {
                    Some(p) => iof + 1 + p,
                    None => usize::MAX,
                };
                continue;
            }
            iof = iof2 + 1;
            if iof >= mapping.len() {
                return Err("Invalid mapping: unexpected end".to_string());
            }

            // parse source
            let mut from_mod = 0u8;
            if bytes[iof] == b'-' || bytes[iof] == b'+' {
                from_mod = bytes[iof];
                iof += 1;
            }
            let mut inv = false;
            if bytes[iof] == b'~' {
                inv = true;
                iof += 1;
            }
            if bytes[iof] == b'b' {
                let (id, next) = parse_int(mapping, iof + 1)?;
                iof = next;
                c.from = MapFrom::Button { id };
            } else if bytes[iof] == b'a' {
                let (id, next) = parse_int(mapping, iof + 1)?;
                iof = next;
                let (min, max) = if from_mod == 0 {
                    (-1.0, 1.0)
                } else {
                    (0.0, if from_mod == b'+' { 1.0 } else { -1.0 })
                };
                c.from = MapFrom::Axis { id, min, max };
                if inv {
                    if let MapTo::Axis { min: tmin, max: tmax, .. } = &mut c.to {
                        std::mem::swap(tmin, tmax);
                    }
                }
            } else if bytes[iof] == b'h' {
                let (id, next) = parse_int(mapping, iof + 1)?;
                if mapping.as_bytes().get(next) != Some(&b'.') {
                    return Err("Invalid mapping: expected . after hat id".to_string());
                }
                let (mask, next) = parse_int(mapping, next + 1)?;
                iof = next;
                c.from = MapFrom::Hat { id, mask };
            } else {
                return Err("Invalid mapping: invalid map-to".to_string());
            }
            self.mappings.push(c);

            iof = match mapping[iof..].find(',') {
                Some(p) => iof + p,
                None => usize::MAX,
            };
        }
        Ok(())
    }

    /// Whether an axis `value` counts as "active" for a mapping source.
    pub fn is_axis_active(from: &MapFrom, value: f32) -> bool {
        if let MapFrom::Axis { min, max, .. } = from {
            if min < max {
                value >= (min + max) / 2.0
            } else {
                value <= (min + max) / 2.0
            }
        } else {
            false
        }
    }

    /// Transform a raw axis `value` through a mapping to the logical axis range.
    /// Returns NaN when the raw value is outside the source range.
    pub fn get_axis_transformed_value(map: &Mapping, value: f32) -> f32 {
        let (amin, amax) = match map.from {
            MapFrom::Axis { min, max, .. } => (min, max),
            _ => return f32::NAN,
        };
        let (dmin, dmax) = match map.to {
            MapTo::Axis { min, max, .. } => (min, max),
            _ => return f32::NAN,
        };
        if value < amin.min(amax) || value > amin.max(amax) {
            return f32::NAN;
        }
        let t = (value - amin) / (amax - amin);
        dmin + t * (dmax - dmin)
    }
}
