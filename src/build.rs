use crate::builder;
use crate::config;
use crate::config::{Arch, LibType, Library, Platform};
use crate::post_build;
use crate::repo;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    pub verbose: bool,
    pub force: bool,
}

pub async fn run(options: BuildOptions) -> Result<()> {
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
            let version = config.get_library_version(library)?;
            for arch in archs_for_platform {
                let can_reuse_cached_build = !options.force
                    && build_artifact_ready(
                        &config.paths.build_dir,
                        *platform,
                        *arch,
                        library,
                        lib_type_for_platform,
                    )?;
                if can_reuse_cached_build {
                    log::info!("Reusing cached {library} for {platform} ({arch})");
                } else if let Some(repo) = repo_map.get(library.repo_name()) {
                    log::info!("Building {library} for {platform} ({arch})");
                    let b = builder::Builder::new(
                        *platform,
                        *arch,
                        *library,
                        repo,
                        &config,
                        options.verbose,
                    );
                    b.build().await?;
                    log::info!("Built {library} for {platform} ({arch}) succeeded!");
                }

                package_artifact_if_needed(
                    &config.paths.build_dir,
                    *platform,
                    library,
                    version,
                    *arch,
                    lib_type_for_platform,
                )?;
            }

            if platform.is_darwin() {
                log::info!("Creating universal binary for {library} for {platform}");
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

fn build_artifact_ready(
    build_dir: &Path,
    platform: Platform,
    arch: Arch,
    library: &Library,
    lib_type: LibType,
) -> Result<bool> {
    Ok(expected_library_path(build_dir, platform, arch, library, lib_type)?.exists())
}

fn expected_library_path(
    build_dir: &Path,
    platform: Platform,
    arch: Arch,
    library: &Library,
    lib_type: LibType,
) -> Result<PathBuf> {
    let platform_dir = platform.to_string().to_lowercase();
    let arch_dir = match platform {
        Platform::Macos | Platform::Ios | Platform::IosSim => {
            crate::platforms::darwin::build::arch_dir_name(arch)?
        }
        Platform::Android => crate::platforms::android::build::arch_dir_name(arch)?,
        Platform::Harmony => crate::platforms::harmony::build::arch_dir_name(arch)?,
    };

    let ext = match platform {
        Platform::Macos | Platform::Ios | Platform::IosSim => lib_type.darwin_ext(),
        Platform::Android | Platform::Harmony => lib_type.linux_ext(),
    };
    let file_name = format!("{}.{}", library.name_with_lib_prefix(), ext);

    Ok(build_dir
        .join(platform_dir)
        .join(arch_dir)
        .join(library.repo_name())
        .join("lib")
        .join(file_name))
}

fn package_artifact_if_needed(
    build_dir: &Path,
    platform: Platform,
    library: &Library,
    version: &str,
    arch: Arch,
    lib_type: LibType,
) -> Result<()> {
    match platform {
        Platform::Android => crate::platforms::android::build::move_android_package(
            build_dir, library, version, arch, lib_type,
        ),
        Platform::Harmony => crate::platforms::harmony::build::move_harmony_package(
            build_dir, library, version, arch, lib_type,
        ),
        Platform::Macos | Platform::Ios | Platform::IosSim => Ok(()),
    }
}
