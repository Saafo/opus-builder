use crate::builder;
use crate::config;
use crate::config::Platform;
use crate::post_build;
use crate::repo;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    pub verbose: bool,
    pub force: bool,
}

pub async fn run(options: BuildOptions) -> Result<()> {
    let _ = options.force;
    let config_path = PathBuf::from("build_config.toml");
    let mut config = config::load_or_create_config(&config_path)?;

    config.general.libraries.sort();

    log::info!("Configuration: {:#?}", config);

    let repos = repo::get_repos(&config)?;
    for repo in &repos {
        repo.ensure(options.verbose).await?;
        repo.clean(options.verbose).await?;
    }

    let repo_map: HashMap<_, _> = repos.iter().map(|r| (r.name.as_str(), r)).collect();

    for platform in &config.general.platforms {
        let archs_for_platform = config.platforms.get_archs_for_platform(platform);
        let lib_type_for_platform = config.platforms.get_lib_type_for_platform(platform);

        for library in &config.general.libraries {
            for arch in archs_for_platform {
                let repo_name = library.repo_name();
                if let Some(repo) = repo_map.get(repo_name) {
                    log::info!("Building {} for {} ({})", library, platform, arch);
                    let b = builder::Builder::new(
                        *platform,
                        *arch,
                        *library,
                        repo,
                        &config,
                        options.verbose,
                    );
                    b.build().await?;
                    log::info!("Built {} for {} ({}) succeeded!", library, platform, arch);
                }
            }

            if *platform == Platform::Macos
                || *platform == Platform::Ios
                || *platform == Platform::IosSim
            {
                log::info!("Creating universal binary for {} for {}", library, platform);
                crate::platforms::darwin::build::create_universal_binary(
                    &config.paths.build_dir,
                    *platform,
                    library,
                    lib_type_for_platform,
                    archs_for_platform,
                )
                .await?;
            }
        }
    }

    post_build::create_xcframework_if_needed(&config).await?;
    post_build::copy_headers_from_build_artifacts(&config)?;

    if !config.general.keep_intermediate {
        log::info!("Cleaning up intermediate build artifacts");
        for platform in &config.general.platforms {
            let platform_str = platform.to_string().to_lowercase();
            let path = config.paths.build_dir.join(platform_str);
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
        }
    }

    println!("\n🎉 Build completed successfully!\n");

    Ok(())
}
