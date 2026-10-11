//! 本地 mock 页面集成测试:用一个最小 HTML 页面替代 DeepSeek,验证
//! "输入 → 发送 → 等待流式结束 → 提取文本" 这条浏览器侧链路本身。
//!
//! 不依赖 DeepSeek、不猜它的私有接口;只验证桥接与页面的交互契约。
//!
//! 默认跳过(需要本机有 Chrome/Chromium):`DSWA_E2E=1 cargo test --test mock_page`。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use ds_webagent::browser::{BrowserAdapter, ChromiumoxideAdapter, LoginState};
use ds_webagent::config::AppConfig;
use ds_webagent::testing::{mock_selectors, MOCK_PAGE_HTML};

/// 一个极小的 HTTP/1.1 静态服务器:只回固定 HTML,够驱动浏览器加载页面。
async fn serve_mock(port: u16) {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(_) => return,
    };
    loop {
        let (mut sock, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            // 不关心请求内容,只要读掉避免 RST
            let _ = tokio::time::timeout(Duration::from_millis(500), sock.read(&mut buf)).await;
            let body = MOCK_PAGE_HTML;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test]
async fn input_send_wait_extract_against_local_mock_page() {
    if std::env::var("DSWA_E2E").is_err() {
        eprintln!("skip: 设 DSWA_E2E=1 且本机装有 Chrome/Chromium 时才会运行这个端到端测试");
        return;
    }

    let port = 18_317u16;
    tokio::spawn(serve_mock(port));

    let mut cfg = AppConfig::default();
    cfg.browser.home_url = format!("http://127.0.0.1:{port}/");
    cfg.browser.headless = false;
    cfg.browser.user_data_dir = format!("./data/e2e-profile-{port}");
    cfg.conversation.stable_secs = 1;
    cfg.conversation.poll_interval_ms = 100;
    cfg.conversation.reply_timeout_secs = 20;
    cfg.selectors = mock_selectors();

    let adapter = ChromiumoxideAdapter::new(Arc::new(cfg));
    adapter
        .launch()
        .await
        .expect("浏览器启动失败:确认已安装 Chrome,或在 [browser].chrome_executable 指定路径");

    // mock 页面里有 #avatar,所以应被判定为已登录
    assert_eq!(
        adapter.login_state().await.unwrap(),
        LoginState::LoggedIn,
        "mock 页面带登录标志却没识别为已登录,选择器/JS 有问题"
    );

    adapter
        .new_conversation()
        .await
        .expect("点击「新对话」后应得到空对话");
    adapter
        .type_message("hello")
        .await
        .expect("应能把文本写入输入框");
    adapter
        .submit()
        .await
        .expect("应能提交消息");

    // 等流式结束并提取最后一条 assistant 文本
    let deadline = Instant::now() + Duration::from_secs(15);
    let text;
    loop {
        assert!(Instant::now() < deadline, "等待 mock 页面回复超时");
        if !adapter.is_generating().await.unwrap() {
            if let Some(t) = adapter.last_assistant_text().await.unwrap() {
                if !t.trim().is_empty() {
                    text = t;
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    assert!(
        text.contains("收到了"),
        "提取到的 assistant 文本不符合预期:{text}"
    );

    let _ = adapter.shutdown().await;
}
