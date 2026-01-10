use crate::config::{Config, Platform};
use anyhow::Result;
use std::fs;

pub fn copy_headers_from_build_artifacts(config: &Config) -> Result<()> {
    for library in &config.general.libraries {
        let lib_name = library.name_with_lib_prefix();
        let repo_name = library.repo_name();

        // copy headers from first available platform since headers are same
        let mut include_source = None;

        for platform in &config.general.platforms {
            let platform_str = platform.to_string().to_lowercase();

            match platform {
                Platform::Android => {
                    if let Some(abi) = config.platforms.android.archs.first() {
                        let Ok(abi_str) = crate::platforms::android::build::arch_dir_name(*abi)
                        else {
                            continue;
                        };
                        let path = config
                            .paths
                            .build_dir
                            .join(&platform_str)
                            .join(abi_str)
                            .join(repo_name)
                            .join(library.include_dir());

                        if path.exists() {
                            include_source = Some(path);
                            break;
                        }
                    }
                }
                Platform::Macos | Platform::Ios | Platform::IosSim => {
                    let archs = config.platforms.get_archs_for_platform(platform);
                    if let Some(arch) = archs.first() {
                        let Ok(arch_dir) = crate::platforms::darwin::build::arch_dir_name(*arch)
                        else {
                            continue;
                        };
                        let path = config
                            .paths
                            .build_dir
                            .join(&platform_str)
                            .join(arch_dir)
                            .join(repo_name)
                            .join(library.include_dir());

                        if path.exists() {
                            include_source = Some(path);
                            break;
                        }
                    }
                }
                Platform::Harmony => {
                    todo!("similar to android")
                }
            }
        }

        if let Some(include_source) = include_source {
            let include_dest = config.paths.build_dir.join("include").join(lib_name);
            fs::create_dir_all(&include_dest)?;

            log::info!(
                "Copying headers from {} to {}",
                include_source.display(),
                include_dest.display()
            );

            // copy header files only
            for entry in fs::read_dir(&include_source)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().is_some_and(|ext| ext == "h") && path.is_file() {
                    let dest_file = include_dest.join(path.file_name().unwrap());
                    fs::copy(&path, &dest_file)?;
                    log::debug!(
                        "Copied header: {}",
                        path.file_name().unwrap().to_string_lossy()
                    );
                }
            }
        } else {
            log::warn!(
                "No include directory found in build artifacts for library: {}",
                lib_name
            );
        }
    }

    Ok(())
}

/// Create an xcframework if any Apple platform was built.
pub async fn create_xcframework_if_needed(config: &Config) -> Result<()> {
    let has_apple_platform = config
        .general
        .platforms
        .iter()
        .any(|p| matches!(p, Platform::Macos | Platform::Ios | Platform::IosSim));

    if !has_apple_platform {
        log::info!("No Apple platforms built, skipping xcframework creation");
        return Ok(());
    }

    for library in &config.general.libraries {
        let version = config.get_library_version(library)?;

        let lib_type = config.platforms.get_lib_type_for_platform(&Platform::Ios);
        crate::platforms::darwin::build::create_xcframework(
            &config.paths.build_dir,
            library,
            version,
            lib_type,
        )
        .await?;
    }

    Ok(())
}
