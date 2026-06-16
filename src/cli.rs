use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "dcc", about = "Dev Container CLI", version)]
pub(crate) struct Cli {
    #[arg(long)]
    pub(crate) strict: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Build {
        #[arg(short = 'p', long, default_value = "devcontainer")]
        profile: String,
        #[arg(long)]
        no_cache: bool,
        #[arg(long)]
        update: bool,
    },
    #[command(trailing_var_arg = true)]
    Exec {
        #[arg(short = 'p', long, default_value = "devcontainer")]
        profile: String,
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "4")]
        cpus: String,
        #[arg(trailing_var_arg = true, required = true)]
        args: Vec<String>,
    },
    Join {
        #[arg(short = 'p', long, default_value = "devcontainer")]
        profile: String,
    },
    Stop {
        #[arg(short = 'p', long, default_value = "devcontainer")]
        profile: String,
    },
    Id {
        #[arg(short = 'p', long, default_value = "devcontainer")]
        profile: String,
    },
    Run {
        #[arg(short = 'p', long, default_value = "devcontainer")]
        profile: String,
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "4")]
        cpus: String,
        script: Option<String>,
    },
}
