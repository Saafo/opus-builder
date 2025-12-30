use crate::config::{Config, Platform};
use anyhow::Result;
use std::fs;

/// 从仓库路径复制头文件到 build/include（平台无关）
pub fn copy_headers_from_repo(config: &Config) -> Result<()> {
    for library in &config.general.libraries {
        let repo_name = library.repo_name();
        let lib_name = library.lib_name();

        // 从 config.paths.repo_path 中查找仓库
        let repo_path = config
            .paths
            .repo_path
            .iter()
            .find(|p| p.join(repo_name).exists())
            .map(|p| p.join(repo_name));

        if let Some(repo_path) = repo_path {
            let include_source = repo_path.join("include");

            if !include_source.exists() {
                log::warn!(
                    "Include directory not found in repo: {}",
                    include_source.display()
                );
                continue;
            }

            let include_dest = config.paths.build_dir.join("include").join(lib_name);
            fs::create_dir_all(&include_dest)?;

            log::info!(
                "Copying headers from {} to {}",
                include_source.display(),
                include_dest.display()
            );

            // 使用 dir::copy 复制整个 include 目录
            fs_extra::dir::copy(
                &include_source,
                &include_dest,
                &fs_extra::dir::CopyOptions::new()
                    .content_only(true)
                    .overwrite(true),
            )?;
        } else {
            log::warn!("Repo path not found for library: {}", repo_name);
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

        crate::platforms::darwin::create_xcframework(&config.paths.build_dir, library, version)
            .await?;
    }

    Ok(())
}
