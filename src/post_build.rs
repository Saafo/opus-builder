use crate::config::{Config, Platform};
use anyhow::Result;
use std::fs;

/// 从仓库路径复制头文件到 build/include（平台无关）
/// 只复制一次，多平台的 headers 暂时是一致的
pub fn copy_headers_from_repo(config: &Config) -> Result<()> {
    for library in &config.general.libraries {
        let lib_name = library.name_with_lib_prefix();
        let repo_name = library.repo_name();

        // 从第一个可用的平台的构建产物中复制 headers
        let mut include_source = None;

        for platform in &config.general.platforms {
            let platform_str = platform.to_string().to_lowercase();

            match platform {
                Platform::Android => {
                    // Android: 从第一个 ABI 的构建产物中复制
                    if let Some(abi) = config.platforms.android.archs.first() {
                        let abi_str = abi.to_string();
                        let path = config
                            .paths
                            .build_dir
                            .join(&platform_str)
                            .join(&abi_str)
                            .join(repo_name)
                            .join(library.include_dir());

                        if path.exists() {
                            include_source = Some(path);
                            break;
                        }
                    }
                }
                Platform::Macos | Platform::Ios | Platform::IosSim => {
                    // Darwin: 从第一个架构的构建产物中复制
                    let archs = config.platforms.get_archs_for_platform(platform);
                    if let Some(arch) = archs.first() {
                        let path = config
                            .paths
                            .build_dir
                            .join(&platform_str)
                            .join(arch.to_string())
                            .join(repo_name)
                            .join(library.include_dir());

                        if path.exists() {
                            include_source = Some(path);
                            break;
                        }
                    }
                }
                Platform::Harmony => {
                    // Harmony: 暂时跳过，逻辑与 Android 类似
                    continue;
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

            // 只复制 .h 文件
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

/// 如果构建了 Apple 平台，则创建 xcframework
pub async fn create_xcframework_if_needed(config: &Config) -> Result<()> {
    // 检查是否构建了 Apple 平台
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
        // 获取库的版本信息，如果库配置中没有指定版本，则直接报错
        let version = if let Some(lib_config) = config.libraries.get(library) {
            if let Some(v) = &lib_config.version {
                v
            } else {
                anyhow::bail!("Version not specified for library: {:?}", library);
            }
        } else {
            anyhow::bail!("Library configuration not found for: {:?}", library);
        };

        let lib_type = config.platforms.get_lib_type_for_platform(&Platform::Ios);
        crate::platforms::darwin::create_xcframework(
            &config.paths.build_dir,
            library,
            version,
            lib_type,
        )
        .await?;
    }

    Ok(())
}
