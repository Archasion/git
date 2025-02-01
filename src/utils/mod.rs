use std::path::PathBuf;

use anyhow::Context;

pub(crate) mod env;
pub(crate) mod hex;
pub(crate) mod objects;
pub(crate) mod test;

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
