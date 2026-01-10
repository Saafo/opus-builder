use crate::config::{Arch, Config, LibType, Library, Platform};
use crate::platforms::{android, darwin, harmony};
use crate::repo::Repo;
use crate::utils::CommandVerboseExt;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tokio::process::Command;

pub struct AutotoolsToolchain {
    pub platform_dir: String,
    pub arch_dir: String,
    pub host: String,
    pub cc: String,
    pub cxx: Option<String>,
    pub extra_env: Vec<(String, String)>,
    pub base_cflags: String,
    pub base_ldflags: String,
}

pub struct Builder<'a> {
    platform: Platform,
    arch: Arch,
    library: Library,
    repo: &'a Repo,
    config: &'a Config,
    verbose: bool,
}

impl<'a> Builder<'a> {
    pub fn new(
        platform: Platform,
        arch: Arch,
        library: Library,
        repo: &'a Repo,
        config: &'a Config,
        verbose: bool,
    ) -> Self {
        Self {
            platform,
            arch,
            library,
            repo,
            config,
            verbose,
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

        let toolchain = match self.platform {
            Platform::Android => android::build::prepare_toolchain(self.arch, self.config),
            Platform::Harmony => harmony::build::prepare_toolchain(self.arch, self.config),
            Platform::Macos | Platform::Ios | Platform::IosSim => {
                darwin::build::prepare_toolchain(self.platform, self.arch, self.config).await
            }
        }
        .with_context(|| {
            format!(
                "prepare toolchain failed for {} ({})",
                self.platform, self.arch
            )
        })?;
        let lib_type = self
            .config
            .platforms
            .get_lib_type_for_platform(&self.platform);
        self.run_autotools(&toolchain, lib_type).await?;

        match self.platform {
            Platform::Android => {
                android::build::move_android_package(
                    &self.config.paths.build_dir,
                    &self.library,
                    self.config.get_library_version(&self.library)?,
                    self.arch,
                    lib_type,
                )?;
            }
            Platform::Harmony => {
                harmony::build::move_harmony_package(
                    &self.config.paths.build_dir,
                    &self.library,
                    self.config.get_library_version(&self.library)?,
                    self.arch,
                    lib_type,
                )?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn run_autotools(&self, toolchain: &AutotoolsToolchain, lib_type: LibType) -> Result<()> {
        let prefix = self
            .config
            .paths
            .build_dir
            .join(&toolchain.platform_dir)
            .join(&toolchain.arch_dir)
            .join(self.library.repo_name());

        fs::create_dir_all(&prefix)?;
        let prefix = fs::canonicalize(&prefix)?;

        let mut cflags = toolchain.base_cflags.clone();
        let mut ldflags = toolchain.base_ldflags.clone();
        let mut pkg_config_path = String::new();
        append_library_build_options(self.config, &self.library, &mut cflags, &mut ldflags);
        append_dependency_search_paths(
            &self.config.paths.build_dir,
            &toolchain.platform_dir,
            &toolchain.arch_dir,
            &self.library,
            &mut cflags,
            &mut ldflags,
            &mut pkg_config_path,
        )?;

        run_autogen(
            &self.repo.local_path,
            self.verbose,
            toolchain,
            &cflags,
            &ldflags,
        )
        .await
        .with_context(|| format!("autogen failed for {}", self.library))?;

        try_make_clean(&self.repo.local_path).await;

        let mut configure_cmd = Command::new("./configure");
        configure_cmd
            .current_dir(&self.repo.local_path)
            .arg(format!("--host={}", toolchain.host))
            .arg(format!("--prefix={}", prefix.display()))
            .env("PKG_CONFIG_PATH", &pkg_config_path);

        match lib_type {
            LibType::Static => {
                configure_cmd.arg("--enable-static").arg("--disable-shared");
            }
            LibType::Shared => {
                configure_cmd.arg("--enable-shared").arg("--disable-static");
            }
        }

        append_configure_flags(self.config, &self.library, &mut configure_cmd);
        apply_common_env(&mut configure_cmd, toolchain, &cflags, &ldflags);

        configure_cmd
            .run_with_verbose(self.verbose)
            .await
            .with_context(|| {
                format!(
                    "configure failed for {} on {}/{}",
                    self.library, toolchain.platform_dir, toolchain.arch_dir
                )
            })?;

        let mut make_cmd = Command::new("make");
        make_cmd
            .current_dir(&self.repo.local_path)
            .arg(format!("-j{}", self.config.build.make_concurrent_jobs));
        apply_common_env(&mut make_cmd, toolchain, &cflags, &ldflags);
        make_cmd
            .run_with_verbose(self.verbose)
            .await
            .with_context(|| {
                format!(
                    "make failed for {} on {}/{}",
                    self.library, toolchain.platform_dir, toolchain.arch_dir
                )
            })?;

        let mut install_cmd = Command::new("make");
        install_cmd
            .current_dir(&self.repo.local_path)
            .arg("install");
        apply_common_env(&mut install_cmd, toolchain, &cflags, &ldflags);
        install_cmd
            .run_with_verbose(self.verbose)
            .await
            .with_context(|| {
                format!(
                    "make install failed for {} on {}/{}",
                    self.library, toolchain.platform_dir, toolchain.arch_dir
                )
            })?;

        try_make_clean(&self.repo.local_path).await;
        Ok(())
    }
}

fn append_library_build_options(
    config: &Config,
    library: &Library,
    cflags: &mut String,
    ldflags: &mut String,
) {
    if let Some(lib_opts) = config.libraries.get(library) {
        if let Some(c) = &lib_opts.cflags
            && !c.is_empty()
        {
            cflags.push(' ');
            cflags.push_str(c);
        }
        if let Some(l) = &lib_opts.ldflags
            && !l.is_empty()
        {
            ldflags.push(' ');
            ldflags.push_str(l);
        }
    }
}

fn append_dependency_search_paths(
    build_dir: &Path,
    platform_dir: &str,
    arch_dir: &str,
    library: &Library,
    cflags: &mut String,
    ldflags: &mut String,
    pkg_config_path: &mut String,
) -> Result<()> {
    let deps: &[Library] = match library {
        Library::Libopusenc => &[Library::Libopus],
        Library::Libopusfile => &[Library::Libopus, Library::Libogg],
        _ => &[],
    };
    if deps.is_empty() {
        return Ok(());
    }

    let mut pkg_config_paths = Vec::new();
    for dep in deps {
        let dep_prefix = build_dir
            .join(platform_dir)
            .join(arch_dir)
            .join(dep.repo_name());

        let include_dir = dep_prefix.join("include");
        cflags.push_str(&format!(" -I{}", include_dir.display()));

        let lib_dir = fs::canonicalize(dep_prefix.join("lib")).with_context(|| {
            format!(
                "Dependency lib dir not found: {}",
                dep_prefix.join("lib").display()
            )
        })?;
        ldflags.push_str(&format!(" -L{}", lib_dir.display()));
        pkg_config_paths.push(lib_dir.join("pkgconfig"));
    }
    *pkg_config_path = pkg_config_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(":");

    Ok(())
}

fn append_configure_flags(config: &Config, library: &Library, cmd: &mut Command) {
    for flag in &config.build.configure_flags {
        cmd.arg(flag);
    }
    if let Some(lib_opts) = config.libraries.get(library)
        && let Some(flags) = &lib_opts.configure_flags
    {
        for flag in flags {
            cmd.arg(flag);
        }
    }
}

fn apply_common_env(
    cmd: &mut Command,
    toolchain: &AutotoolsToolchain,
    cflags: &str,
    ldflags: &str,
) {
    cmd.env("CC", &toolchain.cc)
        .env("CFLAGS", cflags)
        .env("LDFLAGS", ldflags);

    if let Some(cxx) = &toolchain.cxx {
        cmd.env("CXX", cxx).env("CXXFLAGS", cflags);
    }

    for (k, v) in &toolchain.extra_env {
        cmd.env(k, v);
    }
}

async fn run_autogen(
    repo_path: &Path,
    verbose: bool,
    toolchain: &AutotoolsToolchain,
    cflags: &str,
    ldflags: &str,
) -> Result<()> {
    let mut cmd = Command::new("sh");
    cmd.arg("./autogen.sh").current_dir(repo_path);
    apply_common_env(&mut cmd, toolchain, cflags, ldflags);
    cmd.run_with_verbose(verbose).await
}

async fn try_make_clean(repo_path: &Path) {
    let _ = Command::new("make")
        .current_dir(repo_path)
        .arg("clean")
        .output()
        .await;
}
