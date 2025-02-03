extern crate core;

mod commands;
mod utils;

use clap::Parser;
use commands::Command;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    args.command.run()
}
