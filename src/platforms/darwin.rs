use crate::config::{Arch, Config, LibType, Library, Platform};
use crate::repo::Repo;
use anyhow::Result;
use std::fs;
use std::path::Path;
use tokio::process::Command;

pub struct DarwinBuilder;

impl DarwinBuilder {
    pub fn new() -> Self {
        Self
    }

    async fn build_autotools(
        &self,
        platform_name: &str,
        arch_str: &str,
        host: &str,
        sdk_name: &str,
        min_ver_flag: &str,
        library: &Library,
        repo: &Repo,
        config: &Config,
    ) -> Result<()> {
        let autogen_path = repo.local_path.join("autogen.sh");
        if autogen_path.exists() {
            let status = Command::new("sh")
                .arg("./autogen.sh")
                .current_dir(&repo.local_path)
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("autogen.sh failed for {}", library);
            }
        }

        let sdk_root_output = Command::new("xcrun")
            .arg("--sdk")
            .arg(sdk_name)
            .arg("--show-sdk-path")
            .output()
            .await?;
        if !sdk_root_output.status.success() {
            anyhow::bail!("xcrun --show-sdk-path failed");
        }
        let sdk_root = String::from_utf8(sdk_root_output.stdout)?
            .trim()
            .to_string();

        let cc_output = Command::new("xcrun")
            .arg("--sdk")
            .arg(sdk_name)
            .arg("--find")
            .arg("clang")
            .output()
            .await?;
        if !cc_output.status.success() {
            anyhow::bail!("xcrun --find clang failed");
        }
        let cc = String::from_utf8(cc_output.stdout)?.trim().to_string();

        let mut cflags = format!(
            "-arch {} -isysroot {} {} {}",
            arch_str, sdk_root, min_ver_flag, config.build.cflags
        );
        let mut ldflags = format!(
            "-arch {} -isysroot {} {} {}",
            arch_str, sdk_root, min_ver_flag, config.build.ldflags
        );
        let mut cppflags = String::new();

        if let Some(lib_opts) = config.libraries.get(library) {
            if let Some(c) = &lib_opts.cflags {
                cflags.push_str(&format!(" {}", c));
            }
            if let Some(l) = &lib_opts.ldflags {
                ldflags.push_str(&format!(" {}", l));
            }
        }

        match library {
            Library::Libopusenc => {
                let opus_prefix = config
                    .paths
                    .build_dir
                    .join(platform_name)
                    .join(arch_str)
                    .join("opus");
                cppflags.push_str(&format!(" -I{}", opus_prefix.join("include").display()));
                // 转换为绝对路径，因为 libtool 要求绝对路径
                let opus_lib = fs::canonicalize(opus_prefix.join("lib"))?;
                ldflags.push_str(&format!(" -L{}", opus_lib.display()));
            }
            Library::Libopusfile => {
                let opus_prefix = config
                    .paths
                    .build_dir
                    .join(platform_name)
                    .join(arch_str)
                    .join("opus");
                let ogg_prefix = config
                    .paths
                    .build_dir
                    .join(platform_name)
                    .join(arch_str)
                    .join("ogg");
                cppflags.push_str(&format!(" -I{}", opus_prefix.join("include").display()));
                cppflags.push_str(&format!(" -I{}", ogg_prefix.join("include").display()));
                // 转换为绝对路径，因为 libtool 要求绝对路径
                let opus_lib = fs::canonicalize(opus_prefix.join("lib"))?;
                let ogg_lib = fs::canonicalize(ogg_prefix.join("lib"))?;
                ldflags.push_str(&format!(" -L{}", opus_lib.display()));
                ldflags.push_str(&format!(" -L{}", ogg_lib.display()));
            }
            _ => {}
        }

        let prefix = config
            .paths
            .build_dir
            .join(platform_name)
            .join(arch_str)
            .join(library.repo_name());

        fs::create_dir_all(&prefix)?;

        // 转换为绝对路径，因为 configure 要求 --prefix 必须是绝对路径
        let prefix = fs::canonicalize(&prefix)?;

