mod build;
mod cache;
mod cli;
mod config;
mod docker;
mod features;
mod join;
mod profile;
mod run;
mod stop;
mod workspace;

use anyhow::Context as _;
use clap::Parser as _;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let workspace =
        workspace::find_workspace().context("failed to locate .devcontainer directory")?;
    let profile = profile::ProfileName::new(cli.profile);

    match cli.command {
        cli::Command::Build { no_cache } => {
            build::build(&workspace, &profile, no_cache, cli.strict).await
        }
        cli::Command::Run { memory, cpus, args } => {
            let status = run::run(&workspace, &profile, &memory, &cpus, &args, cli.strict).await?;
            std::process::exit(status.code().unwrap_or(1));
        }
        cli::Command::Join => join::join(&workspace, &profile).await,
        cli::Command::Stop => stop::stop(&workspace, &profile).await,
    }
}
