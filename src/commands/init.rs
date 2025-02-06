use std::io::Write;
use std::path::PathBuf;

use clap::Parser;

use crate::commands::CommandArgs;
use crate::utils::env;

impl CommandArgs for InitArgs {
    fn run<W>(self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        let init_path = get_init_path(self.directory, self.bare)?;

        // The directory where git objects are stored.
        // GIT_OBJECT_DIRECTORY takes precedence over the default 'objects' directory.
        let object_dir = std::env::var(env::GIT_OBJECT_DIRECTORY)
            .map(|object_dir| init_path.join(object_dir))
            .unwrap_or_else(|_| init_path.join("objects"));

        // Create the git directory and its subdirectories.
        std::fs::create_dir_all(object_dir)?;
        std::fs::create_dir(init_path.join("refs"))?;

        // Create the main HEAD file.
        std::fs::write(
            init_path.join("HEAD"),
            get_head_ref_content(&self.initial_branch),
        )?;

        // Only print the output if the `--quiet` flag is not passed.
        if !self.quiet {
            let output = format!(
                "Initialized empty Git repository in {}",
                init_path.canonicalize()?.to_str().unwrap()
            );
            writer.write_all(output.as_bytes())?;
        }

        Ok(())
    }
}

/// Returns the content of the HEAD file.
fn get_head_ref_content(initial_branch: &str) -> String {
    format!("ref: refs/heads/{}\n", initial_branch)
}

/// Returns the path to initialize the git repository.
///
/// - If the target directory is not specified, the current directory is used.
/// - If the `--bare` flag is passed, the target directory is used as the .git directory (unless GIT_DIR is set).
/// - If the `--bare` flag is not passed, a .git directory is created in the target directory.
///
/// > Note: The `GIT_DIR` environment variable takes precedence over the default `.git` directory.
///
/// # Arguments
///
/// * `target_dir` - The directory to create the repository in.
/// * `bare` - Create a bare repository.
///
/// # Returns
///
/// The path to initialize the git repository.
fn get_init_path(target_dir: Option<PathBuf>, bare: bool) -> anyhow::Result<PathBuf> {
    // Creates a .git directory in the target directory.
    if !bare {
        // If the target directory is not specified, use the current directory.
        let target_dir = target_dir.unwrap_or_else(|| ".".into());
        // Prioritize the GIT_DIR environment variable over '.git'
        let git_dir = std::env::var(env::GIT_DIR).unwrap_or_else(|_| ".git".to_string());
        return Ok(target_dir.join(git_dir));
    }

    // Creates a bare repository in the target directory.
    // A bare repository uses the target directory as the .git directory.
    if let Some(target_dir) = target_dir {
        Ok(target_dir)
    } else {
        // If the target directory is not specified, use the path defined
        // in GIT_DIR or the current directory.
        let target_dir = std::env::var(env::GIT_DIR).unwrap_or_else(|_| ".".to_string());
        Ok(target_dir.into())
    }
}

#[derive(Parser, Debug)]
pub(crate) struct InitArgs {
    /// directory to create the repository in
    #[arg(name = "directory")]
    directory: Option<PathBuf>,
    /// create a bare repository
    #[arg(long)]
    bare: bool,
    /// be quiet
    #[arg(short, long)]
    quiet: bool,
    /// override the name of the initial branch
    #[arg(short = 'b', long, default_value = "main", name = "name")]
    initial_branch: String,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::InitArgs;
    use crate::commands::CommandArgs;
    use crate::utils::env;
    use crate::utils::test::{TempEnv, TempPwd};

    const INITIAL_BRANCH: &str = "main";
    const CUSTOM_GIT_DIR: &str = "custom_git_dir";
    const CUSTOM_OBJECT_DIR: &str = "custom_object_dir";

    #[test]
    fn inits_repo() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let pwd = TempPwd::new();
        let git_dir = pwd.path().join(".git");
        let args = InitArgs {
            directory: Some(pwd.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join("objects").exists());
        assert!(git_dir.join("refs").exists());
        assert!(git_dir.join("HEAD").exists());

        let head_content = fs::read_to_string(git_dir.join("HEAD")).unwrap();
        assert_eq!(head_content, "ref: refs/heads/main\n");
    }

    #[test]
    fn inits_bare_repo() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let pwd = TempPwd::new();
        let args = InitArgs {
            directory: Some(pwd.path().to_path_buf()),
            bare: true,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_ok());
        assert!(pwd.path().join("objects").exists());
        assert!(pwd.path().join("refs").exists());
        assert!(pwd.path().join("HEAD").exists());

        let head_content = fs::read_to_string(pwd.path().join("HEAD")).unwrap();
        assert_eq!(head_content, "ref: refs/heads/main\n");
    }

    #[test]
    fn inits_repo_with_branch() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let pwd = TempPwd::new();
        let git_dir = pwd.path().join(".git");
        let custom_branch = "develop".to_string();
        let args = InitArgs {
            directory: Some(pwd.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: custom_branch.clone(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join("HEAD").exists());

        let head_content = fs::read_to_string(git_dir.join("HEAD")).unwrap();
        assert_eq!(head_content, format!("ref: refs/heads/{}\n", custom_branch));
    }

    #[test]
    fn inits_repo_with_custom_git_dir() {
        let _env = TempEnv::from([
            (env::GIT_DIR, Some(CUSTOM_GIT_DIR)),
            (env::GIT_OBJECT_DIRECTORY, None),
        ]);

        let pwd = TempPwd::new();
        let git_dir = pwd.path().join(CUSTOM_GIT_DIR);
        let args = InitArgs {
            directory: Some(pwd.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join("objects").exists());
        assert!(git_dir.join("refs").exists());
        assert!(git_dir.join("HEAD").exists());

        let head_content = fs::read_to_string(git_dir.join("HEAD")).unwrap();
        assert_eq!(head_content, "ref: refs/heads/main\n");
    }

    #[test]
    fn inits_repo_with_custom_git_object_dir() {
        let _env = TempEnv::from([
            (env::GIT_DIR, None),
            (env::GIT_OBJECT_DIRECTORY, Some(CUSTOM_OBJECT_DIR)),
        ]);

        let pwd = TempPwd::new();
        let git_dir = pwd.path().join(".git");
        let args = InitArgs {
            directory: Some(pwd.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join(CUSTOM_OBJECT_DIR).exists());
    }

    #[test]
    fn fail_on_invalid_init_path() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let args = InitArgs {
            directory: Some(PathBuf::from("/invalid/directory")),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }
}
