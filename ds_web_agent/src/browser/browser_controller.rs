//! `BrowserAdapter`:真实浏览器(非 headless)的启动与页面操作。
//!
//! 实现要点:
//! - 只使用 chromiumoxide 的**公开** CDP 能力,不注入任何隐藏 WebDriver 特征、
//!   不改写 `navigator` 对象;`BrowserConfig::hide()` 这类反检测开关刻意不使用。
//! - 页面 DOM 操作一律通过 `config.toml` 的候选选择器解析,读文本用页面内脚本按
//!   "候选列表 + 排除子树"处理,所以页面改版只需改配置。
//! - 出错时抓取 URL / 标题 / 截图组成可诊断错误,而不是抛一个 "element not found"。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::handler::viewport::Viewport;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::{Element, Page};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::browser::selector_config::{SelectorConfig, SubmitMode, TypeStrategy};
use crate::config::AppConfig;
use crate::error::{AppError, Result};

/// 登录态。/health 直接暴露它,调用方据此判断是否 ready。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    Starting,
    WaitingLogin,
    LoggedIn,
    Failed,
}

/// 浏览器能力抽象。换实现(例如 headless_chrome)只需要重新实现这个 trait。
#[async_trait]
pub trait BrowserAdapter: Send + Sync {
    /// 启动浏览器内核并打开首页。
    async fn launch(&self) -> Result<()>;
    async fn login_state(&self) -> Result<LoginState>;
    /// 轮询等待用户手动登录;`timeout` 为 None 表示一直等。
    async fn wait_for_login(&self, wait: Option<Duration>) -> Result<LoginState>;

    async fn current_url(&self) -> Result<String>;
    async fn page_title(&self) -> Result<String>;
    async fn user_agent(&self) -> Result<String>;
    /// 截图用于诊断,返回文件路径。
    async fn screenshot(&self, dir: &Path) -> Result<PathBuf>;

    async fn new_conversation(&self) -> Result<()>;
    /// 校验并按下「联网搜索 / 深度思考」开关,使其等于期望值。
    async fn apply_toggles(&self, search: bool, think: bool) -> Result<()>;
    /// 把文本写入输入框(不清空、不发车)。调用方负责先确认输入框为空。
    async fn type_message(&self, text: &str) -> Result<()>;
    async fn clear_input(&self) -> Result<()>;
    /// 当前输入框内容:发送前用来确认输入框是空的,发送后用来确认"确实发出去了"。
    async fn input_value(&self) -> Result<String>;
    async fn submit(&self) -> Result<()>;

    /// 上传文件到当前对话。path 是本地文件路径。
    /// 先点击上传按钮触发文件选择器,再通过 CDP 设置文件路径。
    async fn upload_file(&self, path: &str) -> Result<()>;

    async fn is_generating(&self) -> Result<bool>;
    async fn assistant_message_count(&self) -> Result<usize>;
    async fn last_assistant_text(&self) -> Result<Option<String>>;
    /// 与 [`BrowserAdapter::last_assistant_text`] 同一节点,但**不剔除思考过程**。
    /// 只用于统计网页对话实际消耗的 token,绝不参与解析。
    async fn last_assistant_full_text(&self) -> Result<Option<String>>;
    /// 执行页面脚本并解析成 JSON,供上层做自定义探测。
    async fn eval_json(&self, js: &str) -> Result<Value>;

    /// 页面诊断串(`url=... title=...`),用于错误上报;不落截图。
    async fn describe_page(&self) -> String;

    async fn shutdown(&self) -> Result<()>;
}

struct AdapterInner {
    browser: Option<Browser>,
    page: Option<Page>,
    handler_task: Option<JoinHandle<()>>,
}

pub struct ChromiumoxideAdapter {
    cfg: Arc<AppConfig>,
    selectors: SelectorConfig,
    inner: Mutex<AdapterInner>,
}

/// 一次选择器解析的结果:命中的元素 + 命中的候选与诊断信息。
struct ResolvedElement {
    element: Element,
    selector: String,
    /// 命中信息(候选选择器列表 / 命中下标 / 是否用了临时句柄),保留下来便于诊断
    #[allow(dead_code)]
    info: Value,
    /// 命中 `text=` 形式时,元素被打上临时标记属性,用完需要清理
    used_temp_handle: bool,
}

impl ChromiumoxideAdapter {
    pub fn new(cfg: Arc<AppConfig>) -> Self {
        let selectors = cfg.selectors.clone();
        Self {
            cfg,
            selectors,
            inner: Mutex::new(AdapterInner {
                browser: None,
                page: None,
                handler_task: None,
            }),
        }
    }

