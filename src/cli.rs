use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "dcc", about = "Dev Container CLI")]
pub(crate) struct Cli {
    #[arg(short = 'p', long, default_value = "devcontainer")]
    pub(crate) profile: String,
    #[arg(long)]
    pub(crate) strict: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Build {
        #[arg(long)]
        no_cache: bool,
    },
    #[command(trailing_var_arg = true)]
    Run {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "4")]
        cpus: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Join,
    Stop,
}
