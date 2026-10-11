//! tracing 初始化:控制台 + 文件(`logs/deeptoolbridge.log`)。
//!
//! 返回的 `WorkerGuard` 必须活到进程结束,否则非阻塞 writer 会被 drop 掉、最后一批日志丢失。

use std::path::Path;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::config::AppConfig;
use crate::error::Result;

pub fn init_tracing(cfg: &AppConfig) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    let filter = EnvFilter::try_new(&cfg.logging.level)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let log_path = Path::new(&cfg.storage.log_file);
    let (dir, file) = match (log_path.parent(), log_path.file_name()) {
        (Some(dir), Some(file)) if !dir.as_os_str().is_empty() => (
            dir.to_path_buf(),
            file.to_string_lossy().to_string(),
        ),
        _ => (std::path::PathBuf::from("."), "deeptoolbridge.log".to_string()),
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("创建日志目录 {} 失败: {e}", dir.display());
    }

    // 文件名固定 + 不轮转:日志路径在 README/验收标准里是硬承诺,不要变成带日期的文件。
    let appender = tracing_appender::rolling::never(&dir, file);
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(cfg.logging.ansi_in_file)
        .with_target(true)
        .with_writer(non_blocking);
    let stdout_layer = cfg.logging.stdout.then(|| {
        tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_target(true)
            .with_writer(std::io::stdout)
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    Ok(Some(guard))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_is_split_into_dir_and_file() {
        let cfg = AppConfig::default();
        let path = Path::new(&cfg.storage.log_file);
        assert_eq!(path.file_name().unwrap(), "deeptoolbridge.log");
        assert!(path.parent().is_some());
    }
}
