//! 可执行入口:后台启动真实浏览器并打开 DeepSeek → 等用户手动登录 → 提供本地 HTTP API。
//!
//! 启动顺序刻意分成两段:
//! 1. 先起 HTTP 服务,`/health` 在浏览器还没就绪时返回 `starting`/`login_required`,
//!    客户端因此能在"等待用户登录"期间轮询,而不是连接失败;
//! 2. 浏览器在独立任务里启动,失败只影响依赖浏览器的端点,不会拖垮整个进程。

use std::sync::Arc;

use clap::Parser;
use ds_webagent::agent::AgentLoop;
use ds_webagent::api::{router, AppState};
use ds_webagent::browser::{BrowserAdapter, ChromiumoxideAdapter};
use ds_webagent::config::AppConfig;
use ds_webagent::conversation::{BrowserChatBackend, ChatBackend};
use ds_webagent::storage::{logger, Store};
use ds_webagent::tools::ToolRegistry;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "ds_webagent",
    version,
    about = "把 DeepSeek 网页对话封装成本地 HTTP API(OpenAI 兼容外观)"
)]
struct Cli {
    /// 配置文件路径(默认 ./config.toml,也可用 DSWA_CONFIG 指定)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
    /// 只起 HTTP 服务、不启动浏览器(只用来调试 API 契约)
    #[arg(long)]
    no_browser: bool,
    /// 打印生效配置后退出
    #[arg(long)]
    print_config: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // `AppConfig::load` 内部已经完成:环境变量覆盖 → 强提示词文件 → 校验
    let cfg = Arc::new(AppConfig::load(cli.config.as_deref())?);
    cfg.ensure_dirs()?;
    let _log_guard = logger::init_tracing(&cfg).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    info!(
        version = ds_webagent::VERSION,
        config = %cfg.config_path.display(),
        adapter = %cfg.browser.adapter,
        "DS_WebAgent 启动"
    );

    if cli.print_config {
        println!("{}", toml::to_string_pretty(&*cfg)?);
        return Ok(());
    }

    let store = Arc::new(Store::open(&cfg)?);
    let browser: Arc<dyn BrowserAdapter> =
        Arc::new(ChromiumoxideAdapter::new(cfg.clone()));
    let backend: Arc<dyn ChatBackend> =
        Arc::new(BrowserChatBackend::new(browser.clone(), cfg.clone()));
    let registry = Arc::new(ToolRegistry::new(&cfg));
    let agent = Arc::new(AgentLoop::new(
        backend.clone(),
        registry.clone(),
        cfg.clone(),
        store.clone(),
    ));

    // ---------- 浏览器:独立任务,失败不影响 /health ----------
    if cli.no_browser {
        warn!("--no-browser:已跳过浏览器启动,依赖浏览器的端点会返回 browser_error");
    } else {
        let browser = browser.clone();
        let cfg = cfg.clone();
        tokio::spawn(async move {
            if let Err(e) = browser.launch().await {
                error!(error = %e, "浏览器启动失败");
                return;
            }
            info!(home = %cfg.browser.home_url, "浏览器已启动,请在窗口中手动登录 DeepSeek");
            match browser.wait_for_login(None).await {
                Ok(state) => info!(?state, "登录状态就绪"),
                Err(e) => error!(error = %e, "等待登录时出错"),
            }
        });
    }

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let state = AppState {
        cfg: cfg.clone(),
        browser,
        backend,
        agent,
        registry,
        store,
    };

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(
        addr = %addr,
        auth = if cfg.server.api_token.trim().is_empty() { "关闭(仅本机)" } else { "Bearer" },
        "HTTP 服务已监听"
    );
    println!("DS_WebAgent 已启动:http://{addr}/health");

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("已退出");
    Ok(())
}

/// Ctrl-C / SIGTERM 时优雅退出,让日志 guard 来得及刷盘。
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("收到 Ctrl-C"),
        _ = terminate => info!("收到 SIGTERM"),
    }
}
