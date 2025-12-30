use crate::config::{Arch, Config, Library, Platform};
use crate::platforms::{android::AndroidBuilder, darwin::DarwinBuilder};
use crate::repo::Repo;
use anyhow::Result;

pub struct Builder<'a> {
    platform: Platform,
    arch: Arch,
    library: Library,
    repo: &'a Repo,
    config: &'a Config,
}

impl<'a> Builder<'a> {
    pub fn new(
        platform: Platform,
        arch: Arch,
        library: Library,
        repo: &'a Repo,
        config: &'a Config,
    ) -> Self {
        Self {
            platform,
            arch,
            library,
            repo,
            config,
        }
    }

    pub async fn build(&self) -> Result<()> {
        log::info!(
            "Building {} for {} ({}) from {}",
            self.library,
            self.platform,
            self.arch,
            self.repo.local_path.display()
        );

        match self.platform {
            Platform::Macos | Platform::Ios | Platform::IosSim => {
                let builder = DarwinBuilder::new();
                builder
                    .build(
                        self.platform,
                        self.arch,
                        &self.library,
                        self.repo,
                        self.config,
                    )
                    .await
            }
            Platform::Android => {
                let builder = AndroidBuilder::new();
                builder
                    .build(self.arch, &self.library, self.repo, self.config)
                    .await
            }
            Platform::Harmony => anyhow::bail!("Harmony platform not implemented yet"),
        }
    }
}
