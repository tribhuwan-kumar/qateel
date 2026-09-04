use std::fs;
use anyhow::Result;
use tracing::error;
use std::fs::OpenOptions;
use tracing_subscriber::fmt::time::LocalTime;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter,};

use crate::config::AppConf;

pub fn init_logging(conf: &AppConf) -> Result<WorkerGuard> {
    let data_dir = &conf.data_dir;

    if !data_dir.exists() {
        fs::create_dir_all(&data_dir)?;
    }

    let log_path = data_dir.join("qateel.log");
    let log_level = if conf.verbose { "debug" } else { "info" };

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(&log_path)?;

    let (file_writer, guard) = tracing_appender::non_blocking(file);

    let timer = LocalTime::rfc_3339();

    let file_layer = fmt::layer()
        .with_timer(timer.clone())
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(env_filter.clone());

    let registry = tracing_subscriber::registry().with(file_layer);

    let con_registry = {
        let console_layer = fmt::layer()
            .with_timer(timer)
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_filter(env_filter.clone());
        registry.with(console_layer)
    };

    con_registry.init();

    std::panic::set_hook(Box::new(|panic_info| {
        error!("panic occurred: {:?}", panic_info);
    }));

    Ok(guard)
}

