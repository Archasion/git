use std::path::PathBuf;

use anyhow::Context;
use clap::Subcommand;

mod cat_file;
mod hash_object;
mod init;

impl Command {
    pub fn run(self) -> anyhow::Result<()> {
        match self {
            Command::HashObject(args) => args.run(),
            Command::Init(args) => args.run(),
            Command::CatFile(args) => args.run(),
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    HashObject(hash_object::HashObjectArgs),
    Init(init::InitArgs),
    CatFile(cat_file::CatFileArgs),
}

pub(crate) trait CommandArgs {
    fn run(self) -> anyhow::Result<()>;
}

fn get_current_dir() -> anyhow::Result<PathBuf> {
    std::env::current_dir().context("get path of current directory")
}

fn git_dir() -> anyhow::Result<PathBuf> {
    let git_dir_path = std::env::var("GIT_DIR").unwrap_or_else(|_| ".git".to_string());
    let mut current_dir = get_current_dir()?;

    while current_dir.exists() {
        let git_dir = current_dir.join(&git_dir_path);

        if git_dir.exists() {
            return Ok(git_dir);
        }

        current_dir = current_dir
            .parent()
            .context("get path of parent directory")?
            .to_path_buf();
    }

    anyhow::bail!("not a git repository (or any of the parent directories): .git")
}

fn git_object_dir() -> anyhow::Result<PathBuf> {
    let git_object_dir_path =
        std::env::var("GIT_OBJECT_DIRECTORY").unwrap_or_else(|_| "objects".to_string());

    git_dir().map(|git_dir| git_dir.join(git_object_dir_path))
}

fn get_object_path(object: &str) -> anyhow::Result<PathBuf> {
    let object_dir = git_object_dir()?;
    let object_dir = object_dir.join(&object[..2]);
    let object_path = object_dir.join(&object[2..]);

    if !object_path.exists() {
        anyhow::bail!("fatal: object '{}' not found", object);
    }

    Ok(object_path)
}
