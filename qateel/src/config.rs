use std::fs;
use serde::Deserialize;
use schemars::JsonSchema;
use std::path::{Path, PathBuf};

use crate::utils::{default_data_dir, default_aria2_dir};

use crate::cli::Args;

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "Top-level configuration for the application. All fields are optional and may be overridden by CLI arguments or built-in defaults.")]
pub struct Config {
    #[schemars(description = "Qateel server related settings. Missing values fall back to the built-in defaults: port = 8080, host = \"0.0.0.0\", verbose = false.")]
    pub server: Option<ServerConfig>,
    #[schemars(description = "Aria2 specific settings. Missing values fall back to the built-in defaults: host = \"ws://127.0.0.1\", port = \"6800\".")]
    pub aria2: Option<Aria2Config>,
    #[schemars(description = "TLS configuration. Both certificate and key are optional; if omitted, TLS is disabled unless provided through CLI arguments.")]
    pub tls: Option<TlsConfig>,
    #[schemars(description = "Qateel application settings. Missing values fall back to the built-in defaults.")]
    pub app: Option<AppConfig>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "Qateel server configuration.")]
pub struct ServerConfig {
    #[schemars(description = "Port to bind to. Default: 8080.")]
    pub port: Option<u16>,
    #[schemars(description = "Host address to bind to. Default: \"0.0.0.0\".")]
    pub host: Option<String>,
	#[schemars(description = "Enable verbose logging. Default: false.")]
    pub verbose: Option<bool>,
    #[schemars(description = "Allowed CORS origins. Example: [\"http://localhost:5173\", \"http://127.0.0.1:5173\"].")]
    pub cors_origins: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "Aria2 configuration.")]
pub struct Aria2Config {
	#[schemars(description = "Aria2 host address. Default: \"ws://127.0.0.1\".")]
    pub host: Option<String>,
	#[schemars(description = "Aria2 JSON-RPC port. Default: \"6800\".")]
    pub port: Option<String>,
	#[schemars(description = "Aria2 JSON-RPC secret token. Optional; no default value. You must have to set a Aria2 secret value.")]
    pub secret: Option<String>,
	#[schemars(description = "Aria2 directory to store downloaded content.")]
    pub dir: Option<PathBuf>,
	#[schemars(description = "Aria2 JSON-RPC user name. Optional; no default value.")]
    pub rpc_user: Option<String>,
	#[schemars(description = "Aria2 JSON-RPC user password. Optional; no default value.")]
    pub rpc_passwd: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "TLS configuration for HTTPS-related settings.")]
pub struct TlsConfig {
	#[schemars(description = "Path to the PEM-encoded certificate file. Optional; no default value.")]
    pub cert: Option<PathBuf>,
	#[schemars(description = "Path to the PEM-encoded private key file. Optional; no default value.")]
    pub key: Option<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "Qateel application configuration.")]
pub struct AppConfig {
	#[schemars(description = "Directory used to store application data. Default: the value returned by default_data_dir().")]
    pub data_dir: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct AppConf {
    pub port: u16,
    pub host: String,
    pub aria2_host: String,
    pub aria2_port: String,
    pub aria2_secret: String,
	pub aria2_dir: PathBuf,
	pub aria2_rpc_user: String,
	pub aria2_rpc_passwd: String,
    pub cors_origins: Vec<String>,
    pub data_dir: PathBuf,
    pub verbose: bool,
    pub ssl_cert: Option<PathBuf>,
    pub ssl_key: Option<PathBuf>,
}

impl AppConf {
	fn merge_with_default<T: PartialEq>(
			cli: T,
			default: T,
			config: Option<T>
		) -> T {
		if cli != default {
			cli
		} else {
			config.unwrap_or(default)
		}
	}