        // 在 configure 之前执行 make clean，防止不同架构的中间产物混淆
        let _ = Command::new("make")
            .current_dir(&repo.local_path)
            .arg("clean")
            .status()
            .await?;

        let mut configure_cmd = Command::new("./configure");
        configure_cmd
            .current_dir(&repo.local_path)
            .env("CC", &cc)
            .env("CFLAGS", &cflags)
            .env("LDFLAGS", &ldflags)
            .env("CPPFLAGS", &cppflags)
            .arg(format!("--host={}", host))
            .arg(format!("--prefix={}", prefix.display()));

        // 库特定的配置选项（从 config 中读取）
        if let Some(lib_opts) = config.libraries.get(library)
            && let Some(flags) = &lib_opts.configure_flags
        {
            for flag in flags {
                configure_cmd.arg(flag);
            }
        }

        let status = configure_cmd.status().await?;
        if !status.success() {
            anyhow::bail!("configure failed for {}", library);
        }

        let status = Command::new("make")
            .current_dir(&repo.local_path)
            .arg(format!("-j{}", config.build.make_concurrent_jobs))
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("make failed for {}", library);
        }

        let status = Command::new("make")
            .current_dir(&repo.local_path)
            .arg("install")
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("make install failed for {}", library);
        }

        Ok(())
    }
}

impl DarwinBuilder {
    pub async fn build(
        &self,
        platform: Platform,
        arch: Arch,
        library: &Library,
        repo: &Repo,
        config: &Config,
    ) -> Result<()> {
        let (platform_name, sdk_name, min_ver) = match platform {
            Platform::Macos => (
                "macos",
                "macosx",
                format!(
                    "-mmacosx-version-min={}",
                    config.platforms.macos.min_version
                ),
            ),
            Platform::Ios => (
                "ios",
                "iphoneos",
                format!(
                    "-miphoneos-version-min={}",
                    config.platforms.ios.min_version
                ),
            ),
            Platform::IosSim => (
                "ios-sim",
                "iphonesimulator",
                format!(
                    "-mios-simulator-version-min={}",
                    config.platforms.ios_sim.min_version
                ),
            ),
            _ => anyhow::bail!("Platform not supported for Darwin: {:?}", platform),
        };
        let arch_str = match arch {
            Arch::X86_64 => "x86_64",
            Arch::Arm64 => "arm64",
            _ => anyhow::bail!("Architecture not supported for Darwin platform: {:?}", arch),
        };
        let host = match (arch, platform) {
            (Arch::X86_64, Platform::Macos) => "x86_64-apple-darwin",
            (Arch::X86_64, Platform::IosSim) => "x86_64-apple-ios",
            (Arch::Arm64, Platform::Macos) => "arm64-apple-darwin",
            (Arch::Arm64, Platform::Ios) | (Arch::Arm64, Platform::IosSim) => "aarch64-apple-ios",
            _ => anyhow::bail!(
                "{} architecture not supported for platform: {:?}",
                arch_str,
                platform
            ),
        };

        self.build_autotools(
            platform_name,
            arch_str,
            host,
            sdk_name,
            &min_ver,
            library,
            repo,
            config,
        )
        .await
    }
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
        .map(|arch| {
            let arch_str = arch.to_string();
            build_dir
                .join(platform.to_string().to_lowercase())
                .join(arch_str)
                .join(library.repo_name())
                .join("lib")
                .join(&file_name)
        })
        .filter(|p| p.exists())
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

    // 复制头文件到universal目录
    if let Some(first_arch) = archs.first() {
        let include_source = build_dir
            .join(platform.to_string().to_lowercase())
            .join(first_arch.to_string())
            .join(library.repo_name())
            .join("include");

        if include_source.exists() {
            let include_dest = universal_dir.join("include");
            fs::create_dir_all(&include_dest)?;

            log::info!(
                "Copying headers from {} to {}",
                include_source.display(),
                include_dest.display()
            );

            // 复制整个include目录
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
