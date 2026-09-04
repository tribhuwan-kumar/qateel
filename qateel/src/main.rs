use rand::Rng;
use cli::Args;
use clap::Parser;
use std::sync::Arc;
use config::AppConf;
use tokio::sync::watch;
use std::time::Duration;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing::{info, error, warn};
use axum_server::tls_rustls::RustlsConfig;

mod db;
mod cli;
mod web;
mod api;
mod app;
mod his;
mod key;
mod auth;
mod logs;
mod cron;
mod aria2;
mod utils;
mod addrs;
mod config;
mod status;

use crate::{
    db::init_db,
    app::AppState,
    status::SysStatus,
    aria2::types::Aria2Client,
	aria2::daemon::Aria2Daemon,
    auth::types::AuthController,
    his::service::HistoryService,
};

const CHANNEL: usize = 256;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = Args::parse();
	let config = if let Some(path) = args.config_path.as_ref()
		.filter(|p| p.exists()) {
		Some(AppConf::load_config(path)?)
	} else {
		None
	};

    let conf = AppConf::args_to_config(args.clone(), config);
    let _log_guard = logs::init_logging(&conf)?;


    utils::intro();
	utils::ensure_aria2_installation();

    let (aria2_resp_tx, _aria2_resp_rx) = broadcast::channel(CHANNEL);
    let (history_tx_rw, _history_rx_rw) = broadcast::channel(CHANNEL);
    let (global_tx_rw, _global_rx_rw) = broadcast::channel(CHANNEL);


    if !conf.data_dir.exists() {
        info!("Creating data directory at {:?}", args.data_dir);
        std::fs::create_dir_all(&conf.data_dir)?;
    }

    let aria2_url = format!("{}:{}/jsonrpc", conf.aria2_host, conf.aria2_port);

    /* Generate random 32-char secret */
    let jwt_secret = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();


    let db_pool = init_db(&conf.data_dir).await?;
	let aria2_daemon = Aria2Daemon::new();

    let admin_exists = AuthController::admin_exists(&db_pool).await?;

    let initial_sys_status = SysStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        admin_exists,
        aria2_alive: false,
    };

    let (status_tx, _status_rx) = watch::channel(initial_sys_status);
    let status_tx = Arc::new(status_tx);

    let aria2_client = Aria2Client::new(
        aria2_url.clone(),
        conf.aria2_secret.clone(),
        status_tx.clone(),
        aria2_resp_tx.clone()
    );

    let state = AppState {
        db: db_pool.clone(),
		config: conf.clone(),
		status_tx: status_tx,
        jwt_secret: jwt_secret,
		daemon: Arc::new(aria2_daemon),
        aria2: Arc::new(aria2_client.clone()),
        history_tx: Arc::new(history_tx_rw),
        global_tx: Arc::new(global_tx_rw),
    };

    info!("Starting history service");
    HistoryService::init(state.clone(), aria2_resp_tx.subscribe()).await;


    if !admin_exists {
        warn!("Initializing database: No users found");
        info!("Open the web ui and register for admin");
    } else {
        info!("Database is initialized");
    }

    // Ignore ipv6
    let (ipv4_addrs, _ipv6_addrs) = addrs::interface_addrs()?;
    let all_addrs = [ipv4_addrs].concat();
    let addrs_listens = addrs::print_listening(&conf, &all_addrs)?;

    info!("{}", addrs_listens);
    info!("QateeL version: '{}'", env!("CARGO_PKG_VERSION"));
    info!("Data directory: '{}'", &conf.data_dir.to_string_lossy());

    if !conf.aria2_secret.is_empty() {
        info!("Aria2 secret token loaded");
    } else {
        error!("No Aria2 secret token provided, please define it in CLI args or in Config file");
    }

    let client_clone = aria2_client.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        match client_clone.call("aria2.getVersion", vec![]).await {
            Ok(v) => info!("Aria2 version: {}", v),
            Err(e) => warn!("Could not get Aria2 version: {}", e),
        }
    });

    let app = api::routes(state, conf.cors_origins);
    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    let addr: SocketAddr = format!("{}:{}", &conf.host, &conf.port).parse()?;

    let server = async move {
        match (&conf.ssl_cert, &conf.ssl_key) {
            (Some(cert), Some(key))
				if !cert.to_string_lossy().is_empty() && !key.to_string_lossy().is_empty() => {
					info!("HTTPS enabled with TLS cert/key");
					let config = RustlsConfig::from_pem_file(cert, key).await?;
					axum_server::bind_rustls(addr, config)
						.handle(handle)
						.serve(app.into_make_service())
						.await
				}
            _ => {
                warn!("TLS not configured, serving HTTP only");
                axum_server::bind(addr)
                    .handle(handle)
                    .serve(app.into_make_service())
                    .await
            }
        }
    };

    /*
     * `tokio::pin!`
     * so the same `server` future can be polled multiple times safely
    */
    tokio::pin!(server);

    tokio::select! {
        res = &mut server => {
			if let Err(e) = res {
                let err_msg = e.to_string();
                if err_msg.contains("Address already in use") || err_msg.contains("os error 98") {
                    error!("Address '{}' is already in use", addr);
                    std::process::exit(1);
                } else {
                    error!("Server error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C received, shutting down...");
            shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));

            if let Err(e) = (&mut server).await {
                error!("Server error during shutdown: {}", e);
            }
        }
    }

    Ok(())
}

