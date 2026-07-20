//! Persist inline PEM material from `.ovpn` profiles to disk for NetworkManager
//!
//! # Connection-rename caveat
//!
//! The cert directory is keyed by `connection_name` at import time. If a user
//! later renames the NM connection (e.g. via `nmcli`), `forget_vpn` will look
//! for `certs/<new_name>/` which won't exist, and `certs/<old_name>/` will
//! linger on disk. A future improvement could store the cert directory name in
//! a custom `vpn.data` key so cleanup remains correct after renames.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{ConnectionError, util::validation::validate_connection_name};

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Writes PEM bytes for one material type and returns an **absolute** path for `vpn.data`.
///
/// `cert_type`: `"ca"`, `"cert"`, `"key"`, or `"ta"` (tls-auth static key).
///
/// The write is atomic: data is flushed to a temporary file in the same
/// directory and then renamed into place, so readers never see a half-written
/// PEM file.
pub fn store_inline_cert(
    connection_name: &str,
    cert_type: &str,
    pem_data: &str,
) -> Result<PathBuf, ConnectionError> {
    // Validate the material type before creating anything on disk.
    let filename = filename_for(cert_type)?;
    let dir = connection_cert_dir(connection_name)?;
    fs::create_dir_all(&dir).map_err(|e| {
        ConnectionError::VpnFailed(format!(
            "cert store: create directory {}: {e}",
            dir.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| {
            ConnectionError::VpnFailed(format!(
                "cert store: chmod directory {}: {e}",
                dir.display()
            ))
        })?;
    }

    let path = dir.join(filename);
    let tmp_path = dir.join(format!(".{filename}.{}.tmp", uuid::Uuid::new_v4()));
    let _temporary_file = TemporaryFile(tmp_path.clone());

    {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp_path).map_err(|e| {
            ConnectionError::VpnFailed(format!(
                "cert store: open {} for write: {e}",
                tmp_path.display(),
            ))
        })?;
        file.write_all(pem_data.as_bytes()).map_err(|e| {
            ConnectionError::VpnFailed(format!("cert store: write {}: {e}", tmp_path.display(),))
        })?;
        file.sync_all().map_err(|e| {
            ConnectionError::VpnFailed(format!("cert store: sync {}: {e}", tmp_path.display()))
        })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600)).map_err(|e| {
            ConnectionError::VpnFailed(format!("cert store: chmod {}: {e}", tmp_path.display()))
        })?;
    }

    fs::rename(&tmp_path, &path).map_err(|e| {
        ConnectionError::VpnFailed(format!(
            "cert store: rename {} -> {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;

    path.canonicalize().map_err(|e| {
        ConnectionError::VpnFailed(format!("cert store: canonicalize {}: {e}", path.display()))
    })
}

/// Removes all stored cert files for this connection.
///
/// **Idempotent:** if the directory does not exist, returns `Ok(())`.
pub fn cleanup_certs(connection_name: &str) -> Result<(), ConnectionError> {
    let dir = connection_cert_dir(connection_name)?;
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ConnectionError::VpnFailed(format!(
            "cert store: remove {}: {e}",
            dir.display()
        ))),
    }
}

/// Resolved XDG data home: `$XDG_DATA_HOME`, or `$HOME/.local/share` if unset or empty.
fn xdg_data_home() -> Result<PathBuf, ConnectionError> {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => {
            let home = std::env::var_os("HOME").ok_or_else(|| {
                ConnectionError::VpnFailed(
                    "cert store: HOME is not set (cannot resolve XDG data directory)".into(),
                )
            })?;
            Ok(Path::new(&home).join(".local/share"))
        }
    }
}

/// `$XDG_DATA_HOME/nmrs/certs/<connection_name>/`
fn connection_cert_dir(connection_name: &str) -> Result<PathBuf, ConnectionError> {
    validate_connection_name(connection_name)?;
    if connection_name.contains('/') || connection_name.contains('\\') {
        return Err(ConnectionError::InvalidAddress(
            "connection name must not contain path separators".into(),
        ));
    }
    if connection_name == "." || connection_name == ".." {
        return Err(ConnectionError::InvalidAddress(
            "invalid connection name".into(),
        ));
    }
    Ok(xdg_data_home()?
        .join("nmrs")
        .join("certs")
        .join(connection_name))
}

