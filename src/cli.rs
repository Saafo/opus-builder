use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "opus-builder")]
#[command(about = "Build opus-family libraries for multiple platforms")]
pub struct Cli {
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Build(BuildArgs),
    Clean(CleanArgs),
}

#[derive(Debug, Parser)]
pub struct BuildArgs {
    #[arg(
        short = 'f',
        long = "force",
        help = "Force rebuild, ignoring build/{platform} cache"
    )]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct CleanArgs {
    #[arg(short = 'b', long = "build", help = "Remove build directory")]
    pub build: bool,

    #[arg(short = 'r', long = "repo", help = "Git reset repos")]
    pub repo: bool,
}

impl CleanArgs {
    pub fn normalized(&self) -> (bool, bool) {
        if !self.build && !self.repo {
            (true, true)
        } else {
            (self.build, self.repo)
        }
    }
}
