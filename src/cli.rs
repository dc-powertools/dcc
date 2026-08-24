use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "dcc", about = "Dev Container CLI", version)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) strict: bool,
    /// Validate and report what would be done without invoking Docker.
    #[arg(long, global = true)]
    pub(crate) dry_run: bool,
    /// Print resolved launch or command details to stderr before acting.
    #[arg(long, global = true)]
    pub(crate) debug: bool,
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
        refresh_only: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        /// Overwrite developer-modified declared state during seeding instead of
        /// preserving it. All-or-nothing across every declared state path.
        #[arg(long = "reseed-state")]
        reseed_state: bool,
    },
    #[command(trailing_var_arg = true)]
    Exec {
        #[arg(long)]
        memory: Option<String>,
        #[arg(long)]
        cpus: Option<String>,
        /// Skip supported in-container lifecycle scripts, printing a warning for
        /// each one skipped. Useful for debugging a misbehaving script.
        #[arg(long)]
        skip_lifecycle: bool,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        #[arg(short = 'k', long)]
        keep: bool,
        #[arg(trailing_var_arg = true, required = true)]
        args: Vec<String>,
    },
    Start {
        #[arg(long)]
        memory: Option<String>,
        #[arg(long)]
        cpus: Option<String>,
        #[arg(long)]
        allow_unsafe_runtime: bool,
    },
    #[command(trailing_var_arg = true)]
    Attach {
        #[arg(long)]
        memory: Option<String>,
        #[arg(long)]
        cpus: Option<String>,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        #[arg(short = 'k', long)]
        keep: bool,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Stop {
        /// Force-terminate running commands, run shutdown hooks, then exit.
        #[arg(long)]
        now: bool,
        /// Unconditionally kill the container (`docker kill`). Emergency path for
        /// wedged or corrupted containers.
        #[arg(long)]
        kill: bool,
    },
    Id {},
    Feature {
        /// Add a Feature reference to the selected profile.
        #[arg(short = 'a', long = "add", value_name = "FEATURE")]
        add: Vec<String>,
        /// Remove a Feature reference from the selected profile.
        #[arg(short = 'r', long = "remove", value_name = "FEATURE")]
        remove: Vec<String>,
    },
    Run {
        #[arg(long)]
        memory: Option<String>,
        #[arg(long)]
        cpus: Option<String>,
        #[arg(long)]
        allow_unsafe_runtime: bool,
        #[arg(short = 'k', long)]
        keep: bool,
        script: Option<String>,
    },
}