fn filename_for(cert_type: &str) -> Result<&'static str, ConnectionError> {
    match cert_type {
        "ca" => Ok("ca.pem"),
        "cert" => Ok("cert.pem"),
        "key" => Ok("key.pem"),
        "ta" => Ok("ta.key"),
        "tls-crypt" => Ok("tls-crypt.key"),
        _ => Err(ConnectionError::InvalidAddress(format!(
            "unknown cert_type {cert_type:?} (expected ca, cert, key, ta, tls-crypt)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_utils::{ENV_LOCK, with_fake_xdg};

    struct EnvRestore {
        xdg: Option<std::ffi::OsString>,
        home: Option<std::ffi::OsString>,
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            // SAFETY: each caller holds ENV_LOCK until after this guard drops.
            unsafe {
                match &self.xdg {
                    Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                    None => std::env::remove_var("XDG_DATA_HOME"),
                }
                match &self.home {
                    Some(value) => std::env::set_var("HOME", value),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    fn lock_env() -> (std::sync::MutexGuard<'static, ()>, EnvRestore) {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let restore = EnvRestore {
            xdg: std::env::var_os("XDG_DATA_HOME"),
            home: std::env::var_os("HOME"),
        };
        (lock, restore)
    }

    fn temporary_files(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "tmp"))
            .collect()
    }

    #[test]
    fn write_read_cleanup_cycle() {
        with_fake_xdg(|| {
            let pem = "-----BEGIN CERTIFICATE-----\nABC\n-----END CERTIFICATE-----\n";
            let p = store_inline_cert("MyVPN", "ca", pem).unwrap();
            let got = std::fs::read_to_string(&p).unwrap();
            assert_eq!(got, pem);
            cleanup_certs("MyVPN").unwrap();
            assert!(!p.exists());
        });
    }

    #[test]
    fn cleanup_nonexistent_is_ok() {
        with_fake_xdg(|| {
            cleanup_certs("does-not-exist").unwrap();
        });
    }

    #[test]
    fn double_cleanup_ok() {
        with_fake_xdg(|| {
            store_inline_cert("x", "ca", "pem").unwrap();
            cleanup_certs("x").unwrap();
            cleanup_certs("x").unwrap();
        });
    }

    #[test]
    fn all_supported_material_types_use_fixed_filenames() {
        let cases = [
            ("ca", "ca.pem"),
            ("cert", "cert.pem"),
            ("key", "key.pem"),
            ("ta", "ta.key"),
            ("tls-crypt", "tls-crypt.key"),
        ];

        for (cert_type, expected) in cases {
            assert_eq!(filename_for(cert_type).unwrap(), expected);
        }
    }

    #[test]
    fn unknown_material_type_is_rejected_without_creating_a_directory() {
        with_fake_xdg(|| {
            let data_home = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
            let result = store_inline_cert("unknown-type", "bogus", "secret");

            assert!(matches!(
                result,
                Err(ConnectionError::InvalidAddress(message))
                    if message == "unknown cert_type \"bogus\" (expected ca, cert, key, ta, tls-crypt)"
            ));
            assert!(!data_home.join("nmrs/certs/unknown-type").exists());
        });
    }

    #[test]
    fn connection_name_cannot_escape_the_cert_root() {
        with_fake_xdg(|| {
            for name in ["../outside", "nested/name", r"nested\name"] {
                let result = connection_cert_dir(name);
                assert!(matches!(
                    result,
                    Err(ConnectionError::InvalidAddress(message))
                        if message == "connection name must not contain path separators"
                ));
            }

            for name in [".", ".."] {
                let result = connection_cert_dir(name);
                assert!(matches!(
                    result,
                    Err(ConnectionError::InvalidAddress(message))
                        if message == "invalid connection name"
                ));
            }
        });
    }

    #[test]
    fn overwrite_replaces_contents_and_removes_temporary_file() {
        with_fake_xdg(|| {
            let first = store_inline_cert("overwrite", "ca", "old").unwrap();
            let second = store_inline_cert("overwrite", "ca", "new").unwrap();

            assert_eq!(second, first);
            assert_eq!(std::fs::read_to_string(&second).unwrap(), "new");
            assert!(temporary_files(second.parent().unwrap()).is_empty());
        });
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_writes_are_atomic_and_do_not_share_temporary_files() {
        with_fake_xdg(|| {
            let payloads: Vec<String> = (0..8)
                .map(|index| format!("-----BEGIN KEY-----\npayload-{index}\n-----END KEY-----\n"))
                .collect();
            let handles: Vec<_> = payloads
                .iter()
                .cloned()
                .map(|payload| {
                    std::thread::spawn(move || store_inline_cert("concurrent", "key", &payload))
                })
                .collect();

            let paths: Vec<_> = handles
                .into_iter()
                .map(|handle| handle.join().unwrap().unwrap())
                .collect();
            assert!(paths.iter().all(|path| path == &paths[0]));

            let final_payload = std::fs::read_to_string(&paths[0]).unwrap();
            assert!(payloads.contains(&final_payload));
            assert!(temporary_files(paths[0].parent().unwrap()).is_empty());
        });
    }

    #[test]
    fn rename_failure_removes_temporary_file() {
        with_fake_xdg(|| {
            let data_home = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
            let cert_dir = data_home.join("nmrs/certs/rename-failure");
            std::fs::create_dir_all(cert_dir.join("ca.pem")).unwrap();

            let result = store_inline_cert("rename-failure", "ca", "secret");

            assert!(matches!(
                result,
                Err(ConnectionError::VpnFailed(message))
                    if message.contains("cert store: rename")
            ));
            assert!(temporary_files(&cert_dir).is_empty());
            assert!(cert_dir.join("ca.pem").is_dir());
        });
    }

    #[test]
    fn create_directory_io_error_has_operation_context() {
        with_fake_xdg(|| {
            let data_home = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
            std::fs::remove_dir(&data_home).unwrap();
            std::fs::write(&data_home, "not a directory").unwrap();

            let result = store_inline_cert("io-error", "ca", "secret");

            assert!(matches!(
                result,
                Err(ConnectionError::VpnFailed(message))
                    if message.contains("cert store: create directory")
                        && message.contains("nmrs/certs/io-error")
            ));
        });
    }

    #[test]
    fn cleanup_io_error_has_operation_context() {
        with_fake_xdg(|| {
            let data_home = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
            let target = data_home.join("nmrs/certs/not-a-directory");
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(&target, "file").unwrap();

            let result = cleanup_certs("not-a-directory");

            assert!(matches!(
                result,
                Err(ConnectionError::VpnFailed(message))
                    if message.contains("cert store: remove")
                        && message.contains("nmrs/certs/not-a-directory")
            ));
        });
    }

    #[test]
    fn cleanup_rejects_unsafe_connection_names() {
        with_fake_xdg(|| {
            let result = cleanup_certs("../outside");
            assert!(matches!(
                result,
                Err(ConnectionError::InvalidAddress(message))
                    if message == "connection name must not contain path separators"
            ));
        });
    }

    #[test]
    fn xdg_data_home_falls_back_to_home_when_xdg_is_empty() {
        let (_lock, _restore) = lock_env();
        let home = std::env::temp_dir().join(format!("nmrs-home-{}", uuid::Uuid::new_v4()));
        // SAFETY: this test holds ENV_LOCK and EnvRestore restores both variables.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "");
            std::env::set_var("HOME", &home);
        }

        assert_eq!(xdg_data_home().unwrap(), home.join(".local/share"));
    }

    #[test]
    fn xdg_data_home_reports_missing_home() {
        let (_lock, _restore) = lock_env();
        // SAFETY: this test holds ENV_LOCK and EnvRestore restores both variables.
        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
            std::env::remove_var("HOME");
        }

        assert!(matches!(
            xdg_data_home(),
            Err(ConnectionError::VpnFailed(message))
                if message == "cert store: HOME is not set (cannot resolve XDG data directory)"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn permissions_are_rw_for_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        with_fake_xdg(|| {
            let p = store_inline_cert("perm", "key", "secret").unwrap();
            let file_mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            let dir_mode = std::fs::metadata(p.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
            assert_eq!(dir_mode, 0o700);
        });
    }
}
