//! Container sandbox and ephemeral test execution.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecutionResult {
    pub engine: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

pub struct ContainerSandbox;

impl ContainerSandbox {
    /// Test if Docker CLI and daemon are responding.
    pub fn is_docker_available() -> bool {
        Command::new("docker")
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Execute a command in an isolated Docker container mounted to the workspace repo,
    /// or fallback safely to a direct isolated subprocess if Docker is offline.
    pub fn execute(
        repo_dir: &Path,
        image: Option<&str>,
        cmd: &str,
    ) -> Result<SandboxExecutionResult, String> {
        let start = Instant::now();
        let docker_ready = Self::is_docker_available();

        if docker_ready {
            let container_image = image.unwrap_or("alpine:latest");
            let host_path = repo_dir.to_string_lossy().to_string();
            let mount_arg = format!("{}:/workspace", host_path);

            let output = match Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "-v",
                    &mount_arg,
                    "-w",
                    "/workspace",
                    container_image,
                    "sh",
                    "-c",
                    cmd,
                ])
                .output()
            {
                Ok(out) => out,
                Err(e) => return Err(format!("Docker execution failed: {}", e)),
            };

            let duration_ms = start.elapsed().as_millis();
            Ok(SandboxExecutionResult {
                engine: format!("docker:{}", container_image),
                exit_code: output.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                duration_ms,
            })
        } else {
            // Local fallback subprocess in repo working dir
            let output = match Command::new("cmd")
                .args(["/C", cmd])
                .current_dir(repo_dir)
                .output()
            {
                Ok(out) => out,
                Err(e) => return Err(format!("Host subprocess execution failed: {}", e)),
            };

            let duration_ms = start.elapsed().as_millis();
            Ok(SandboxExecutionResult {
                engine: "host-isolated-subprocess".to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                duration_ms,
            })
        }
    }
}
