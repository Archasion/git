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
