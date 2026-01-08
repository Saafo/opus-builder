use std::env;

use clap::{CommandFactory, Parser};

mod build;
mod builder;
mod clean;
mod cli;
mod config;
mod platforms;
mod post_build;
mod repo;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    let cli = cli::Cli::parse();

    let Some(command) = cli.command else {
        cli::Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        cli::Commands::Build(args) => {
            build::run(build::BuildOptions {
                verbose: cli.verbose,
                force: args.force,
            })
            .await?;
        }
        cli::Commands::Clean(args) => {
            let (clean_build_dir, clean_repos) = args.normalized();
            clean::run(clean::CleanOptions {
                verbose: cli.verbose,
                clean_build_dir,
                clean_repos,
            })
            .await?;
        }
    }
    Ok(())
}
