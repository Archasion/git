//! Utility structs and functions for testing

#![cfg(test)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A temporary environment for testing.
/// Changes the environment variable and restores it on drop.
/// Tests must be run serially to avoid conflicts (`cargo test -- --test-threads=1`)
pub(crate) struct TempEnv(HashMap<String, Option<String>>);

impl TempEnv {
    /// Set a new temporary environment variable.
    /// 
    /// # Example
    /// 
    /// ```
    /// # use crate::utils::test::TempEnv;
    /// let temp_env = TempEnv::set("KEY", "VALUE");
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    /// 
    /// // The environment variable is restored when the `TempEnv` instance is dropped
    /// drop(temp_env);
    /// assert!(std::env::var("KEY").is_err());
    #[allow(dead_code)]
    pub(crate) fn set<S>(key: S, value: &str) -> Self
    where
        S: Into<String>,
    {
        let key = key.into();
        // Get the current value of the environment variable
        let old_value = std::env::var(&key).ok();
        std::env::set_var(&key, value);
        // Store the previous state of the environment variable
        TempEnv(HashMap::from([(key, old_value)]))
    }
    
    /// Unset a temporary environment variable.
    /// 
    /// # Example
    /// 
    /// ```
    /// # use crate::utils::test::TempEnv;
    /// let temp_env = TempEnv::unset("KEY");
    /// assert!(std::env::var("KEY").is_err());
    #[allow(dead_code)]
    pub(crate) fn unset<S>(key: S) -> Self
    where
        S: Into<String>,
    {
        let key = key.into();
        // Get the current value of the environment variable
        let old_value = std::env::var(&key).ok();
        std::env::remove_var(&key);
        // Store the previous state of the environment variable
        TempEnv(HashMap::from([(key, old_value)]))
    }
}

impl<S, const N: usize> From<[(S, Option<&str>); N]> for TempEnv
where
    S: Into<String> + Clone,
{
    /// Create a new temporary environment variable from an array of key-value pairs.
    /// See [`TempEnv::new`] for more information.
    ///
    /// # Example
    ///
    /// ```
    /// # use crate::utils::test::TempEnv;
    /// let temp_env = TempEnv::from([("KEY1", Some("VALUE1")), ("KEY2", None)]);
    /// assert_eq!(std::env::var("KEY1"), Ok("VALUE1".to_string()));
    /// assert!(std::env::var("KEY2").is_err());
    ///
    /// // The environment variables are restored when the `TempEnv` instance is dropped
    /// drop(temp_env);
    fn from(slice: [(S, Option<&str>); N]) -> Self {
        let mut map = HashMap::with_capacity(N);

        for (key, value) in slice.iter() {
            let key: String = key.clone().into();
            // Get the current value of the environment variable
            let old_value = std::env::var(&key).ok();

            // Set or unset the environment variable
            if let Some(value) = value {
                std::env::set_var(&key, value);
            } else {
                std::env::remove_var(&key);
            }

            // Store the previous state of the environment variable
            map.insert(key, old_value);
        }

        TempEnv(map)
    }
}

impl Drop for TempEnv {
    fn drop(&mut self) {
        for (key, old_value) in self.0.iter() {
            // Restore the previous state of the environment variable
            if let Some(old_value) = old_value {
                std::env::set_var(key, old_value);
            } else {
                std::env::remove_var(key);
            }
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
