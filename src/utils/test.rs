//! Utility structs and functions for testing

#![cfg(test)]

use std::path::{Path, PathBuf};

/// A temporary environment for testing.
/// Changes the environment variable and restores it on drop.
/// Tests must be run serially to avoid conflicts (`cargo test -- --test-threads=1`)
///
/// # Example
///
/// ```
/// # use crate::utils::test::TempEnv;
/// let temp_env = TempEnv::new("KEY", Some("VALUE"));
/// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
///
/// // The environment variable is restored when the `TempEnv` instance is dropped
/// drop(temp_env);
///
/// // Setting the value to `None` unsets the environment variable
/// let temp_env = TempEnv::new("KEY", None);
/// assert!(std::env::var("KEY").is_err());
///
/// drop(temp_env);
/// ```
pub(crate) struct TempEnv {
    /// The environment variable's key
    key: String,
    /// The old value of the environment variable
    old_value: Option<String>,
}

impl TempEnv {
    /// Create a new temporary environment variable.
    ///
    /// * If `value` is `Some`, the environment variable is set to that value.
    /// * If `value` is `None`, the environment variable is unset.
    pub(crate) fn new<S>(key: S, value: Option<&str>) -> Self
    where
        S: Into<String>,
    {
        let key = key.into();
        let old_value = std::env::var(&key).ok();

        if let Some(value) = value {
            std::env::set_var(&key, value);
        } else {
            std::env::remove_var(&key);
        }

        TempEnv { key, old_value }
    }
}

impl Drop for TempEnv {
    fn drop(&mut self) {
        if let Some(value) = &self.old_value {
            std::env::set_var(&self.key, value);
        } else {
            std::env::remove_var(&self.key);
        }
    }
}

/// A temporary directory for testing.
/// Changes the current directory to the temporary directory and restores it on drop.
///
/// # Example
///
/// ```
/// # use crate::utils::test::TempPwd;
/// let temp_pwd = TempPwd::new();
/// assert_eq!(std::env::current_dir().unwrap(), temp_pwd.temp_pwd.path());
///
/// // The current directory is restored when the `TempPwd` instance is dropped
/// drop(temp_pwd);
/// ```
pub(crate) struct TempPwd {
    old_pwd: PathBuf,
    temp_pwd: tempfile::TempDir,
}

impl TempPwd {
    pub(crate) fn new() -> Self {
        let old_pwd = std::env::current_dir().unwrap();
        let temp_pwd = tempfile::tempdir().unwrap();

        // Change the current directory to the temporary directory
        std::env::set_current_dir(&temp_pwd).unwrap();

        Self { old_pwd, temp_pwd }
    }

    pub(crate) fn path(&self) -> &Path {
        self.temp_pwd.path()
    }
}

impl Drop for TempPwd {
    fn drop(&mut self) {
        // Restore the current directory
        std::env::set_current_dir(&self.old_pwd).unwrap();
    }
}
