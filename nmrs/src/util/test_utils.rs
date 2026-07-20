//! Shared test utilities.
//!
//! This module provides common test helpers that need to be shared across
//! multiple test modules to avoid race conditions.

use std::sync::Mutex;

/// Global mutex for tests that manipulate environment variables.
///
/// Any test that sets `XDG_DATA_HOME` or other env vars must hold this lock
/// to avoid race conditions with other tests running in parallel.
pub static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Runs a test closure with a fake `XDG_DATA_HOME` pointing to a unique temp directory.
///
/// This function:
/// 1. Acquires the global `ENV_LOCK` to serialize env var access
/// 2. Creates a unique temp directory
/// 3. Sets `XDG_DATA_HOME` to that directory
/// 4. Runs the provided closure
/// 5. Restores the previous env var value and removes the temp directory
///
/// If the closure panics, cleanup still happens (via Drop) but the mutex
/// will be poisoned. Use `ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())`
/// if you need to recover from poisoned state.
pub fn with_fake_xdg<R>(f: impl FnOnce() -> R) -> R {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poisoned| {
        // Recover from poisoned mutex (previous test panicked)
        poisoned.into_inner()
    });

    with_fake_xdg_unlocked(f)
}

fn with_fake_xdg_unlocked<R>(f: impl FnOnce() -> R) -> R {
    let previous_xdg_data_home = std::env::var_os("XDG_DATA_HOME");
    let base = std::env::temp_dir().join(format!("nmrs-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).expect("failed to create temp directory for test");

    // SAFETY: tests are serialized on ENV_LOCK; no other thread modifies env concurrently.
    unsafe {
        std::env::set_var("XDG_DATA_HOME", &base);
    }

    // Use a guard struct to ensure cleanup happens even on panic
    struct Cleanup {
        base: std::path::PathBuf,
        previous_xdg_data_home: Option<std::ffi::OsString>,
    }

    impl Drop for Cleanup {
        fn drop(&mut self) {
            // SAFETY: tests using this helper serialize environment access on ENV_LOCK.
            unsafe {
                match &self.previous_xdg_data_home {
                    Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                    None => std::env::remove_var("XDG_DATA_HOME"),
                }
            }
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    let _cleanup = Cleanup {
        base,
        previous_xdg_data_home,
    };
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RestoreXdg(Option<std::ffi::OsString>);

    impl Drop for RestoreXdg {
        fn drop(&mut self) {
            // SAFETY: the test holds ENV_LOCK until this guard is dropped.
            unsafe {
                match &self.0 {
                    Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                    None => std::env::remove_var("XDG_DATA_HOME"),
                }
            }
        }
    }

    #[test]
    fn fake_xdg_restores_existing_value() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _restore = RestoreXdg(std::env::var_os("XDG_DATA_HOME"));
        let original = std::env::temp_dir().join("nmrs-original-xdg");
        // SAFETY: the test holds ENV_LOCK and RestoreXdg restores the process state.
        unsafe { std::env::set_var("XDG_DATA_HOME", &original) };

        with_fake_xdg_unlocked(|| {
            assert_ne!(
                std::env::var_os("XDG_DATA_HOME").as_deref(),
                Some(original.as_os_str())
            );
        });

        assert_eq!(
            std::env::var_os("XDG_DATA_HOME").as_deref(),
            Some(original.as_os_str())
        );
    }

    #[test]
    fn fake_xdg_removes_value_when_initially_unset() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _restore = RestoreXdg(std::env::var_os("XDG_DATA_HOME"));
        // SAFETY: the test holds ENV_LOCK and RestoreXdg restores the process state.
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        with_fake_xdg_unlocked(|| {
            assert!(std::env::var_os("XDG_DATA_HOME").is_some());
        });

        assert!(std::env::var_os("XDG_DATA_HOME").is_none());
    }
}
