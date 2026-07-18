//! Kernel rfkill state reader via sysfs.
//!
//! Reads `/sys/class/rfkill/*/type` and `/sys/class/rfkill/*/hard` to detect
//! hardware radio kill switches. This is a fallback for cases where
//! NetworkManager's `*HardwareEnabled` properties disagree with the kernel.

use std::fs;
use std::path::Path;

/// Snapshot of hardware (hard-block) rfkill state for each radio type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RfkillSnapshot {
    /// `true` if any WLAN rfkill entry reports a hard block.
    pub wlan_hard_block: bool,
    /// `true` if any WWAN rfkill entry reports a hard block.
    pub wwan_hard_block: bool,
    /// `true` if any Bluetooth rfkill entry reports a hard block.
    pub bluetooth_hard_block: bool,
}

/// Reads the current rfkill hardware-block state from sysfs.
///
/// Returns an all-false snapshot if `/sys/class/rfkill` is unreadable
/// (common in containers and CI environments).
pub(crate) fn read_rfkill() -> RfkillSnapshot {
    read_rfkill_from(Path::new("/sys/class/rfkill"))
}

fn read_rfkill_from(rfkill_dir: &Path) -> RfkillSnapshot {
    let entries = match fs::read_dir(rfkill_dir) {
        Ok(e) => e,
        Err(_) => return RfkillSnapshot::default(),
    };

    let mut snapshot = RfkillSnapshot::default();

    for entry in entries.flatten() {
        let path = entry.path();

        let type_str = match fs::read_to_string(path.join("type")) {
            Ok(s) => s.trim().to_string(),
            Err(_) => continue,
        };

        let hard_blocked = match fs::read_to_string(path.join("hard")) {
            Ok(s) => s.trim() == "1",
            Err(_) => false,
        };

        if hard_blocked {
            match type_str.as_str() {
                "wlan" => snapshot.wlan_hard_block = true,
                "wwan" => snapshot.wwan_hard_block = true,
                "bluetooth" => snapshot.bluetooth_hard_block = true,
                _ => {}
            }
        }
    }

    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RfkillFixture {
        root: std::path::PathBuf,
    }

    impl RfkillFixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("nmrs-rfkill-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn add_entry(&self, name: &str, radio_type: Option<&str>, hard: Option<&str>) {
            let entry = self.root.join(name);
            fs::create_dir_all(&entry).unwrap();
            if let Some(radio_type) = radio_type {
                fs::write(entry.join("type"), radio_type).unwrap();
            }
            if let Some(hard) = hard {
                fs::write(entry.join("hard"), hard).unwrap();
            }
        }
    }

    impl Drop for RfkillFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn unreadable_rfkill_directory_returns_unblocked_snapshot() {
        let missing =
            std::env::temp_dir().join(format!("nmrs-rfkill-missing-{}", uuid::Uuid::new_v4()));

        assert_eq!(read_rfkill_from(&missing), RfkillSnapshot::default());
    }

    #[test]
    fn parses_hard_blocks_for_each_supported_radio_type() {
        let fixture = RfkillFixture::new();
        fixture.add_entry("rfkill0", Some("wlan\n"), Some("1\n"));
        fixture.add_entry("rfkill1", Some("wwan"), Some(" 1 "));
        fixture.add_entry("rfkill2", Some("bluetooth\n"), Some("1"));

        assert_eq!(
            read_rfkill_from(&fixture.root),
            RfkillSnapshot {
                wlan_hard_block: true,
                wwan_hard_block: true,
                bluetooth_hard_block: true,
            }
        );
    }

    #[test]
    fn ignores_soft_unblocked_unknown_and_incomplete_entries() {
        let fixture = RfkillFixture::new();
        fixture.add_entry("rfkill0", Some("wlan"), Some("0"));
        fixture.add_entry("rfkill1", Some("wwan"), Some("not-a-bit"));
        fixture.add_entry("rfkill2", Some("nfc"), Some("1"));
        fixture.add_entry("rfkill3", Some("bluetooth"), None);
        fixture.add_entry("rfkill4", None, Some("1"));

        assert_eq!(read_rfkill_from(&fixture.root), RfkillSnapshot::default());
    }

    #[test]
    fn any_hard_blocked_entry_wins_for_a_radio_type() {
        let fixture = RfkillFixture::new();
        fixture.add_entry("rfkill0", Some("wlan"), Some("0"));
        fixture.add_entry("rfkill1", Some("wlan"), Some("1"));

        assert!(read_rfkill_from(&fixture.root).wlan_hard_block);
    }
}
