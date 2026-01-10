use crate::config::Config;
use crate::utils::CommandVerboseExt;
use anyhow::Context;
use anyhow::Result;
use glob::glob;
use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Debug)]
pub struct Repo {
    pub name: String,
    pub url: String,
    pub local_path: PathBuf,
    pub version: String,
}

impl Repo {
    pub async fn ensure(&self, verbose: bool) -> Result<()> {
        if self.local_path.exists() {
            log::info!(
                "Found repo '{}' at {}",
                self.name,
                self.local_path.display()
            );
        } else {
            log::info!(
                "Cloning repo '{}' from {} to {}",
                self.name,
                self.url,
                self.local_path.display()
            );
            Command::new("git")
                .arg("clone")
                .arg(&self.url)
                .arg(&self.local_path)
                .run_with_verbose(verbose)
                .await
                .context(format!("Failed to clone repo '{}'", self.name))?;
        }

        log::info!(
            "Checking out version '{}' for repo '{}'",
            self.version,
            self.name
        );
        Command::new("git")
            .arg("checkout")
            .arg(&self.version)
            .current_dir(&self.local_path)
            .run_with_verbose(verbose)
            .await
            .context(format!(
                "Failed to checkout version '{}' for repo '{}'",
                self.version, self.name
            ))?;

        Ok(())
    }

    fn cache_opus_model_before_clean(&self) -> Result<()> {
        let opus_model_dir = PathBuf::from("opus-model");
        if !opus_model_dir.exists() {
            fs::create_dir_all(&opus_model_dir)?;
        }

        let model_in_cache_pattern = opus_model_dir.join("opus_data-*.tar.gz");
        let model_in_cache = glob(&model_in_cache_pattern.to_string_lossy())?
            .next()
            .is_some();

        if !model_in_cache {
            let model_in_repo_pattern = self.local_path.join("opus_data-*.tar.gz");
            if let Some(Ok(model_in_repo)) = glob(&model_in_repo_pattern.to_string_lossy())?.next()
                && let Some(model_filename) = model_in_repo.file_name()
            {
                log::info!(
                    "Caching opus model file: {}",
                    model_filename.to_string_lossy()
                );
                fs::copy(&model_in_repo, opus_model_dir.join(model_filename))?;
            }
        }
        Ok(())
    }

    fn restore_opus_model_after_clean(&self) -> Result<()> {
        let opus_model_dir = PathBuf::from("opus-model");
        let model_in_cache_pattern = opus_model_dir.join("opus_data-*.tar.gz");

        if let Some(Ok(model_in_cache)) = glob(&model_in_cache_pattern.to_string_lossy())?.next()
            && let Some(model_filename) = model_in_cache.file_name()
        {
            log::info!(
                "Restoring opus model file: {}",
                model_filename.to_string_lossy()
            );
            fs::copy(&model_in_cache, self.local_path.join(model_filename))?;
        }
        Ok(())
    }

    pub async fn clean(&self, verbose: bool) -> Result<()> {
        if self.name == "opus" {
            self.cache_opus_model_before_clean()?;
        }

        log::info!("Cleaning repo '{}'", self.name);
        Command::new("git")
            .arg("reset")
            .arg("--hard")
            .current_dir(&self.local_path)
            .run_with_verbose(verbose)
            .await
            .context(format!("Failed to clean repo '{}'", self.name))?;

        Command::new("git")
            .arg("clean")
            .arg("-fdx")
            .current_dir(&self.local_path)
            .run_with_verbose(verbose)
            .await
            .context(format!("Failed to clean repo '{}'", self.name))?;

        if self.name == "opus" {
            self.restore_opus_model_after_clean()?;
        }

        Ok(())
    }
}

pub fn get_repos(config: &Config) -> anyhow::Result<Vec<Repo>> {
    let repo_prefix = &config.general.repo_prefix;

    let mut search_paths = config.paths.repo_path.to_vec();
    let current_dir = env::current_dir()?;
    search_paths.push(current_dir.clone());
    let mut parent = current_dir.parent();
    while let Some(p) = parent {
        search_paths.push(p.to_path_buf());
        parent = p.parent();
    }

    let mut repos = Vec::new();
    for lib in &config.general.libraries {
        let name = lib.repo_name();
        let url = format!("{}{}.git", repo_prefix, name);

        let version = if let Some(lib_config) = config.libraries.get(lib) {
            if let Some(v) = &lib_config.version {
                v
            } else {
                anyhow::bail!("Version not specified for library: {:?}", lib);
            }
        } else {
            anyhow::bail!("Library configuration not found for: {:?}", lib);
        };

        let local_path = search_paths
            .iter()
            .find_map(|p| {
                let potential_path = p.join(name);
                if potential_path.exists() {
                    log::info!("Found repo '{}' at {}", name, potential_path.display());
                    Some(potential_path)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| PathBuf::from("repos").join(name));

        repos.push(Repo {
            name: name.to_string(),
            url: url.to_string(),
            local_path,
            version: version.to_string(),
        });
    }
    Ok(repos)
}
