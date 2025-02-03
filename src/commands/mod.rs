use std::io::Write;

use clap::Subcommand;

mod cat_file;
mod hash_object;
mod init;
mod show_ref;

impl Command {
    pub fn run(self) -> anyhow::Result<()> {
        let mut stdout = std::io::stdout();

        match self {
            Command::HashObject(args) => args.run(&mut stdout),
            Command::Init(args) => args.run(&mut stdout),
            Command::CatFile(args) => args.run(&mut stdout),
            Command::ShowRef(args) => args.run(&mut stdout),
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    HashObject(hash_object::HashObjectArgs),
    Init(init::InitArgs),
    CatFile(cat_file::CatFileArgs),
    ShowRef(show_ref::ShowRefArgs),
}

pub(crate) trait CommandArgs {
    fn run<W>(self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write;
}