    pub fn selectors(&self) -> &SelectorConfig {
        &self.selectors
    }

    async fn page(&self) -> Result<Page> {
        self.inner
            .lock()
            .await
            .page
            .clone()
            .ok_or(AppError::BrowserNotReady)
    }

    async fn launch_inner(&self) -> Result<()> {
        let b = &self.cfg.browser;
        let launch_timeout = Duration::from_secs(b.launch_timeout_secs.max(5));

        let mut builder = BrowserConfig::builder()
            // 硬性要求:有头真实浏览器
            .with_head()
            .window_size(b.window_width, b.window_height)
            // 不设置 CDP 视口,否则窗口会被强制改成模拟视口尺寸
            .viewport(None::<Viewport>)
            .user_data_dir(PathBuf::from(&b.user_data_dir))
            .launch_timeout(launch_timeout)
            .request_timeout(Duration::from_secs(b.launch_timeout_secs.max(5)))
            .arg(format!("--lang={}", b.locale));

        if !b.chrome_executable.trim().is_empty() {
            builder = builder.chrome_executable(PathBuf::from(b.chrome_executable.trim()));
        }
        if b.no_sandbox {
            // 仅在 root / 容器环境需要;正常桌面环境不要开
            builder = builder.no_sandbox();
        }
        if !b.user_agent.trim().is_empty() {
            // 仅设置 UA 字符串(标准启动参数)。这不是指纹伪装,也不隐藏自动化特征。
            builder = builder.arg(format!("--user-agent={}", b.user_agent.trim()));
        }
        for arg in &b.extra_args {
            if !arg.trim().is_empty() {
                builder = builder.arg(arg.clone());
            }
        }

        let config = builder
            .build()
            .map_err(|e| AppError::Config(format!("构建浏览器启动参数失败: {e}")))?;

        info!(
            "启动浏览器(有头模式,独立 user-data-dir): home={}, headless={}, user_data_dir={}",
            b.home_url, b.headless, b.user_data_dir
        );

        let (browser, mut handler) = Browser::launch(config).await.map_err(|e| {
            AppError::Browser(format!(
                "启动浏览器失败: {e}。请确认已安装 Chrome/Chromium,必要时在 config.toml 的 \
                 [browser].chrome_executable 指定可执行文件路径(或设置 DSWA_CHROME_EXECUTABLE)"
            ))
        })?;

        // handler 必须被持续驱动,否则所有 CDP 调用都会卡住
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    // 拷贝成 owned 再记,避免 tracing 的 Value 在 !Send 闭包里跨 await
                    let msg = err.to_string();
                    debug!("CDP handler 事件错误: {msg}");
                }
            }
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(AppError::browser)?;

        {
            let mut inner = self.inner.lock().await;
            inner.browser = Some(browser);
            inner.page = Some(page.clone());
            inner.handler_task = Some(handler_task);
        }

        page.goto(self.cfg.browser.home_url.as_str())
            .await
            .map_err(|e| {
                AppError::Browser(format!("打开 {} 失败: {e}", self.cfg.browser.home_url))
            })?;

        let url = self.current_url().await.unwrap_or_default();
        info!("已打开 DeepSeek 对话页: {url}");
        Ok(())
    }

    /// 解析候选选择器列表中的第一个命中元素。
    async fn resolve(&self, candidates: &[String]) -> Result<Option<ResolvedElement>> {
        if candidates.is_empty() {
            return Ok(None);
        }
        let js = JS_RESOLVE.replace("__SELECTORS__", &SelectorConfig::js_array(candidates));
        let info = self.eval_json(&js).await?;
        if info.get("found").and_then(Value::as_bool) != Some(true) {
            return Ok(None);
        }
        let selector = info
            .get("selector")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let used_temp_handle = info.get("temp_handle").and_then(Value::as_bool) == Some(true);

        let page = self.page().await?;
        let query = if used_temp_handle {
            format!("[{}]", TEMP_HANDLE_ATTR)
        } else {
            selector.clone()
        };
        let element = page
            .find_element(query.clone())
            .await
            .map_err(|e| AppError::Internal(format!("解析到元素后无法获取句柄({query}): {e}")))?;
        Ok(Some(ResolvedElement {
            element,
            selector,
            info,
            used_temp_handle,
        }))
    }

    async fn release(&self, resolved: &ResolvedElement) {
        if resolved.used_temp_handle {
            let _ = self.eval_json(JS_CLEAR_HANDLE).await;
        }
    }

    /// 组件化的诊断错误:选择器没命中时,必须能一眼看出页面现在长什么样。
    async fn selector_error(&self, selector: &str) -> AppError {
        let url = self.current_url().await.unwrap_or_else(|e| format!("<{e}>"));
        let title = self.page_title().await.unwrap_or_else(|e| format!("<{e}>"));
        let screenshot = match self.screenshot(Path::new("")).await {
            Ok(p) => p.display().to_string(),
            Err(e) => format!("<截图失败: {e}>"),
        };
        warn!(selector, url, title, screenshot, "选择器未命中");
        AppError::SelectorNotFound {
            selector: selector.to_string(),
            url,
            title,
            screenshot,
        }
    }

    async fn eval_string(&self, js: &str) -> Result<String> {
        let page = self.page().await?;
        page.evaluate(js)
            .await
            .map_err(AppError::browser)?
            .into_value::<String>()
            .map_err(|e| AppError::Internal(format!("脚本返回值不是字符串: {e}")))
    }

    async fn read_input_value(&self) -> Result<String> {
        let sel = SelectorConfig::js_array(&self.selectors.chat_input);
        let js = JS_INPUT_VALUE.replace("__SELECTORS__", &sel);
        let v = self.eval_json(&js).await?;
        Ok(v.get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// 写通知/日志时不带截图,避免每次状态查询都落盘。
    pub async fn page_description(&self) -> String {
        let url = self.current_url().await.unwrap_or_default();
        let title = self.page_title().await.unwrap_or_default();
        format!("url={url} title={title}")
    }
}

#[async_trait]
impl BrowserAdapter for ChromiumoxideAdapter {
    async fn launch(&self) -> Result<()> {
        self.launch_inner().await
    }

    async fn login_state(&self) -> Result<LoginState> {
        let url = self.current_url().await.unwrap_or_default();
        // 判定顺序很关键:登录页(sign_in)也有手机号输入框,所以先排除登录页 URL,
        // 再看聊天输入框是否可用——这是"已登录"最稳的信号,不依赖会变的头像类名。
        let on_login_page = self
            .selectors
            .not_logged_in_url_markers
            .iter()
            .any(|m| !m.is_empty() && url.contains(m));
        if on_login_page {
            return Ok(LoginState::WaitingLogin);
        }
        let has_input = self.resolve(&self.selectors.login_input).await?.is_some();
        if has_input {
            return Ok(LoginState::LoggedIn);
        }
        // 兜底:头像类标志或已登录 URL 片段命中
        let has_indicator = self
            .resolve(&self.selectors.login_indicator)
            .await?
            .is_some();
        let url_hit = self
            .selectors
            .logged_in_url_markers
            .iter()
            .any(|m| !m.is_empty() && url.contains(m));
        if has_indicator || url_hit {
            Ok(LoginState::LoggedIn)
        } else {
            Ok(LoginState::WaitingLogin)
        }
    }

    async fn wait_for_login(&self, wait: Option<Duration>) -> Result<LoginState> {
        let started = Instant::now();
        let poll = Duration::from_secs(self.cfg.browser.login_poll_secs.max(1));
        let mut last_log = Instant::now() - Duration::from_secs(60);
        loop {
            match self.login_state().await {
                Ok(LoginState::LoggedIn) => {
                    info!("检测到已登录 DeepSeek");
                    return Ok(LoginState::LoggedIn);
                }
                Ok(_) => {}
                Err(err) => debug!("登录检测暂不可用: {err}"),
            }
            if let Some(limit) = wait {
                if started.elapsed() >= limit {
                    warn!("等待登录超时({} 秒)", limit.as_secs());
                    return Ok(LoginState::WaitingLogin);
                }
            }
            if last_log.elapsed() >= Duration::from_secs(30) {
                info!("等待用户在该浏览器窗口中手动登录 DeepSeek…");
                last_log = Instant::now();
            }
            sleep(poll).await;
        }
    }

    async fn current_url(&self) -> Result<String> {
        let page = self.page().await?;
        Ok(page
            .url()
            .await
            .map_err(AppError::browser)?
            .unwrap_or_default())
    }

    async fn page_title(&self) -> Result<String> {
        let page = self.page().await?;
        Ok(page
            .get_title()
            .await
            .map_err(AppError::browser)?
            .unwrap_or_default())
    }

    async fn user_agent(&self) -> Result<String> {
        self.eval_string("navigator.userAgent").await
    }

    async fn screenshot(&self, dir: &Path) -> Result<PathBuf> {
        let dir = if dir.as_os_str().is_empty() {
            PathBuf::from(&self.cfg.storage.screenshot_dir)
        } else {
            dir.to_path_buf()
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Storage(format!("创建截图目录 {} 失败: {e}", dir.display())))?;
        let path = dir.join(format!(
            "page_{}.png",
            chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
        ));
        let page = self.page().await?;
        let bytes = page
            .screenshot(ScreenshotParams::builder().build())
            .await
            .map_err(AppError::browser)?;
        std::fs::write(&path, bytes)
            .map_err(|e| AppError::Storage(format!("写入截图 {} 失败: {e}", path.display())))?;
        Ok(path)
    }

    async fn new_conversation(&self) -> Result<()> {
        let resolved = match self.resolve(&self.selectors.new_chat_button).await? {
            Some(r) => r,
            None => {
                return Err(self
                    .selector_error(&self.selectors.new_chat_button.join(" | "))
                    .await)
            }
        };
        let before_url = self.current_url().await.unwrap_or_default();
        let before_rounds = self.assistant_message_count().await.unwrap_or(0);
        debug!(selector = %resolved.selector, "点击「新对话」");
        let _ = resolved.element.click().await.map_err(AppError::browser)?;
        self.release(&resolved).await;

        // 判定新对话就绪:assistant 消息清零(或 URL 变化)且输入框可用
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let rounds = self.assistant_message_count().await.unwrap_or(usize::MAX);
            let url = self.current_url().await.unwrap_or_default();
            let input_ready = self.resolve(&self.selectors.chat_input).await?.is_some();
            if input_ready && rounds == 0 {
                info!("已进入新对话(URL={url})");
                return Ok(());
            }
            if Instant::now() >= deadline {
                if url != before_url && before_rounds > 0 {
                    warn!("新对话 URL 已变化但消息计数未清零,继续执行");
                    return Ok(());
                }
                return Err(AppError::SendFailed(format!(
                    "点击「新对话」后 20 秒内未就绪(url={url}, assistant 计数={rounds})"
                )));
            }
            sleep(Duration::from_millis(self.cfg.conversation.poll_interval_ms.max(100))).await;
        }
    }

    async fn apply_toggles(&self, search: bool, think: bool) -> Result<()> {
        self.apply_one_toggle(&self.selectors.toggle_search, search, "联网搜索")
            .await?;
        self.apply_one_toggle(&self.selectors.toggle_think, think, "深度思考")
            .await
    }

    async fn type_message(&self, text: &str) -> Result<()> {
        let resolved = match self.resolve(&self.selectors.chat_input).await? {
            Some(r) => r,
            None => {
                return Err(self
                    .selector_error(&self.selectors.chat_input.join(" | "))
                    .await)
            }
        };

        // 先点一下聚焦,再走 CDP 文本输入:这是对 React 受控组件最稳的路径
        let _ = resolved.element.click().await.map_err(AppError::browser)?;
        let typed = resolved.element.type_str(text).await.is_ok();

        let mut need_fallback = !typed;
        if !need_fallback && self.selectors.type_strategy == TypeStrategy::TypeStrVerify {
            // 输入后读回校验:某些受控组件会吞掉非原生的赋值
            let current = self.input_value().await.unwrap_or_default();
            need_fallback = current.trim() != text.trim();
        }
        if need_fallback {
            debug!(selector = %resolved.selector, "type_str 未生效,回退到 JS 原生 setter + input 事件");
            self.js_set_input(text).await?;
        }
        self.release(&resolved).await;
        Ok(())
    }

    async fn clear_input(&self) -> Result<()> {
        let sel = SelectorConfig::js_array(&self.selectors.chat_input);
        let js = JS_CLEAR_INPUT.replace("__SELECTORS__", &sel);
        self.eval_json(&js).await?;
        Ok(())
    }

    async fn submit(&self) -> Result<()> {
        match self.selectors.submit_with {
            SubmitMode::Button => {
                self.click_send_button().await?;
                Ok(())
            }
            SubmitMode::Enter => {
                let resolved = match self.resolve(&self.selectors.chat_input).await? {
                    Some(r) => r,
                    None => {
                        return Err(self
                            .selector_error(&self.selectors.chat_input.join(" | "))
                            .await)
                    }
                };
                let res = resolved
                    .element
                    .press_key("Enter")
                    .await
                    .map_err(AppError::browser);
                self.release(&resolved).await;
                res.map(|_| ())
            }
            SubmitMode::EnterThenButton => {
                if self.click_send_button().await.is_err() {
                    let resolved = self
                        .resolve(&self.selectors.chat_input)
                        .await?
                        .ok_or(AppError::SelectorNotFound {
                            selector: "chat_input".into(),
                            url: String::new(),
                            title: String::new(),
                            screenshot: String::new(),
                        })?;
                    let res = resolved
                        .element
                        .press_key("Enter")
                        .await
                        .map_err(AppError::browser);
                    self.release(&resolved).await;
                    res.map(|_| ())?;
                }
                Ok(())
            }
        }
    }

    async fn upload_file(&self, path: &str) -> Result<()> {
        use std::path::Path;
        let p = Path::new(path);
        if !p.exists() {
            return Err(AppError::BadRequest(format!("文件不存在: {}", path)));
        }
        let abs = std::fs::canonicalize(p)
            .map_err(|e| AppError::BadRequest(format!("无法解析文件路径 {}: {}", path, e)))?;
        let file_path = abs.to_str().ok_or_else(|| {
            AppError::BadRequest(format!("文件路径包含非 UTF-8 字符: {}", path))
        })?;

        // 先尝试直接找 file input(有些页面它一直存在,只是隐藏)
        let resolved = self.resolve(&self.selectors.file_input).await?;
        let element = match resolved {
            Some(r) => r,
            None => {
                // 找不到就点上传按钮触发文件选择器
                let btn = self.resolve(&self.selectors.upload_button).await?;
                let btn = match btn {
                    Some(b) => b,
                    None => {
                        return Err(self
                            .selector_error(&self.selectors.upload_button.join(" | "))
                            .await)
                    }
                };
                let _ = btn.element.click().await.map_err(AppError::browser)?;
                self.release(&btn).await;
                // 等文件选择器出现
                tokio::time::sleep(Duration::from_millis(500)).await;
                match self.resolve(&self.selectors.file_input).await? {
                    Some(r) => r,
                    None => {
                        return Err(self
                            .selector_error(&self.selectors.file_input.join(" | "))
                            .await)
                    }
                }
            }
        };

        // 用 chromiumoxide 的 upload_file 通过 CDP 设置文件
        element
            .element
            .upload_file(file_path)
            .await
            .map_err(|e| {
                AppError::Browser(format!("上传文件失败({}): {}", file_path, e))
            })?;
        self.release(&element).await;

        // 等文件上传完成(页面会显示上传进度/预览)
        tokio::time::sleep(Duration::from_secs(2)).await;
        info!("文件已上传: {}", file_path);
        Ok(())
    }

    async fn is_generating(&self) -> Result<bool> {
        Ok(self.resolve(&self.selectors.stop_button).await?.is_some())
    }

    async fn assistant_message_count(&self) -> Result<usize> {
        let sel = SelectorConfig::js_array(&self.selectors.assistant_message);
        let js = JS_COUNT.replace("__SELECTORS__", &sel);
        let v = self.eval_json(&js).await?;
        Ok(v.get("count").and_then(Value::as_u64).unwrap_or(0) as usize)
    }

    async fn last_assistant_text(&self) -> Result<Option<String>> {
        self.read_assistant_text(true).await
    }

    async fn last_assistant_full_text(&self) -> Result<Option<String>> {
        self.read_assistant_text(false).await
    }

    async fn eval_json(&self, js: &str) -> Result<Value> {
        let page = self.page().await?;
        let raw = page
            .evaluate(js)
            .await
            .map_err(AppError::browser)?
            .into_value::<String>()
            .map_err(|e| AppError::Internal(format!("脚本返回值不是字符串: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::Internal(format!("脚本返回非法 JSON: {e}; raw={raw}")))
    }

    async fn input_value(&self) -> Result<String> {
        self.read_input_value().await
    }

    async fn describe_page(&self) -> String {
        self.page_description().await
    }

    async fn shutdown(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.page = None;
        if let Some(task) = inner.handler_task.take() {
            task.abort();
        }
        if let Some(mut browser) = inner.browser.take() {
            let _ = browser.close().await;
            let _ = browser.kill().await;
        }
        info!("浏览器已关闭");
        Ok(())
    }
}

impl ChromiumoxideAdapter {
    /// `strip_excludes = true`:剔除思考过程/工具栏子树,得到交给模型解析的正文。
    /// `strip_excludes = false`:保留思考过程,仅用于 token 统计。
    async fn read_assistant_text(&self, strip_excludes: bool) -> Result<Option<String>> {
        let selectors = SelectorConfig::js_array(&self.selectors.assistant_message);
        let excludes = SelectorConfig::js_array(&self.selectors.assistant_message_exclude);
        let js = if strip_excludes && !self.selectors.assistant_extract_js.trim().is_empty() {
            self.selectors
                .assistant_extract_js
                .replace("__ASSISTANT__", &selectors)
                .replace("__EXCLUDES__", &excludes)
        } else if strip_excludes {
            JS_LAST_ASSISTANT
                .replace("__SELECTORS__", &selectors)
                .replace("__EXCLUDES__", &excludes)
        } else {
            JS_LAST_ASSISTANT_FULL.replace("__SELECTORS__", &selectors)
        };
        let v = self.eval_json(&js).await?;
        if v.get("found").and_then(Value::as_bool) != Some(true) {
            return Ok(None);
        }
        Ok(Some(
            v.get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ))
    }

    async fn click_send_button(&self) -> Result<()> {
        let resolved = match self.resolve(&self.selectors.send_button).await? {
            Some(r) => r,
            None => {
                return Err(self
                    .selector_error(&self.selectors.send_button.join(" | "))
                    .await)
            }
        };
        let _ = resolved.element.click().await.map_err(AppError::browser)?;
        self.release(&resolved).await;
        Ok(())
    }

    /// 读开关状态 → 与期望不符则点击 → 复读确认。
    async fn apply_one_toggle(&self, candidates: &[String], want: bool, name: &str) -> Result<()> {
        if candidates.iter().all(|c| c.trim().is_empty()) {
            warn!(toggle = name, "未配置开关选择器,跳过");
            return Ok(());
        }
        let resolved = match self.resolve(candidates).await? {
            Some(r) => r,
            None => {
                // 开关可能因账号/模型差异不存在,这里只告警不阻断
                warn!(toggle = name, "未找到开关,按页面当前状态继续");
                return Ok(());
            }
        };
        let state = self.toggle_state(&resolved.selector).await?;
        let state_on = state.get("on").and_then(Value::as_bool);
        if state_on == Some(want) {
            debug!(toggle = name, on = want, "开关状态已符合期望");
            self.release(&resolved).await;
            return Ok(());
        }
        let _ = resolved.element.click().await.map_err(AppError::browser)?;
        self.release(&resolved).await;
        sleep(Duration::from_millis(300)).await;
        let after = self.toggle_state(&resolved.selector).await?;
        let after_on = after.get("on").and_then(Value::as_bool);
        if after_on != Some(want) {
            warn!(
                toggle = name,
                want,
                got = ?after_on,
                "开关点击后状态未达到期望值,可能页面结构已变化"
            );
        } else {
            info!(toggle = name, on = want, "已设置开关");
        }
        Ok(())
    }

    async fn toggle_state(&self, selector: &str) -> Result<Value> {
        let js = JS_TOGGLE_STATE
            .replace("__SELECTOR__", &serde_json::to_string(selector).unwrap_or_default())
            .replace(
                "__ATTRS__",
                &SelectorConfig::js_array(&self.selectors.toggle_checked_attr),
            )
            .replace(
                "__VALUES__",
                &SelectorConfig::js_array(&self.selectors.toggle_checked_values),
            )
            .replace(
                "__MARKERS__",
                &SelectorConfig::js_array(&self.selectors.toggle_checked_class_markers),
            );
        self.eval_json(&js).await
    }

    async fn js_set_input(&self, text: &str) -> Result<()> {
        let sel = SelectorConfig::js_array(&self.selectors.chat_input);
        let js = JS_SET_INPUT
            .replace("__SELECTORS__", &sel)
            .replace("__TEXT__", &serde_json::to_string(text).unwrap_or_default());
        let v = self.eval_json(&js).await?;
        if v.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(AppError::SendFailed(format!(
                "写入输入框失败: {}",
                v.get("reason").and_then(Value::as_str).unwrap_or("未知")
            )));
        }
        Ok(())
    }
}

// ── 页面内脚本 ────────────────────────────────────────────────────────────
// 用 `__TOKEN__` 占位符而不是 format!,避免 JS 里的花括号需要转义。

const TEMP_HANDLE_ATTR: &str = "data-dswa-handle";

/// 解析候选选择器:返回第一个命中的候选。支持 `text=按钮文字` 形式。
/// 命中 text= 形式时给元素打临时属性,让 Rust 侧能拿到句柄。
const JS_RESOLVE: &str = r#"(function(){
  var sels = __SELECTORS__;
  var ATTR = "data-dswa-handle";
  function vis(el){
    try { return !!(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length)); }
    catch(e) { return !!el; }
  }
  function byText(t){
    var all = document.querySelectorAll('button,a,[role="button"],div');
    for (var i = 0; i < all.length; i++) {
      var el = all[i];
      if (!vis(el)) continue;
      var txt = (el.innerText || el.textContent || '').trim();
      if (txt === t) return el;
    }
    return null;
  }
  function q(sel){
    if (sel.indexOf('text=') === 0) return byText(sel.slice(5));
    var list = [];
    try { list = document.querySelectorAll(sel); } catch(e) { return null; }
    for (var i = 0; i < list.length; i++) { if (vis(list[i])) return list[i]; }
    return list.length ? list[0] : null;
  }
  for (var i = 0; i < sels.length; i++) {
    var el = q(sels[i]);
    if (!el) continue;
    var count = 0;
    try { count = document.querySelectorAll(sels[i]).length; } catch(e) {}
    var isText = sels[i].indexOf('text=') === 0;
    if (isText) { el.setAttribute(ATTR, "1"); }
    return JSON.stringify({
      found: true, index: i, selector: sels[i], temp_handle: isText, count: count,
      tag: el.tagName,
      value: (el.value === undefined ? null : el.value),
      text: ((el.innerText || el.textContent || '') + '').slice(0, 200),
      disabled: !!el.disabled,
      visible: vis(el)
    });
  }
  return JSON.stringify({ found: false });
})()"#;