    fn parse_cors_origins(value: Option<String>) -> Vec<String> {
        value
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|origin| !origin.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn load_config(path: &Path) -> anyhow::Result<Config> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn args_to_config(args: Args, config: Option<Config>) -> Self {
        let server = config.as_ref().and_then(|c| c.server.as_ref());
        let aria2 = config.as_ref().and_then(|c| c.aria2.as_ref());
        let tls = config.as_ref().and_then(|c| c.tls.as_ref());
        let app = config.as_ref().and_then(|c| c.app.as_ref());

		let cli_cors_origins = Self::parse_cors_origins(args.cors_origins);
        let secret_opt = args
            .aria2_secret
            .clone()
            .or_else(|| aria2.and_then(|a| a.secret.clone()))
            .filter(|s| !s.trim().is_empty());
		let rpc_user_opt = args
			.aria2_rpc_user
			.clone()
			.or_else(|| aria2.as_ref().and_then(|a| a.rpc_user.clone()))
			.filter(|s| !s.trim().is_empty());
		let rpc_passwd_opt = args
			.aria2_rpc_password
			.clone()
			.or_else(|| aria2.as_ref().and_then(|a| a.rpc_passwd.clone()))
			.filter(|s| !s.trim().is_empty());

		match (&secret_opt, &rpc_user_opt, &rpc_passwd_opt) {
            // Case A: Everything is missing
            (None, None, None) => {
                eprintln!(
                    // "\x1b[1;31mError:\x1b[0m Aria2 JSON-RPC auth credentials missing. You must provide either Aria2 secret or Aria2 JSON-RPC user & password."
					"\x1b[1;31mError:\x1b[0m  Aria2 JSON-RPC secret must be provided either via cli arguments or configuration file."
                );
                std::process::exit(1);
            }
            // Case B: Clashing configurations (secret & user,pass details are provided)
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                eprintln!(
                    "\x1b[1;31mError:\x1b[0m Clashing auth credentials. Define either Aria2 secret or Aria2 JSON-RPC user & password, but do not define both authentication methods at the same time."
                );
                std::process::exit(1);
            }
            // Case C: User defined without password
            (None, Some(_), None) => {
                eprintln!(
                    "\x1b[1;31mError:\x1b[0m Aria2 JSON-RPC user is defined, but JSON-RPC password is missing. Both must be provided together."
                );
                std::process::exit(1);
            }
            // Case D: Password defined without User
            (None, None, Some(_)) => {
                eprintln!(
                    "\x1b[1;31mError:\x1b[0m Aria2 JSON-RPC password is defined, but JSON-RPC user is missing. Both must be provided together."
                );
                std::process::exit(1);
            }
            // Case E: Fully valid states (secret only, or complete user,pass only)
            _ => {}
        }

        Self {
            port: Self::merge_with_default(args.port, 8080, server.and_then(|s| s.port)),
            host: Self::merge_with_default(
                args.host,
                "0.0.0.0".to_string(),
                server.and_then(|s| s.host.clone()),
            ),
            aria2_host: Self::merge_with_default(
                args.aria2_host,
                "ws://127.0.0.1".to_string(),
                aria2.and_then(|a| a.host.clone()),
            ),
            aria2_port: Self::merge_with_default(
                args.aria2_port,
                "6800".to_string(),
                aria2.and_then(|a| a.port.clone()),
            ),
            aria2_secret: secret_opt.unwrap_or_default(),
            aria2_rpc_user: rpc_user_opt.unwrap_or_default(),
            aria2_rpc_passwd: rpc_passwd_opt.unwrap_or_default(),
            aria2_dir: Self::merge_with_default(
                args.aria2_dir,
                default_aria2_dir(),
                aria2.and_then(|a| a.dir.clone()),
            ),
            cors_origins: if !cli_cors_origins.is_empty() {
                cli_cors_origins
            } else {
                server
                    .and_then(|s| s.cors_origins.clone())
                    .unwrap_or_default()
            },
            data_dir: Self::merge_with_default(
                args.data_dir,
                default_data_dir(),
                app.and_then(|a| a.data_dir.clone()),
            ),
            verbose: args.verbose || server.and_then(|s| s.verbose).unwrap_or(false),
            ssl_cert: args.ssl_cert.or(tls.and_then(|t| t.cert.clone())),
            ssl_key: args.ssl_key.or(tls.and_then(|t| t.key.clone())),
        }
    }
}

