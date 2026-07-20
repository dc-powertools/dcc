use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "dcc", about = "Dev Container CLI", version)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) strict: bool,
    /// Validate and report what would be done without invoking Docker.
    #[arg(long, global = true)]
    pub(crate) dry_run: bool,
    /// Output format for dry-run reports and supported structured output.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text)]
    pub(crate) format: OutputFormat,
    /// Profile to operate on. Global so it may appear before or after the
    /// subcommand (`dcc -p base build` and `dcc build -p base` are equivalent).
    #[arg(short = 'p', long, global = true, default_value = "devcontainer")]
    pub(crate) profile: String,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Build {
        #[arg(long)]
        no_cache: bool,
        #[arg(long)]
        update: bool,
        #[arg(long)]
        refresh_only: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
    },
    #[command(trailing_var_arg = true)]
    Exec {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "2")]
        cpus: String,
        /// Skip supported in-container lifecycle scripts, printing a warning for
        /// each one skipped. Useful for debugging a misbehaving script.
        #[arg(long)]
        skip_lifecycle: bool,
        /// Print the resolved launch details (env, mounts, lifecycle scripts, and
        /// the docker command) to stderr before starting the container.
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        #[arg(short = 'k', long)]
        keep: bool,
        #[arg(trailing_var_arg = true, required = true)]
        args: Vec<String>,
    },
    Start {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "2")]
        cpus: String,
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
    },
    #[command(trailing_var_arg = true)]
    Attach {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "2")]
        cpus: String,
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        #[arg(short = 'k', long)]
        keep: bool,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Stop {},
    Id {},
    Run {
        #[arg(long, default_value = "4g")]
        memory: String,
        #[arg(long, default_value = "2")]
        cpus: String,
        /// Print the resolved launch details (env, mounts, lifecycle scripts, and
        /// the docker command) to stderr before starting the container.
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        #[arg(short = 'k', long)]
        keep: bool,
        script: Option<String>,
    },
}
