use crate::builder::AutotoolsToolchain;
use crate::config::{Arch, Config, LibType, Library, Platform};
use anyhow::Result;
use std::fs;
use std::path::Path;
use tokio::process::Command;

pub mod build {
    use super::*;

    pub fn arch_dir_name(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::X86_64 => Ok("x86_64"),
            Arch::Arm64 => Ok("arm64"),
            _ => anyhow::bail!("Architecture not supported for Darwin platform: {:?}", arch),
        }
    }

    fn platform_dir(platform: Platform) -> Result<&'static str> {
        match platform {
            Platform::Macos => Ok("macos"),
            Platform::Ios => Ok("ios"),
            Platform::IosSim => Ok("ios-sim"),
            _ => anyhow::bail!("Platform not supported for Darwin: {:?}", platform),
        }
    }

    fn sdk_name(platform: Platform) -> Result<&'static str> {
        match platform {
            Platform::Macos => Ok("macosx"),
            Platform::Ios => Ok("iphoneos"),
            Platform::IosSim => Ok("iphonesimulator"),
            _ => anyhow::bail!("Platform not supported for Darwin: {:?}", platform),
        }
    }

    fn min_ver_flag(platform: Platform, config: &Config) -> Result<String> {
        match platform {
            Platform::Macos => Ok(format!(
                "-mmacosx-version-min={}",
                config.platforms.macos.min_version
            )),
            Platform::Ios => Ok(format!(
                "-miphoneos-version-min={}",
                config.platforms.ios.min_version
            )),
            Platform::IosSim => Ok(format!(
                "-mios-simulator-version-min={}",
                config.platforms.ios_sim.min_version
            )),
            _ => anyhow::bail!("Platform not supported for Darwin: {:?}", platform),
        }
    }

    /// Note: the host is only used for `./configure`.
    /// If we use `*-apple-ios`, configure won't support shared library,
    /// so we use `*-apple-darwin`
    fn configure_host(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::Arm64 => Ok("arm64-apple-darwin"),
            Arch::X86_64 => Ok("x86_64-apple-darwin"),
            _ => anyhow::bail!("Architecture not supported for Darwin: {:?}", arch),
        }
    }

    fn target(platform: Platform, arch: Arch) -> Result<&'static str> {
        match (platform, arch) {
            (Platform::Macos, Arch::Arm64) => Ok("arm64-apple-macos"),
            (Platform::Macos, Arch::X86_64) => Ok("x86_64-apple-macos"),
            (Platform::Ios, Arch::Arm64) => Ok("arm64-apple-ios"),
            (Platform::IosSim, Arch::Arm64) => Ok("arm64-apple-ios-simulator"),
            (Platform::IosSim, Arch::X86_64) => Ok("x86_64-apple-ios-simulator"),
            _ => anyhow::bail!(
                "{} architecture not supported for platform: {:?}",
                arch_dir_name(arch)?,
                platform
            ),
        }
    }

    async fn xcrun_show_sdk_path(sdk_name: &str) -> Result<String> {
        let sdk_root_output = Command::new("xcrun")
            .arg("--sdk")
            .arg(sdk_name)
            .arg("--show-sdk-path")
            .output()
            .await?;
        if !sdk_root_output.status.success() {
            anyhow::bail!("xcrun --show-sdk-path failed");
        }
        Ok(String::from_utf8(sdk_root_output.stdout)?
            .trim()
            .to_string())
    }

    async fn xcrun_find_tool(sdk_name: &str, tool: &str) -> Result<String> {
        let tool_output = Command::new("xcrun")
            .arg("--sdk")
            .arg(sdk_name)
            .arg("--find")
            .arg(tool)
            .output()
            .await?;
        if !tool_output.status.success() {
            anyhow::bail!("xcrun --find {} failed", tool);
        }
        Ok(String::from_utf8(tool_output.stdout)?.trim().to_string())
    }

    pub async fn prepare_toolchain(
        platform: Platform,
        arch: Arch,
        config: &Config,
    ) -> Result<AutotoolsToolchain> {
        let platform_dir = platform_dir(platform)?.to_string();
        let sdk_name = sdk_name(platform)?;
        let min_ver_flag = min_ver_flag(platform, config)?;

        let arch_dir = arch_dir_name(arch)?.to_string();

        let host = configure_host(arch)?.to_string();

        let sdk_root = xcrun_show_sdk_path(sdk_name).await?;
        let cc = xcrun_find_tool(sdk_name, "clang").await?;
        let target = target(platform, arch)?;

        let base_cflags = format!(
            "-target {target} -arch {arch_dir} -isysroot {sdk_root} {} {}",
            min_ver_flag, config.build.cflags
        );
        let base_ldflags = format!(
            "-arch {arch_dir} -isysroot {sdk_root} {} {}",
            min_ver_flag, config.build.ldflags
        );

        Ok(AutotoolsToolchain {
            platform_dir,
            arch_dir,
            host,
            cc,
            cxx: None,
            extra_env: Vec::new(),
            base_cflags,
            base_ldflags,
        })
    }

    pub async fn create_universal_binary(
        build_dir: &Path,
        platform: Platform,
        library: &Library,
        lib_type: LibType,
        archs: &[Arch],
    ) -> Result<()> {
        let universal_dir = build_dir
            .join(platform.to_string().to_lowercase())
            .join("universal")
            .join(library.repo_name());
        fs::create_dir_all(universal_dir.join("lib"))?;

        let lib_name = library.name_with_lib_prefix();
        let file_name = format!("{}.{}", lib_name, lib_type.darwin_ext());
        let lib_files: Vec<_> = archs
            .iter()
            .filter_map(|arch| {
                let arch_dir = arch_dir_name(*arch).ok()?;
                let p = build_dir
                    .join(platform.to_string().to_lowercase())
                    .join(arch_dir)
                    .join(library.repo_name())
                    .join("lib")
                    .join(&file_name);
                p.exists().then_some(p)
            })
            .collect();

        if lib_files.is_empty() {
            log::warn!(
                "Skipping universal binary for {} as no architecture-specific libraries were found.",
                lib_name
            );
            return Ok(());
        }

        let output_path = universal_dir.join("lib").join(&file_name);

        log::info!(
            "Creating universal binary for {} at {}",
            lib_name,
            output_path.display()
        );

        let mut cmd = Command::new("lipo");
        cmd.arg("-create");
        for lib_file in &lib_files {
            cmd.arg(lib_file);
        }
        cmd.arg("-output");
        cmd.arg(&output_path);

        let status = cmd.status().await?;
        if !status.success() {
            anyhow::bail!("lipo failed for {}", lib_name);
        }

        if let Some(first_arch) = archs.first().copied()
            && let Ok(first_arch_dir) = arch_dir_name(first_arch)
        {
            let include_source = build_dir
                .join(platform.to_string().to_lowercase())
                .join(first_arch_dir)
                .join(library.repo_name())
                .join("include");

            if include_source.exists() {
                let include_dest = universal_dir.join("include");
                fs::create_dir_all(&include_dest)?;

                fs_extra::dir::copy(
                    &include_source,
                    &include_dest,
                    &fs_extra::dir::CopyOptions::new()
                        .content_only(true)
                        .overwrite(true),
                )?;
            }
        }

        Ok(())
    }

    pub async fn create_xcframework(
        build_dir: &Path,
        library: &Library,
        version: &str,
        lib_type: LibType,
    ) -> Result<()> {
        let repo_name = library.repo_name();
        let lib_name = library.name_with_lib_prefix();

        let final_dir = build_dir.join("lib").join("darwin");
        fs::create_dir_all(&final_dir)?;

        let file_name = format!("{}.{}", lib_name, lib_type.darwin_ext());
        let xcframework_name = format!(
            "{}-{}.xcframework",
            lib_name,
            version.trim_start_matches('v')
        );
        let xcframework_path = final_dir.join(xcframework_name);

        if xcframework_path.exists() {
            fs::remove_dir_all(&xcframework_path)?;
        }

        let mut cmd = Command::new("xcodebuild");
        cmd.arg("-create-xcframework");

        let macos_universal_path = build_dir.join("macos").join("universal").join(repo_name);
        let ios_universal_path = build_dir.join("ios").join("universal").join(repo_name);
        let ios_sim_universal_path = build_dir.join("ios-sim").join("universal").join(repo_name);

        if macos_universal_path.exists() {
            cmd.arg("-library");
            cmd.arg(macos_universal_path.join("lib").join(&file_name));
            cmd.arg("-headers");
            cmd.arg(macos_universal_path.join("include"));
        }

        if ios_universal_path.exists() {
            cmd.arg("-library");
            cmd.arg(ios_universal_path.join("lib").join(&file_name));
            cmd.arg("-headers");
            cmd.arg(ios_universal_path.join("include"));
        }

        if ios_sim_universal_path.exists() {
            cmd.arg("-library");
            cmd.arg(ios_sim_universal_path.join("lib").join(&file_name));
            cmd.arg("-headers");
            cmd.arg(ios_sim_universal_path.join("include"));
        }

        cmd.arg("-output");
        cmd.arg(&xcframework_path);

        log::info!(
            "Creating xcframework for {} at {}",
            repo_name,
            xcframework_path.display()
        );

        let status = cmd.status().await?;
        if !status.success() {
            anyhow::bail!("xcodebuild failed for {}", repo_name);
        }

        Ok(())
    }
}