const JS_CLEAR_HANDLE: &str = r#"(function(){
  var els = document.querySelectorAll('[data-dswa-handle]');
  for (var i = 0; i < els.length; i++) { els[i].removeAttribute('data-dswa-handle'); }
  return JSON.stringify({ ok: true, cleared: els.length });
})()"#;

const JS_INPUT_VALUE: &str = r#"(function(){
  var sels = __SELECTORS__;
  for (var i = 0; i < sels.length; i++) {
    var el = null;
    try { el = document.querySelector(sels[i]); } catch(e) { continue; }
    if (!el) continue;
    var v = (el.value !== undefined) ? el.value : (el.innerText || '');
    return JSON.stringify({ value: v, selector: sels[i] });
  }
  return JSON.stringify({ value: null });
})()"#;

/// React 受控组件的原生 setter 兜底:必须走 prototype 上的 setter 再派发 input 事件,
/// 直接赋 `el.value` 不会触发框架的 onChange。
const JS_SET_INPUT: &str = r#"(function(){
  var sels = __SELECTORS__;
  var text = __TEXT__;
  function setNative(el){
    var proto = el.tagName === 'TEXTAREA'
      ? window.HTMLTextAreaElement.prototype
      : window.HTMLInputElement.prototype;
    var setter = Object.getOwnPropertyDescriptor(proto, 'value');
    if (setter && setter.set) { setter.set.call(el, text); } else { el.value = text; }
  }
  for (var i = 0; i < sels.length; i++) {
    var el = null;
    try { el = document.querySelector(sels[i]); } catch(e) { continue; }
    if (!el) continue;
    el.focus();
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') {
      setNative(el);
    } else {
      el.textContent = text;
    }
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    var got = (el.value !== undefined) ? el.value : (el.innerText || '');
    return JSON.stringify({ ok: got.trim() === text.trim(), selector: sels[i], got: got.slice(0, 80) });
  }
  return JSON.stringify({ ok: false, reason: '输入框未找到' });
})()"#;

