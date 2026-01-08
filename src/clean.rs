use crate::config;
use crate::repo;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub verbose: bool,
    pub clean_build_dir: bool,
    pub clean_repos: bool,
}

pub async fn run(options: CleanOptions) -> Result<()> {
    let config_path = PathBuf::from("build_config.toml");
    let config = config::load_or_create_config(&config_path)?;

    if options.clean_build_dir {
        let build_dir = &config.paths.build_dir;
        if build_dir.exists() {
            fs::remove_dir_all(build_dir)?;
            log::info!("Removed {}", build_dir.display());
        }
    }

    if options.clean_repos {
        let repos = repo::get_repos(&config)?;
        for repo in &repos {
            if repo.local_path.exists() {
                repo.clean(options.verbose).await?;
            }
        }
    }

    Ok(())
}
