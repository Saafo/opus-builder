use crate::config::{Arch, Config, LibType, Library};
use crate::repo::Repo;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use tokio::process::Command;

// 构建环境变量结构体
struct BuildEnv<'a> {
    cc: &'a str,
    ar: &'a Path,
    as_tool: &'a str,
    ld: &'a Path,
    nm: &'a Path,
    ranlib: &'a Path,
    strip: &'a Path,
    cflags: &'a str,
    ldflags: &'a str,
}

// 为 Command 添加 set_build_env 扩展方法
trait CommandExt {
    fn set_build_env(&mut self, env: &BuildEnv) -> &mut Self;
}

impl CommandExt for Command {
    fn set_build_env(&mut self, env: &BuildEnv) -> &mut Self {
        self.env("CC", env.cc)
            .env("AR", env.ar)
            .env("AS", env.as_tool)
            .env("LD", env.ld)
            .env("NM", env.nm)
            .env("RANLIB", env.ranlib)
            .env("STRIP", env.strip)
            .env("CFLAGS", env.cflags)
            .env("LDFLAGS", env.ldflags)
    }
}

pub struct AndroidBuilder;

impl AndroidBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn get_android_abi(arch: &Arch) -> &str {
        match arch {
            Arch::ArmeabiV7a => "armeabi-v7a",
            Arch::Arm64V8a => "arm64-v8a",
            Arch::X86 => "x86",
            Arch::X86_64 => "x86_64",
            _ => panic!("Unsupported architecture for Android: {:?}", arch),
        }
    }

    fn get_android_host(arch: &Arch) -> &str {
        match arch {
            Arch::ArmeabiV7a => "arm-linux-androideabi",
            Arch::Arm64V8a => "aarch64-linux-android",
            Arch::X86 => "i686-linux-android",
            Arch::X86_64 => "x86_64-linux-android",
            _ => panic!("Unsupported architecture for Android: {:?}", arch),
        }
    }

    fn get_host_platform() -> &'static str {
        if cfg!(target_os = "macos") {
            "darwin-x86_64"
        } else if cfg!(target_os = "linux") {
            "linux-x86_64"
        } else {
            panic!("Unsupported host OS for Android NDK: {}", env::consts::OS);
        }
    }

    async fn build_autotools(
        &self,
        arch: &Arch,
        library: &Library,
        repo: &Repo,
        config: &Config,
    ) -> Result<()> {
        let android_config = &config.platforms.android;

        let abi = Self::get_android_abi(arch);
        let host = Self::get_android_host(arch);
        let host_platform = Self::get_host_platform();

        // 设置工具链路径
        let toolchain_bin = android_config
            .ndk_path
            .join("toolchains/llvm/prebuilt")
            .join(host_platform)
            .join("bin");

        // 使用 llvm 工具
        let ar = toolchain_bin.join("llvm-ar");
        let ranlib = toolchain_bin.join("llvm-ranlib");
        let strip = toolchain_bin.join("llvm-strip");
        let nm = toolchain_bin.join("llvm-nm");

        // 使用 clang 作为编译器，添加 --target 参数
        let api_level = android_config.native_api_level;
        let cc_target = format!("{}{}", host, api_level);
        let cc = format!(
            "{} --target={}",
            toolchain_bin.join("clang").display(),
            cc_target
        );
        let cxx = format!(
            "{} --target={}",
            toolchain_bin.join("clang++").display(),
            cc_target
        );

        // 使用 ld 而不是 ld.lld
        let ld = toolchain_bin.join("ld");

        // CFLAGS 简化为 -Oz
        let mut cflags = format!("-Oz {}", config.build.cflags);

        // LDFLAGS 包含依赖库路径（暂时为空，后续添加）
        let mut ldflags = String::new();
        if !config.build.ldflags.is_empty() {
            ldflags.push_str(&config.build.ldflags);
        }

        // 添加库特定的编译标志
        if let Some(lib_opts) = config.libraries.get(library) {
            if let Some(c) = &lib_opts.cflags {
                cflags.push_str(&format!(" {}", c));
            }
            if let Some(l) = &lib_opts.ldflags {
                ldflags.push_str(&format!(" {}", l));
            }
        }

        // 创建构建环境变量（可复用）
        let env = BuildEnv {
            cc: &cc,
            ar: &ar,
            as_tool: &cc,
            ld: &ld,
            nm: &nm,
            ranlib: &ranlib,
            strip: &strip,
            cflags: &cflags,
            ldflags: &ldflags,
        };

        // 运行 autogen.sh（如果存在）
        let autogen_path = repo.local_path.join("autogen.sh");
        if autogen_path.exists() {
            let status = Command::new("sh")
                .arg("./autogen.sh")
                .current_dir(&repo.local_path)
                .set_build_env(&env)
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("autogen.sh failed for {}", library);
            }
        }

        let prefix = config
            .paths
            .build_dir
            .join("android")
            .join(abi)
            .join(library.repo_name());

        fs::create_dir_all(&prefix)?;

        // 转换为绝对路径，因为 configure 要求 --prefix 必须是绝对路径
        let prefix = fs::canonicalize(&prefix)?;

        // 在 configure 之前执行 make clean，防止不同架构之间复用错误的中间产物
        let _ = Command::new("make")
            .current_dir(&repo.local_path)
            .arg("clean")
            .status()
            .await?;

        let mut configure_cmd = Command::new("./configure");
        configure_cmd
            .current_dir(&repo.local_path)
            .arg(format!("--host={}", host))
            .arg(format!("--prefix={}", prefix.display()));

        // 根据 lib_type 配置决定构建静态库还是动态库
        match android_config.lib_type {
            LibType::Static => {
                configure_cmd.arg("--enable-static").arg("--disable-shared");
            }
            LibType::Shared => {
                configure_cmd.arg("--disable-static").arg("--enable-shared");
            }
        }

        // 库特定的配置选项（从 config 中读取）
        if let Some(lib_opts) = config.libraries.get(library)
            && let Some(flags) = &lib_opts.configure_flags
        {
            for flag in flags {
                configure_cmd.arg(flag);
            }
        }

        let status = configure_cmd
            .set_build_env(&env)
            .env("CXX", &cxx)
            .env("CXXFLAGS", &cflags)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("configure failed for {} on {}", library, abi);
        }

        let status = Command::new("make")
            .current_dir(&repo.local_path)
            .arg(format!("-j{}", config.build.make_concurrent_jobs))
            .set_build_env(&env)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("make failed for {} on {}", library, abi);
        }

        let status = Command::new("make")
            .current_dir(&repo.local_path)
            .arg("install")
            .set_build_env(&env)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("make install failed for {} on {}", library, abi);
        }

        // 构建完成后立即移动库文件到 build/lib
        let version = if let Some(lib_config) = config.libraries.get(library) {
            if let Some(v) = &lib_config.version {
                v
            } else {
                anyhow::bail!("Version not specified for library: {:?}", library);
            }
        } else {
            anyhow::bail!("Library configuration not found for: {:?}", library);
        };
        move_android_package(
            &config.paths.build_dir,
            library,
            version,
            arch,
            android_config.lib_type,
        )?;

        Ok(())
    }
}

