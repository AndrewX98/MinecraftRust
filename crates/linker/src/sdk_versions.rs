use std::sync::atomic::{AtomicI32, Ordering};

const ANDROID_API: i32 = 35;
static TARGET_SDK_VERSION: AtomicI32 = AtomicI32::new(ANDROID_API);

pub fn set_application_target_sdk_version(target: i32) {
    let target = if target == 0 { ANDROID_API } else { target };
    TARGET_SDK_VERSION.store(target, Ordering::SeqCst);
}

pub fn get_application_target_sdk_version() -> i32 {
    TARGET_SDK_VERSION.load(Ordering::SeqCst)
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
    fn test_default_sdk_version() {
        with_lock(|| {
            assert_eq!(get_application_target_sdk_version(), ANDROID_API);
        });
    }

    #[test]
    fn test_set_sdk_version() {
        with_lock(|| {
            let prev = get_application_target_sdk_version();
            set_application_target_sdk_version(30);
            assert_eq!(get_application_target_sdk_version(), 30);
            set_application_target_sdk_version(prev);
        });
    }

    #[test]
    fn test_set_zero_uses_default() {
        with_lock(|| {
            set_application_target_sdk_version(0);
            assert_eq!(get_application_target_sdk_version(), ANDROID_API);
        });
    }
}