const JS_CLEAR_INPUT: &str = r#"(function(){
  var sels = __SELECTORS__;
  for (var i = 0; i < sels.length; i++) {
    var el = null;
    try { el = document.querySelector(sels[i]); } catch(e) { continue; }
    if (!el) continue;
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') {
      var proto = el.tagName === 'TEXTAREA'
        ? window.HTMLTextAreaElement.prototype
        : window.HTMLInputElement.prototype;
      var setter = Object.getOwnPropertyDescriptor(proto, 'value');
      if (setter && setter.set) { setter.set.call(el, ''); } else { el.value = ''; }
    } else {
      el.textContent = '';
    }
    el.dispatchEvent(new Event('input', { bubbles: true }));
    return JSON.stringify({ ok: true, selector: sels[i] });
  }
  return JSON.stringify({ ok: false });
})()"#;

const JS_COUNT: &str = r#"(function(){
  var sels = __SELECTORS__;
  for (var i = 0; i < sels.length; i++) {
    var n = 0;
    try { n = document.querySelectorAll(sels[i]).length; } catch(e) { continue; }
    if (n > 0) { return JSON.stringify({ count: n, selector: sels[i] }); }
  }
  return JSON.stringify({ count: 0 });
})()"#;

/// 读最后一条 assistant 文本。先克隆节点再剔除"思考过程/工具栏"等子树,
/// 这样正文里不会混进模型的思维链。
const JS_LAST_ASSISTANT: &str = r#"(function(){
  var sels = __SELECTORS__;
  var excl = __EXCLUDES__;
  var nodes = [];
  for (var i = 0; i < sels.length; i++) {
    var list = [];
    try { list = Array.prototype.slice.call(document.querySelectorAll(sels[i])); } catch(e) { continue; }
    if (list.length) { nodes = list; break; }
  }
  if (!nodes.length) { return JSON.stringify({ found: false, count: 0, text: '' }); }
  var last = nodes[nodes.length - 1];
  var clone = last.cloneNode(true);
  for (var j = 0; j < excl.length; j++) {
    var bad = [];
    try { bad = clone.querySelectorAll(excl[j]); } catch(e) { continue; }
    for (var k = 0; k < bad.length; k++) {
      if (bad[k].parentNode) { bad[k].parentNode.removeChild(bad[k]); }
    }
  }
  var text = (clone.innerText || clone.textContent || '').trim();
  return JSON.stringify({ found: true, count: nodes.length, text: text });
})()"#;

