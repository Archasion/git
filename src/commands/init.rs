use crate::commands::CommandArgs;

use clap::Parser;
use std::path::PathBuf;

impl CommandArgs for InitArgs {
    fn run(self) -> anyhow::Result<()> {
        // Initializes a new git repository in the specified directory.
        let git_dir = if self.bare {
            if let Some(directory) = self.directory {
                directory
            } else {
                let directory = std::env::current_dir()?;
                let git_dir = std::env::var("GIT_DIR").unwrap_or_else(|_| ".".to_string());
                directory.join(git_dir)
            }
        } else {
            let directory = self.directory.unwrap_or_else(|| ".".into());
            let git_dir = std::env::var("GIT_DIR").unwrap_or_else(|_| ".git".to_string());
            directory.join(git_dir)
        };

        // The directory where git objects are stored.
        let git_object_dir = std::env::var("GIT_OBJECT_DIRECTORY")
            .map(|object_dir| git_dir.join(object_dir))
            .unwrap_or_else(|_| git_dir.join("objects"));

        // Create the git directory and its subdirectories.
        std::fs::create_dir_all(&git_dir)?;
        std::fs::create_dir(git_object_dir)?;
        std::fs::create_dir(git_dir.join("refs"))?;

        let head = format!("ref: refs/heads/{}\n", self.initial_branch);
        std::fs::write(git_dir.join("HEAD"), head)?;

        if !self.quiet {
            println!(
                "Initialized empty Git repository in {}",
                git_dir.canonicalize()?.to_str().unwrap()
            );
        }
        Ok(())
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
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const INITIAL_BRANCH: &str = "main";
    const CUSTOM_GIT_DIR: &str = "custom_git_dir";
    const CUSTOM_OBJECT_DIR: &str = "custom_object_dir";

    struct TempEnv {
        old_git_dir: Option<String>,
        old_git_object_dir: Option<String>,
    }

    impl TempEnv {
        fn new(git_dir: Option<&str>, git_object_dir: Option<&str>) -> Self {
            let old_git_dir = std::env::var("GIT_DIR").ok();
            let old_git_object_dir = std::env::var("GIT_OBJECT_DIRECTORY").ok();

            if let Some(git_dir) = git_dir {
                std::env::set_var("GIT_DIR", git_dir);
            } else {
                std::env::remove_var("GIT_DIR");
            }

            if let Some(git_object_dir) = git_object_dir {
                std::env::set_var("GIT_OBJECT_DIRECTORY", git_object_dir);
            } else {
                std::env::remove_var("GIT_OBJECT_DIRECTORY");
            }

            TempEnv {
                old_git_dir,
                old_git_object_dir,
            }
        }
    }

    impl Drop for TempEnv {
        fn drop(&mut self) {
            if let Some(git_dir) = &self.old_git_dir {
                std::env::set_var("GIT_DIR", git_dir);
            } else {
                std::env::remove_var("GIT_DIR");
            }

            if let Some(git_object_dir) = &self.old_git_object_dir {
                std::env::set_var("GIT_OBJECT_DIRECTORY", git_object_dir);
            } else {
                std::env::remove_var("GIT_OBJECT_DIRECTORY");
            }
        }
    }

    #[test]
    fn init_repository() {
        let _env = TempEnv::new(None, None);

        let temp_dir = tempdir().unwrap();
        let git_dir = temp_dir.path().join(".git");
        let args = InitArgs {
            directory: Some(temp_dir.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run();
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join("objects").exists());
        assert!(git_dir.join("refs").exists());
        assert!(git_dir.join("HEAD").exists());

        let head_content = fs::read_to_string(git_dir.join("HEAD")).unwrap();
        assert_eq!(head_content, "ref: refs/heads/main\n");
    }

    #[test]
    fn init_bare_repository() {
        let _env = TempEnv::new(None, None);

        let temp_dir = tempdir().unwrap();
        let args = InitArgs {
            directory: Some(temp_dir.path().to_path_buf()),
            bare: true,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run();
        assert!(result.is_ok());
        assert!(temp_dir.path().join("objects").exists());
        assert!(temp_dir.path().join("refs").exists());
        assert!(temp_dir.path().join("HEAD").exists());

        let head_content = fs::read_to_string(temp_dir.path().join("HEAD")).unwrap();
        assert_eq!(head_content, "ref: refs/heads/main\n");
    }

    #[test]
    fn init_repository_with_branch() {
        let _env = TempEnv::new(None, None);

        let temp_dir = tempdir().unwrap();
        let git_dir = temp_dir.path().join(".git");
        let custom_branch = "develop".to_string();
        let args = InitArgs {
            directory: Some(temp_dir.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: custom_branch.clone(),
        };

        let result = args.run();
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join("HEAD").exists());

        let head_content = fs::read_to_string(git_dir.join("HEAD")).unwrap();
        assert_eq!(head_content, format!("ref: refs/heads/{}\n", custom_branch));
    }

    #[test]
    fn init_repository_with_git_dir() {
        let _env = TempEnv::new(Some(CUSTOM_GIT_DIR), None);

        let temp_dir = tempdir().unwrap();
        let git_dir = temp_dir.path().join(CUSTOM_GIT_DIR);
        let args = InitArgs {
            directory: Some(temp_dir.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run();
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join("objects").exists());
        assert!(git_dir.join("refs").exists());
        assert!(git_dir.join("HEAD").exists());

        let head_content = fs::read_to_string(git_dir.join("HEAD")).unwrap();
        assert_eq!(head_content, "ref: refs/heads/main\n");
    }

    #[test]
    fn init_repository_with_object_dir() {
        let _env = TempEnv::new(None, Some(CUSTOM_OBJECT_DIR));

        let temp_dir = tempdir().unwrap();
        let git_dir = temp_dir.path().join(".git");
        let args = InitArgs {
            directory: Some(temp_dir.path().to_path_buf()),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run();
        assert!(result.is_ok());
        assert!(git_dir.exists());
        assert!(git_dir.join(CUSTOM_OBJECT_DIR).exists());
    }

    #[test]
    fn fail_on_invalid_dir() {
        let _env = TempEnv::new(None, None);

        let args = InitArgs {
            directory: Some(PathBuf::from("/invalid/directory")),
            bare: false,
            quiet: true,
            initial_branch: INITIAL_BRANCH.to_string(),
        };

        let result = args.run();
        assert!(result.is_err());
    }
}
