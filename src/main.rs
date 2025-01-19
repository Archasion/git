mod commands;

use clap::Parser;
use commands::Command;
use commands::CommandArgs;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let command = args.command.parse();
    command.run()
}