/// 移动单个架构的 Android 库文件到 build/lib
fn move_android_package(
    build_dir: &Path,
    library: &Library,
    version: &str,
    arch: &Arch,
    lib_type: LibType,
) -> Result<()> {
    let lib_name = library.lib_name();
    let repo_name = library.repo_name();
    let version = version.trim_start_matches('v');

    // 确定库文件扩展名
    let lib_ext = match lib_type {
        LibType::Static => "a",
        LibType::Shared => "so",
    };

    let abi = AndroidBuilder::get_android_abi(arch);

    // 源文件路径
    let source_lib = build_dir
        .join("android")
        .join(abi)
        .join(repo_name)
        .join("lib")
        .join(format!("lib{}.{}", lib_name, lib_ext));

    // 目标目录
    let dest_dir = build_dir
        .join("lib")
        .join("android")
        .join(abi)
        .join(format!("{}-{}", lib_name, version));

    fs::create_dir_all(&dest_dir)?;

    // 目标文件路径
    let dest_lib = dest_dir.join(format!("lib{}.{}", lib_name, lib_ext));

    if source_lib.exists() {
        log::info!(
            "Moving {} from {} to {}",
            lib_name,
            source_lib.display(),
            dest_lib.display()
        );
        fs::copy(&source_lib, &dest_lib)?;
    } else {
        log::warn!("Library file not found: {}, skipping", source_lib.display());
    }

    Ok(())
}

impl AndroidBuilder {
    pub async fn build(
        &self,
        arch: Arch,
        library: &Library,
        repo: &Repo,
        config: &Config,
    ) -> Result<()> {
        self.build_autotools(&arch, library, repo, config).await
    }
}
