use anyhow::Result;
use tokio::process::Command;

/// 为 tokio::process::Command 添加扩展方法，支持 verbose 模式控制输出
pub trait CommandVerboseExt {
    /// 执行命令，根据 verbose 参数控制输出
    ///
    /// - verbose=true: 实时显示命令输出
    /// - verbose=false: 默认不显示输出，仅在命令失败时显示
    async fn run_with_verbose(&mut self, verbose: bool) -> Result<()>;
}

impl CommandVerboseExt for Command {
    async fn run_with_verbose(&mut self, verbose: bool) -> Result<()> {
        // 通过 as_std() 获取 program 和 args 用于日志
        let std_cmd = self.as_std();
        let program = std_cmd.get_program().to_string_lossy().to_string();
        let args: Vec<String> = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        log::info!("Executing Command: {} {}", program, args.join(" "));

        if verbose {
            // verbose 模式：实时显示输出
            let status = self.status().await?;
            if !status.success() {
                anyhow::bail!("Command failed with exit code: {:?}", status.code());
            }
        } else {
            // 默认模式：捕获输出，失败时才显示
            let output = self.output().await?;
            if !output.status.success() {
                if !output.stdout.is_empty() {
                    eprintln!("\nSTDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
                }
                if !output.stderr.is_empty() {
                    eprintln!("\nSTDERR:\n{}", String::from_utf8_lossy(&output.stderr));
                }
                eprintln!("\nCommand failed: {} {}", program, args.join(" "));
                eprintln!("Exit code: {:?}\n", output.status.code());

                anyhow::bail!("Command failed with exit code: {:?}", output.status.code());
            }
        }
        Ok(())
    }
}
