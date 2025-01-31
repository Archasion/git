use std::fmt;
use std::path::PathBuf;

use anyhow::Context;
use clap::ValueEnum;

const HEX_CHARS: &[u8] = b"0123456789abcdef";

/// Convert a binary slice to a hex slice.
pub(crate) fn binary_to_hex_bytes(bytes: &mut Vec<u8>) {
    for _ in 0..bytes.len() {
        let byte = bytes.remove(0);
        bytes.push(HEX_CHARS[(byte >> 4) as usize]);
        bytes.push(HEX_CHARS[(byte & 0xf) as usize]);
    }
}

/// Format the header of a `.git/objects` file
pub(crate) fn format_header<O, S>(object_type: O, size: S) -> String
where
    O: fmt::Display,
    S: fmt::Display,
{
    format!("{} {}\0", object_type, size)
}

/// Parse the header of a `.git/objects` file into the [`ObjectHeader`] struct.
pub(crate) fn parse_header(header: &[u8]) -> anyhow::Result<ObjectHeader> {
    // Split the header into type and size
    let mut header = header.splitn(2, |&b| b == b' ');

    let object_type = header.next().context("invalid object header")?;
    let size = header.next().context("invalid object header")?;
    let size = &size[..size.len().saturating_sub(1)]; // Remove the trailing null byte

    Ok(ObjectHeader { object_type, size })
}

/// Get the path of the current directory.
pub(crate) fn get_current_dir() -> anyhow::Result<PathBuf> {
    std::env::current_dir().context("get path of current directory")
}

/// Get the path to the git directory.
/// This could be either of the following (in order of precedence):
///
/// 1. `$GIT_DIR`
/// 2. `.git`
///
/// # Returns
///
/// The path to the git directory
pub(crate) fn git_dir() -> anyhow::Result<PathBuf> {
    let git_dir_path = std::env::var(env::GIT_DIR).unwrap_or_else(|_| ".git".to_string());
    let mut current_dir = get_current_dir()?;

    // Search for the git directory in the current directory and its parents
    while current_dir.exists() {
        let git_dir = current_dir.join(&git_dir_path);

        // Return the git directory if it exists
        if git_dir.exists() {
            return Ok(git_dir);
        }

        let Some(parent_dir) = current_dir.parent() else {
            break;
        };

        current_dir = parent_dir.to_path_buf();
    }

    anyhow::bail!(
        "not a git repository (or any of the parent directories): {}",
        git_dir_path
    )
}

/// Get the path to the git object directory.
/// This could be either of the following (in order of precedence):
///
/// 1. `<git_directory>/$GIT_OBJECT_DIRECTORY`
/// 2. `<git_directory>/objects`
///
/// # Arguments
///
/// * `check_exists` - Whether to check if the object directory exists,
///   exiting with an error if it does not
///
/// # Returns
///
/// The path to the git object directory
pub(crate) fn git_object_dir(check_exists: bool) -> anyhow::Result<PathBuf> {
    let git_dir = git_dir()?;
    let git_object_dir =
        std::env::var(env::GIT_OBJECT_DIRECTORY).unwrap_or_else(|_| "objects".to_string());
    let git_object_dir = git_dir.join(&git_object_dir);

    // Check if the object directory exists
    if check_exists && !git_object_dir.exists() {
        anyhow::bail!(
            "{}/{} directory does not exist",
            git_dir.display(),
            git_object_dir.display()
        );
    }

    Ok(git_object_dir)
}

/// Get the path to a git object.
/// The path is constructed as follows:
///
/// `<git_object_directory>/<hash[..2]>/<hash[2..]>`
///
/// # Example
///
/// If the default git and object directories are used,
/// the path for object `e7a11a969c037e00a796aafeff6258501ec15e9a` would be:
///
/// `.git/objects/e7/a11a969c037e00a796aafeff6258501ec15e9a`
///
/// # Arguments
///
/// * `hash` - The object hash
/// * `check_exists` - Whether to check if the object exists,
///   exiting with an error if it does not
///
/// # Returns
///
/// The path to the object file
pub(crate) fn get_object_path(hash: &str, check_exists: bool) -> anyhow::Result<PathBuf> {
    let object_dir = git_object_dir(check_exists)?;
    let object_dir = object_dir.join(&hash[..2]);
    let object_path = object_dir.join(&hash[2..]);

    // Check if the object exists
    if check_exists && !object_path.exists() {
        anyhow::bail!("{} is not a valid object", hash);
    }

    Ok(object_path)
}

/// The type of object in the Git object database
#[derive(Default, Debug, ValueEnum, Clone)]
pub(crate) enum ObjectType {
    #[default]
    Blob,
    Tree,
    Commit,
    Tag,
}

/// The header of a Git object
pub(crate) struct ObjectHeader<'a> {
    /// The type of object
    pub(crate) object_type: &'a [u8],
    /// The size of the object in bytes
    pub(crate) size: &'a [u8],
}

impl ObjectHeader<'_> {
    /// Parse the size of the object
    pub(crate) fn parse_size(&self) -> anyhow::Result<usize> {
        let size = std::str::from_utf8(self.size)
            .context("object size is not valid utf-8")?
            .parse::<usize>()
            .context("object size is not a number")?;

        Ok(size)
    }

    /// Parse the type of the object
    pub(crate) fn parse_type(&self) -> anyhow::Result<ObjectType> {
        ObjectType::try_from(self.object_type)
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::Blob => write!(f, "blob"),
            ObjectType::Tree => write!(f, "tree"),
            ObjectType::Commit => write!(f, "commit"),
            ObjectType::Tag => write!(f, "tag"),
        }
    }
}

impl TryFrom<&[u8]> for ObjectType {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> anyhow::Result<Self> {
        match value {
            b"blob" => Ok(ObjectType::Blob),
            b"tree" => Ok(ObjectType::Tree),
            b"commit" => Ok(ObjectType::Commit),
            b"tag" => Ok(ObjectType::Tag),
            _ => {
                let value = std::str::from_utf8(value).context("object type is not valid utf-8")?;
                anyhow::bail!("unknown object type: {}", value)
            },
        }
    }
}

/// Utility structs and functions for testing
#[cfg(test)]
pub(crate) mod test {
    use std::path::{Path, PathBuf};

    use super::binary_to_hex_bytes;

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

    #[test]
    fn valid_binary_to_hex_bytes() {
        let mut binary = vec![0x00, 0x01, 0x02, 0x03];
        binary_to_hex_bytes(&mut binary);
        assert_eq!(binary, b"00010203");
    }
}

/// Environment variables used by the Git CLI
pub(crate) mod env {
    pub(crate) const GIT_DIR: &str = "GIT_DIR";
    pub(crate) const GIT_OBJECT_DIRECTORY: &str = "GIT_OBJECT_DIRECTORY";
}
