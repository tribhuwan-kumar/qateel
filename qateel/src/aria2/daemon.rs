use std::sync::Arc;
use std::path::Path;
use tokio::sync::Mutex;
use tokio::time::sleep;
use std::process::Stdio;
use std::time::Duration;
use std::collections::HashMap;
use tokio::process::{Child, Command};
use tracing::{info, debug, warn, error};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::config::AppConf;

#[derive(Clone, Debug)]
pub struct Aria2Daemon {
	child_process: Arc<Mutex<Option<Child>>>,
}

pub const RESTRICTED_ARIA2_KEYS: &[&str] = &[
    "dir",
    "daemon",
    "rpc-user",
    "rpc-passwd",
    "rpc-secret",
    "enable-rpc",
    "rpc-listen-port",
    "rpc-save-upload-metadata",
];

// TODO: Implement self healing aria2 daemon
impl Aria2Daemon {
    pub fn new() -> Self {
        Self {
			child_process: Arc::new(Mutex::new(None)),
        }
    }

    /// Starts the daemon combining immutable locked configs and arguments
    pub async fn start(&self, config: &AppConf, args: &HashMap<String, String>) -> Result<(), String> {
		let mut lock = self.child_process.lock().await;

		if lock.is_some() {
			return Err("Aria2 daemon is already running.".into());
		}

		let mut cmd = Command::new("aria2c");
		debug!("Aria2 download dir: {:?}", config.aria2_dir.display());

        // No `--daemon=true` here because spawn `Aria2` as `Qateel` child process
        // To hold the stdout/stderr pipes and manage the lifecycle directly.
        cmd.arg("--daemon=false")
			.arg("--enable-rpc=true")
			.arg(format!("--dir={}", config.aria2_dir.display()))
			.arg(format!("--rpc-listen-port={}", config.aria2_port))
			.arg(format!("--rpc-save-upload-metadata=false"))
			.stdout(Stdio::piped())
			.stderr(Stdio::piped());

        if !config.aria2_secret.is_empty() {
            cmd.arg(format!("--rpc-secret={}", config.aria2_secret));
        }

		if !config.aria2_rpc_user.is_empty() && !config.aria2_rpc_passwd.is_empty()  {
			cmd.arg(format!("--rpc-user={}", config.aria2_rpc_user));
			cmd.arg(format!("--rpc-passwd={}", config.aria2_rpc_passwd));
		}

        debug!("Aria2 arguments: {:?}", args);
        for (key, value) in args {
			// Restricted keys that is not allowed to override via api
            if RESTRICTED_ARIA2_KEYS.contains(&key.as_str()) {
                continue;
            }

            // Format as --key=value (since aria2 expects everything as strings)
            if value.trim().is_empty() {
                continue;
            }

            if key == "conf-path" || key == "input-file" {
                let path = Path::new(value.trim());
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create configuration directory: {}", e))?;
                    }
                }
                if !path.exists() {
                    info!("Creating missing empty aria2 configuration file: {:?}", path);
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(path)
                        .map_err(|e| format!("Failed to initialize empty aria2 configuration file: {}", e))?;
                }
            }

            cmd.arg(format!("--{}={}", key, value));
        }

        info!("Spawning aria2c child worker thread...");
		let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn aria2c executor: {}", e))?;

        sleep(Duration::from_millis(500)).await;

		let stdout = child.stdout.take();
        let stderr = child.stderr.take();

		if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
			return Err(format!("Aria2 exited immediately with status: {}, stderr: {:?}", status, stderr));
		}

		if let Some(out) = stdout {
			tokio::spawn(async move {
				let mut reader = BufReader::new(out).lines();
				while let Ok(Some(line)) = reader.next_line().await {
					debug!("[Aria2c stdout] {}", line);
				}
			});
		}

		if let Some(err) = stderr {
			tokio::spawn(async move {
				let mut reader = BufReader::new(err).lines();
				while let Ok(Some(line)) = reader.next_line().await {
					warn!("[Aria2c stderr] {}", line);
				}
			});
		}

		*lock = Some(child);
        Ok(())
    }

    /// Sends a kill sequence to the active process handle and reclaims resources
	pub async fn stop(&self) -> Result<(), String> {
		let mut lock = self.child_process.lock().await;

		if let Some(mut child) = lock.take() {
			info!("Stopping aria2c process...");
			if let Err(e) = child.kill().await {
				let err = format!("Failed to kill aria2 process: {}", e);
				error!("{}", err);
				return Err(err);
			}
			// Await the os resource cleanup
			let _ = child.wait().await;
			Ok(())
		} else {
			Ok(())
		}
	}

    /// Restarts the daemon
	pub async fn restart(&self, config: &AppConf, args: &HashMap<String, String>) -> Result<(), String> {
		let _ = self.stop().await;
		self.start(config, args).await
	}
}
