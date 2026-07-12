use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

static PROPERTIES: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn bool_from_string(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "1" | "y" | "yes" | "on" | "true" => Some(true),
        "0" | "n" | "no" | "off" | "false" => Some(false),
        _ => None,
    }
}

/// Get a system property value.
pub fn get_property(key: &str, default_value: &str) -> String {
    let map = PROPERTIES.read().unwrap();
    match map.get(key) {
        Some(v) if !v.is_empty() => v.clone(),
        _ => default_value.to_string(),
    }
}

/// Set a system property value.
pub fn set_property(key: &str, value: &str) -> bool {
    let mut map = PROPERTIES.write().unwrap();
    if value.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), value.to_string());
    }
    true
}

/// Get a boolean system property.
pub fn get_bool_property(key: &str, default_value: bool) -> bool {
    match bool_from_string(&get_property(key, "")) {
        Some(v) => v,
        None => default_value,
    }
}

/// Get a signed integer system property with range constraints.
pub fn get_int_property<T>(key: &str, default_value: T, min: T, max: T) -> T
where
    T: std::str::FromStr + PartialOrd + Copy,
{
    let value = get_property(key, "");
    if value.is_empty() {
        return default_value;
    }
    match value.parse::<T>() {
        Ok(v) if v >= min && v <= max => v,
        _ => default_value,
    }
}

/// Get an unsigned integer system property with upper bound.
pub fn get_uint_property<T>(key: &str, default_value: T, max: T) -> T
where
    T: std::str::FromStr + PartialOrd + Copy,
{
    let value = get_property(key, "");
    if value.is_empty() {
        return default_value;
    }
    match value.parse::<T>() {
        Ok(v) if v <= max => v,
        _ => default_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_lock<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().unwrap();
        f();
    }

    #[test]
    fn test_get_property_default() {
        with_lock(|| {
            let v = get_property("nonexistent", "default_val");
            assert_eq!(v, "default_val");
        });
    }

    #[test]
    fn test_set_and_get_property() {
        with_lock(|| {
            set_property("test.key", "hello");
            assert_eq!(get_property("test.key", ""), "hello");
        });
    }

    #[test]
    fn test_get_property_empty_is_default() {
        with_lock(|| {
            set_property("test.empty", "");
            let v = get_property("test.empty", "fallback");
            assert_eq!(v, "fallback");
        });
    }

    #[test]
    fn test_set_empty_removes() {
        with_lock(|| {
            set_property("test.remove", "value");
            assert_eq!(get_property("test.remove", ""), "value");
            set_property("test.remove", "");
            assert_eq!(get_property("test.remove", "default"), "default");
        });
    }

    #[test]
    fn test_get_bool_true_values() {
        with_lock(|| {
            for val in &["1", "y", "yes", "on", "true", "TRUE", "Yes"] {
                let key = format!("test.bool.true.{}", val);
                set_property(&key, val);
                assert!(get_bool_property(&key, false), "expected true for '{}'", val);
            }
        });
    }

    #[test]
    fn test_get_bool_false_values() {
        with_lock(|| {
            for val in &["0", "n", "no", "off", "false"] {
                let key = format!("test.bool.false.{}", val);
                set_property(&key, val);
                assert!(!get_bool_property(&key, true), "expected false for '{}'", val);
            }
        });
    }

    #[test]
    fn test_get_bool_default() {
        with_lock(|| {
            assert!(get_bool_property("does.not.exist", true));
            assert!(!get_bool_property("does.not.exist", false));
            set_property("test.unknown", "burp");
            assert!(get_bool_property("test.unknown", true));
            assert!(!get_bool_property("test.unknown", false));
        });
    }

    #[test]
    fn test_get_int_property_in_range() {
        with_lock(|| {
            set_property("test.int", "42");
            assert_eq!(get_int_property::<i32>("test.int", 0, 0, 100), 42);
        });
    }

    #[test]
    fn test_get_int_property_below_min() {
        with_lock(|| {
            set_property("test.int", "-5");
            assert_eq!(get_int_property::<i32>("test.int", 10, 0, 100), 10);
        });
    }

    #[test]
    fn test_get_int_property_above_max() {
        with_lock(|| {
            set_property("test.int", "200");
            assert_eq!(get_int_property::<i32>("test.int", 10, 0, 100), 10);
        });
    }

    #[test]
    fn test_get_int_property_invalid() {
        with_lock(|| {
            set_property("test.int", "not_a_number");
            assert_eq!(get_int_property::<i32>("test.int", -1, -100, 100), -1);
        });
    }

    #[test]
    fn test_get_int_property_empty_uses_default() {
        with_lock(|| {
            set_property("test.int", "");
            assert_eq!(get_int_property::<i32>("test.int", 99, 0, 200), 99);
        });
    }

    #[test]
    fn test_get_int_property_int8_types() {
        with_lock(|| {
            set_property("test.int8", "127");
            assert_eq!(get_int_property::<i8>("test.int8", 0, -128, 127), 127);
        });
    }

    #[test]
    fn test_get_int_property_int64_types() {
        with_lock(|| {
            set_property("test.int64", "9223372036854775807");
            assert_eq!(
                get_int_property::<i64>("test.int64", 0, 0, i64::MAX),
                9223372036854775807
            );
        });
    }

    #[test]
    fn test_get_uint_property() {
        with_lock(|| {
            set_property("test.uint", "100");
            assert_eq!(get_uint_property::<u32>("test.uint", 0, 200), 100);
        });
    }

    #[test]
    fn test_get_uint_property_above_max() {
        with_lock(|| {
            set_property("test.uint", "300");
            assert_eq!(get_uint_property::<u32>("test.uint", 50, 200), 50);
        });
    }

    #[test]
    fn test_get_uint_property_invalid() {
        with_lock(|| {
            set_property("test.uint", "neg");
            assert_eq!(get_uint_property::<u32>("test.uint", 7, 100), 7);
        });
    }

    #[test]
    fn test_get_uint_property_uint8() {
        with_lock(|| {
            set_property("test.u8", "255");
            assert_eq!(get_uint_property::<u8>("test.u8", 0, 255), 255);
        });
    }

    #[test]
    fn test_get_uint_property_uint64() {
        with_lock(|| {
            set_property("test.u64", "18446744073709551615");
            assert_eq!(
                get_uint_property::<u64>("test.u64", 0, u64::MAX),
                18446744073709551615
            );
        });
    }

    #[test]
    fn test_set_property_returns_true() {
        with_lock(|| {
            assert!(set_property("test", "val"));
        });
    }

    #[test]
    fn test_multiple_properties_independent() {
        with_lock(|| {
            set_property("key1", "val1");
            set_property("key2", "val2");
            assert_eq!(get_property("key1", ""), "val1");
            assert_eq!(get_property("key2", ""), "val2");
        });
    }

    #[test]
    fn test_overwrite_property() {
        with_lock(|| {
            set_property("test.over", "first");
            assert_eq!(get_property("test.over", ""), "first");
            set_property("test.over", "second");
            assert_eq!(get_property("test.over", ""), "second");
        });
    }
}
