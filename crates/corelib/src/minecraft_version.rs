use std::ffi::c_char;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub revision: i32,
    pub code: i32,
}

impl Version {
    pub fn get_string(&self) -> String {
        format!("{}.{}.{}.{}", self.major, self.minor, self.patch, self.revision)
    }
}

pub fn decode(version_code: i64) -> Option<(i32, i32, i32, i32)> {
    let is_android = version_code >= 950_000_000 && version_code < 990_000_000;
    let is_chromeos = version_code >= 1_950_000_000 && version_code < 1_990_000_000;
    if !(is_android || is_chromeos) {
        return None;
    }
    let mut parts = (version_code % 10_000_000) as i32;
    let major = 1;
    let minor = parts / 100_000;
    parts = parts % 100_000;
    let patch = parts / 100;
    parts = parts % 100;
    let revision = parts;
    Some((major, minor, patch, revision))
}

static STATE: Mutex<Version> = Mutex::new(Version {
    major: 0,
    minor: 0,
    patch: 0,
    revision: 0,
    code: 0,
});

#[no_mangle]
pub extern "C" fn mc_init_version(package: *const c_char, version_code: i32) {
    let package = unsafe {
        if package.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(package).to_string_lossy().into_owned())
        }
    };
    let (major, minor, patch, revision) = match decode(version_code as i64) {
        Some(v) => v,
        None => (0, 0, 0, 0),
    };
    let mut state = STATE.lock().unwrap();
    state.major = major;
    state.minor = minor;
    state.patch = patch;
    state.revision = revision;
    state.code = version_code;
    drop(package);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_android() {
        assert_eq!(decode(962_112_004), Some((1, 21, 120, 4)));
    }

    #[test]
    fn decode_chromeos() {
        assert_eq!(decode(1_987_654_321), Some((1, 76, 543, 21)));
    }

    #[test]
    fn decode_non_android_passthrough() {
        assert_eq!(decode(123), None);
        assert_eq!(decode(949_999_999), None);
        assert_eq!(decode(990_000_000), None);
    }

    #[test]
    fn get_string_android() {
        let (maj, min, pat, rev) = decode(962_112_004).unwrap();
        let v = Version { major: maj, minor: min, patch: pat, revision: rev, code: 962_112_004 };
        assert_eq!(v.get_string(), "1.21.120.4");
    }

    #[test]
    fn get_string_non_android() {
        let v = Version { major: 0, minor: 0, patch: 0, revision: 0, code: 123 };
        assert_eq!(v.get_string(), "0.0.0.0");
    }
}
