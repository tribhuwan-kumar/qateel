use clap::Parser;
use std::path::PathBuf;

use crate::utils::{default_data_dir, default_aria2_dir};

// Inlcude random letters as short so they don't look misaligned :(
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Port to serve the Web UI and API
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// Host to bind the server to
    #[arg(short='a', long, default_value = "0.0.0.0")]
    pub host: String,

    /// Aria2 RPC Host (Websocket preferred, use `wss` for TLS)
    #[arg(short='x', long, default_value = "ws://127.0.0.1")]
    pub aria2_host: String,

    /// Aria2 RPC Port
    #[arg(short='y', long, default_value = "6800")]
    pub aria2_port: String,

    /// Aria2 RPC Secret Token
    #[arg(short='s', long, env = "ARIA2_SECRET")]
    pub aria2_secret: Option<String>,

    /// Download directory for Aria2
    #[arg(short='o', long, default_value_os_t = default_aria2_dir())]
    pub aria2_dir: PathBuf,

    /// Allowed CORS origins as comma separated
    #[arg(short='t', long)]
    pub cors_origins: Option<String>,

    /// Qateel TOML config path
    #[arg(short='c', long)]
    pub config_path: Option<PathBuf>,

    /// Directory to store application data
    #[arg(short='d', long, default_value_os_t = default_data_dir())]
    pub data_dir: PathBuf,

    /// Enable verbose logging
    #[arg(short='l', long)]
    pub verbose: bool,

    /// Path to TLS certificate (PEM) to serve with HTTPS
    #[arg(short='j', long, env = "QATEEL_SSL_CERT")]
    pub ssl_cert: Option<PathBuf>,

    /// Path to TLS private key (PEM)
    #[arg(short='k', long, env = "QATEEL_SSL_KEY")]
    pub ssl_key: Option<PathBuf>,
}

