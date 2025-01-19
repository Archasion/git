use clap::Subcommand;

mod hash_object;

impl Command {
    pub fn parse(self) -> impl CommandArgs {
        match self {
            Command::HashObject(args) => args,
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    HashObject(hash_object::HashObjectArgs),
}

pub(crate) trait CommandArgs {
    fn run(self) -> anyhow::Result<()>;
}