/// 同 JS_LAST_ASSISTANT,但**保留**思考过程等子树 —— 只用于统计实际消耗的 token。
const JS_LAST_ASSISTANT_FULL: &str = r#"(function(){
  var sels = __SELECTORS__;
  var nodes = [];
  for (var i = 0; i < sels.length; i++) {
    var list = [];
    try { list = Array.prototype.slice.call(document.querySelectorAll(sels[i])); } catch(e) { continue; }
    if (list.length) { nodes = list; break; }
  }
  if (!nodes.length) { return JSON.stringify({ found: false, count: 0, text: '' }); }
  var last = nodes[nodes.length - 1];
  var text = (last.innerText || last.textContent || '').trim();
  return JSON.stringify({ found: true, count: nodes.length, text: text });
})()"#;

const JS_TOGGLE_STATE: &str = r#"(function(){
  var sel = __SELECTOR__;
  var attrs = __ATTRS__;
  var values = __VALUES__;
  var markers = __MARKERS__;
  var el = null;
  try { el = document.querySelector(sel); } catch(e) { return JSON.stringify({ found: false, on: null }); }
  if (!el) { return JSON.stringify({ found: false, on: null }); }
  for (var i = 0; i < attrs.length; i++) {
    var v = el.getAttribute(attrs[i]);
    if (v !== null && v !== undefined) {
      var lv = ('' + v).toLowerCase();
      if (values.indexOf(lv) >= 0) { return JSON.stringify({ found: true, on: true, via: attrs[i] }); }
      if (lv === 'false' || lv === 'off' || lv === 'unchecked') {
        return JSON.stringify({ found: true, on: false, via: attrs[i] });
      }
    }
  }
  var cls = (el.className || '') + '';
  for (var j = 0; j < markers.length; j++) {
    if (cls.toLowerCase().indexOf(markers[j]) >= 0) {
      return JSON.stringify({ found: true, on: true, via: 'class:' + markers[j] });
    }
  }
  var state = el.getAttribute('data-state');
  if (state) { return JSON.stringify({ found: true, on: state === 'on', via: 'data-state' }); }
  return JSON.stringify({ found: true, on: null, via: 'unknown' });
})()"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_templates_keep_placeholder_tokens() {
        // 占位符被误删会导致脚本静默变成"找不到元素",这里做一次廉价护栏
        assert!(JS_RESOLVE.contains("__SELECTORS__"));
        assert!(JS_LAST_ASSISTANT.contains("__SELECTORS__"));
        assert!(JS_LAST_ASSISTANT.contains("__EXCLUDES__"));
        assert!(JS_LAST_ASSISTANT_FULL.contains("__SELECTORS__"));
        assert!(!JS_LAST_ASSISTANT_FULL.contains("__EXCLUDES__"));
        assert!(JS_SET_INPUT.contains("__TEXT__"));
        assert!(JS_TOGGLE_STATE.contains("__SELECTOR__"));
        assert!(JS_TOGGLE_STATE.contains("__ATTRS__"));
    }

    #[test]
    fn selector_json_is_escaped_for_js() {
        let arr = SelectorConfig::js_array(&["div[class*='a']".to_string(), "text=新对话".to_string()]);
        assert!(arr.starts_with('['));
        assert!(arr.contains("text=新对话"));
    }
}
