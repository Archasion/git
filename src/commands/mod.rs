use clap::Subcommand;

mod hash_object;
mod init;

impl Command {
    pub fn run(self) -> anyhow::Result<()> {
        match self {
            Command::HashObject(args) => args.run(),
            Command::Init(args) => args.run(),
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    HashObject(hash_object::HashObjectArgs),
    Init(init::InitArgs),
}

pub(crate) trait CommandArgs {
    fn run(self) -> anyhow::Result<()>;
}
