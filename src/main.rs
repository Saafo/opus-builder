use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

mod builder;
mod config;
mod platforms;
mod post_build;
mod repo;
mod utils;

use config::Platform;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        unsafe { env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    let config_path = PathBuf::from("build_config.toml");
    let mut config = config::load_or_create_config(&config_path)?;

    // Sort libraries to build them in the correct order of dependency
    config.general.libraries.sort();

    log::info!("Configuration: {:#?}", config);

    let repos = repo::get_repos(&config)?;
    for repo in &repos {
        repo.ensure(config.general.verbose).await?;
        repo.clean(config.general.verbose).await?;
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
                    let builder = builder::Builder::new(*platform, *arch, *library, repo, &config);
                    builder.build().await?;
                    log::info!("Built {} for {} ({}) succeeded!", library, platform, arch);
                }
            }

            if *platform == Platform::Macos
                || *platform == Platform::Ios
                || *platform == Platform::IosSim
            {
                log::info!("Creating universal binary for {} for {}", library, platform);
                crate::platforms::darwin::create_universal_binary(
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

    // 如果构建了 Apple 平台，则创建 xcframework
    post_build::create_xcframework_if_needed(&config).await?;

    // 统一复制头文件到 build/include（从仓库路径，平台无关）
    post_build::copy_headers_from_repo(&config)?;

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
