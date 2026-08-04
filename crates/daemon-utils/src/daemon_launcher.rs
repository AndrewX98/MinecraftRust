#![allow(async_fn_in_trait)]
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

pub trait DaemonLauncher: Send {
    fn service_path(&self) -> &str;
    fn get_arguments(&self) -> Vec<String>;
    fn get_cwd(&self) -> String {
        "/".to_string()
    }

    async fn start(&self) -> Result<u32, String> {
        let args = self.get_arguments();
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        cmd.current_dir(self.get_cwd());
        // Detach from the launcher's stdio so the daemon (which may outlive the
        // launcher) doesn't keep our stdout/stderr pipes open.
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {}", e))?;
        let pid = child.id().ok_or("no pid")?;

        // Wait for socket to appear (up to 10 seconds)
        let path2 = self.service_path().to_string();
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if Path::new(&path2).exists() {
                return Ok(pid);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("Timeout waiting for daemon socket".into())
    }
}
