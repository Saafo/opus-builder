use anyhow::Result;
use tokio::process::Command;

/// Extension methods for `tokio::process::Command` to support a verbose mode.
pub trait CommandVerboseExt {
    /// Executes the command and controls output based on `verbose`.
    ///
    /// - `verbose = true`: stream output directly
    /// - `verbose = false`: capture output and only print it on failure
    async fn run_with_verbose(&mut self, verbose: bool) -> Result<()>;
}

impl CommandVerboseExt for Command {
    async fn run_with_verbose(&mut self, verbose: bool) -> Result<()> {
        let std_cmd = self.as_std();
        let program = std_cmd.get_program().to_string_lossy().to_string();
        let args: Vec<String> = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        log::info!("Executing Command: {} {}", program, args.join(" "));

        if verbose {
            let status = self.status().await?;
            if !status.success() {
                anyhow::bail!("Command failed with exit code: {:?}", status.code());
            }
        } else {
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
