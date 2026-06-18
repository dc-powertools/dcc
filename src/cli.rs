use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "dcc", about = "Dev Container CLI", version)]
pub(crate) struct Cli {
    #[arg(long)]
    pub(crate) strict: bool,
    /// Profile to operate on. Global so it may appear before or after the
    /// subcommand (`dcc -p base build` and `dcc build -p base` are equivalent).
    #[arg(short = 'p', long, global = true, default_value = "devcontainer")]
    pub(crate) profile: String,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Build {
        #[arg(long)]
        no_cache: bool,
        #[arg(long)]
        update: bool,
    },
    #[command(trailing_var_arg = true)]
    Exec {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "4")]
        cpus: String,
        #[arg(trailing_var_arg = true, required = true)]
        args: Vec<String>,
    },
    Join {},
    Stop {},
    Id {},
    Run {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "4")]
        cpus: String,
        script: Option<String>,
    },
}
