use rand::Rng;
use std::fmt::Display;
use tracing::{info, error};
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

/// Print intro
pub fn intro() {
    let intro = r#"
        ....                             s                                ..
    .n~8888888nx                        :8                          x .d88"
  :88>'8888888888:                     .88                           5888R
 :8888 "*888888888k           u       :888ooo      .u         .u     '888R
 '88888.         "8>       us888u.  -*8888888   ud8888.    ud8888.    888R
  ?88888          'X    .@88 "8888"   8888    :888'8888. :888'8888.   888R
?  %888!           !    9888  9888    8888    d888 '88%" d888 '88%"   888R
 ".:88"            !    9888  9888    8888    8888.+"    8888.+"      888R
   xHH8Hx.        .X  : 9888  9888   .8888Lu= 8888L      8888L        888R
 :888888888hx....x\8..X 9888  9888   ^%888*   '8888c. .+ '8888c. .+  .888B .
:~  `"8888888888!`'8888 "888*""888"    'Y"     "88888%    "88888%    ^*888%
       `""*8*""`   "*"   ^Y"   ^Y'               "YP'       "YP'       "%
							𝖆𝖗𝖎𝖆2 𝖜𝖊𝖇 𝖚𝖎, 𝖇𝖚𝖙 𝖎𝖙'𝖘 𝖇𝖊𝖙𝖙𝖊𝖗!!
                                                "#;
    println!("{:}", intro);
}

/// Generates a valid 16-character hex string for Aria2 GIDs
pub fn generate_gid() -> String {
    let mut rng = rand::rng();
    loop {
        let bytes: [u8; 8] = rng.random();
        if bytes.iter().all(|&b| b == 0) {
            continue;
        }
        return bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
    }
}

/// Get default data dir
pub fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".qateel"))
        .join("qateel")
}

/// Torrent stored torrents file path
pub fn get_torrent_path<T: Display>(data_dir: &Path, user_id: T, info_hash: &str) -> PathBuf {
    let file_name = format!("{}-{}.torrent", user_id, info_hash);
    data_dir.join("torrents").join(file_name)
}

/// Default aria2 downlaod dir
pub fn default_aria2_dir() -> PathBuf {
    let base_dir = match dirs::download_dir() {
        Some(path) => path,
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    base_dir.join("aria2")
}

/// Check aria2 installation
pub fn ensure_aria2_installation() {
    let result = Command::new("aria2c")
        .arg("--version")
        .output();

    match result {
        Ok(output) if output.status.success() => {
			info!("Aria2 is installed!!")
		}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.trim().is_empty() {
                error!("Aria2 is installed but failed to run");
            } else {
                error!("Aria2 is installed but failed to run: {}", stderr.trim());
            }
            exit(1);
        }
        _ => {
            error!("Aria2 is not installed or not available in $PATH. Please install it before running Qateel!!");
            exit(1);
        }
    }
}

/// Graceful stuff
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to install SIGTERM handler");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl+C received");
            }
            _ = sigterm.recv() => {
                info!("SIGTERM received");
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        info!("Ctrl+C received");
    }
}

