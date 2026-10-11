//! 配置:config.toml + 环境变量覆盖。
//!
//! 设计约定:
//! - 每个 section 都实现 `Default`,配置文件缺失字段时用默认值补齐,升级不会因为缺字段启动失败。
//! - 环境变量只覆盖**白名单**里的关键项,名字固定(见 `apply_env`),不做反射式覆盖,
//!   避免拼错的变量静默改变行为。
//! - `validate()` 把"配置错误"和"页面漂移"分开:前者启动即失败,后者运行时才可能发生。

use crate::browser::selector_config::SelectorConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub api_token: String,
    pub max_concurrent_requests: usize,
    pub request_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 9178,
            api_token: "local-dev-token".into(),
            max_concurrent_requests: 1,
            request_timeout_secs: 900,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserSettings {
    pub adapter: String,
    pub chrome_executable: String,
    pub home_url: String,
    pub user_data_dir: String,
    pub headless: bool,
    pub window_width: u32,
    pub window_height: u32,
    pub user_agent: String,
    pub locale: String,
    pub no_sandbox: bool,
    pub launch_timeout_secs: u64,
    pub extra_args: Vec<String>,
    pub login_poll_secs: u64,
    /// 0 表示无限等待用户登录
    pub login_wait_secs: u64,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            adapter: "chromiumoxide".into(),
            chrome_executable: String::new(),
            home_url: "https://chat.deepseek.com/".into(),
            user_data_dir: "./data/browser-profile".into(),
            headless: false,
            window_width: 1440,
            window_height: 900,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                         (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                .into(),
            locale: "zh-CN".into(),
            no_sandbox: false,
            launch_timeout_secs: 60,
            extra_args: vec![],
            login_poll_secs: 3,
            login_wait_secs: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConversationSettings {
    pub reply_timeout_secs: u64,
    pub stable_secs: u64,
    pub poll_interval_ms: u64,
    pub send_retry: usize,
    pub max_rounds_per_conversation: usize,
    pub min_wait_after_send_ms: u64,
}

impl Default for ConversationSettings {
    fn default() -> Self {
        Self {
            reply_timeout_secs: 120,
            stable_secs: 3,
            poll_interval_ms: 500,
            send_retry: 1,
            max_rounds_per_conversation: 12,
            min_wait_after_send_ms: 800,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    pub max_rounds: usize,
    pub max_rounds_limit: usize,
    pub max_json_retries: usize,
    pub prompt_file: String,
    pub record_text_limit: usize,
    /// `prompt_file` 读入后的强提示词模板;不参与反序列化
    #[serde(skip)]
    pub system_prompt: String,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_rounds: 8,
            max_rounds_limit: 24,
            max_json_retries: 2,
            prompt_file: String::new(),
            record_text_limit: 4000,
            system_prompt: String::new(),
        }
    }
}

/// token 估算方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenEstimateMode {
    /// 字符数 / 4(需求默认口径)
    CharsDiv4,
    /// 中日韩字符按 1 token、其余按 4 字符 1 token,比裸 chars/4 更接近真实用量
    CjkAware,
}

impl Default for TokenEstimateMode {
    fn default() -> Self {
        TokenEstimateMode::CharsDiv4
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextSettings {
    pub enabled: bool,
    pub mode: TokenEstimateMode,
    pub max_context_chars: usize,
    pub reserve_chars: usize,
    pub summary_max_chars: usize,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: TokenEstimateMode::CharsDiv4,
            max_context_chars: 24_000,
            reserve_chars: 4_000,
            summary_max_chars: 1_200,
        }
    }
}

impl ContextSettings {
    pub fn tokens_from_chars(&self, chars: usize) -> usize {
        // 聚合统计只有字符数可用,统一按需求默认口径 chars/4 估算。
        chars.div_ceil(4)
    }

    /// 估算一段文本的 token 数。模型只知道"大概多少",所以这里刻意保持廉价、确定性。
    pub fn estimate_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        match self.mode {
            TokenEstimateMode::CharsDiv4 => text.chars().count().div_ceil(4),
            TokenEstimateMode::CjkAware => {
                let mut cjk = 0usize;
                let mut other = 0usize;
                for ch in text.chars() {
                    if is_cjk(ch) {
                        cjk += 1;
                    } else {
                        other += 1;
                    }
                }
                cjk + other.div_ceil(4)
            }
        }
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3000..=0x303F      // CJK 标点
        | 0x3040..=0x30FF    // 假名
        | 0x3400..=0x4DBF    // 扩展 A
        | 0x4E00..=0x9FFF    // 基本区
        | 0xF900..=0xFAFF    // 兼容表意
        | 0xFF00..=0xFFEF    // 全角
        | 0x20000..=0x2FA1F  // 扩展 B~F
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpGetSettings {
    /// 出网白名单:为空表示禁止访问任何主机
    pub allowed_hosts: Vec<String>,
    pub timeout_secs: u64,
    pub max_bytes: usize,
}

impl Default for HttpGetSettings {
    fn default() -> Self {
        Self {
            allowed_hosts: vec![],
            timeout_secs: 10,
            max_bytes: 256 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsSettings {
    pub allow_dangerous: bool,
    pub enabled: Vec<String>,
    pub execute_timeout_secs: u64,
    pub http_get: HttpGetSettings,
}

impl Default for ToolsSettings {
    fn default() -> Self {
        Self {
            allow_dangerous: false,
            enabled: vec!["get_time".into(), "echo".into(), "http_get".into()],
            execute_timeout_secs: 20,
            http_get: HttpGetSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    pub enabled: bool,
    pub dir: String,
    pub log_file: String,
    pub screenshot_dir: String,
    pub jsonl_dir: String,
    pub sqlite_path: String,
    pub record_http_bodies: bool,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: "./logs".into(),
            log_file: "./logs/deeptoolbridge.log".into(),
            screenshot_dir: "./logs/screenshots".into(),
            jsonl_dir: "./logs/records".into(),
            sqlite_path: "./data/dswa.db".into(),
            record_http_bodies: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingSettings {
    /// tracing 级别过滤,等价于 RUST_LOG
    pub level: String,
    /// 写日志文件时是否保留 ANSI 颜色(文件里通常关闭)
    pub ansi_in_file: bool,
    /// 是否把日志同时输出到控制台
    pub stdout: bool,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: "info,ds_webagent=debug,chromiumoxide=warn,hyper=warn".into(),
            ansi_in_file: false,
            stdout: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PricingConfig {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub currency: String,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            input_per_million: 9.0,
            output_per_million: 27.0,
            currency: "CNY".into(),
        }
    }
}

/// 一个可被外部 API 调用的模型:对应网页上「联网搜索 / 深度思考」两个开关的一种组合。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVariant {
    pub id: String,
    pub label: String,
    pub search: bool,
    pub think: bool,
}

impl ModelVariant {
    pub fn new(id: &str, label: &str, search: bool, think: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            search,
            think,
        }
    }

    /// 供日志/提示词使用的人类可读模式名。
    pub fn mode_name(&self) -> &'static str {
        match (self.search, self.think) {
            (true, true) => "搜索且思考",
            (true, false) => "搜索不思考",
            (false, true) => "思考不搜索",
            (false, false) => "普通对话",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsSection {
    /// 请求未指定 model 时使用的变体
    pub default: String,
    pub apply_toggles_on_new_conversation: bool,
    pub variants: Vec<ModelVariant>,
}

impl Default for ModelsSection {
    fn default() -> Self {
        Self {
            default: "ds-web-search".into(),
            apply_toggles_on_new_conversation: true,
            variants: vec![
                ModelVariant::new("ds-web-search", "搜索不思考", true, false),
                ModelVariant::new("ds-web-think", "思考不搜索", false, true),
                ModelVariant::new("ds-web-search-think", "搜索且思考", true, true),
            ],
        }
    }
}

impl ModelsSection {
    pub fn find(&self, id: &str) -> Option<&ModelVariant> {
        if id.is_empty() {
            return self.default_variant();
        }
        self.variants.iter().find(|v| v.id == id)
    }

    pub fn default_variant(&self) -> Option<&ModelVariant> {
        self.variants
            .iter()
            .find(|v| v.id == self.default)
            .or_else(|| self.variants.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub browser: BrowserSettings,
    pub conversation: ConversationSettings,
    pub agent: AgentSettings,
    pub context: ContextSettings,
    pub tools: ToolsSettings,
    pub storage: StorageSettings,
    pub logging: LoggingSettings,
    pub pricing: PricingConfig,
    pub models: ModelsSection,
    pub selectors: SelectorConfig,

    /// 实际加载的配置文件路径(不参与反序列化)
    #[serde(skip)]
    pub config_path: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            browser: BrowserSettings::default(),
            conversation: ConversationSettings::default(),
            agent: AgentSettings::default(),
            context: ContextSettings::default(),
            tools: ToolsSettings::default(),
            storage: StorageSettings::default(),
            logging: LoggingSettings::default(),
            pricing: PricingConfig::default(),
            models: ModelsSection::default(),
            selectors: SelectorConfig::default(),
            config_path: PathBuf::from("config.toml"),
        }
    }
}

impl AppConfig {
    /// 加载配置:`explicit` → `$DSWA_CONFIG` → `./config.toml` → 内置默认值。
    pub fn load(explicit: Option<&Path>) -> anyhow::Result<Self> {
        let path = explicit
            .map(PathBuf::from)
            .or_else(|| std::env::var("DSWA_CONFIG").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("config.toml"));

        let mut cfg = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("读取配置 {} 失败: {e}", path.display()))?;
            toml::from_str::<AppConfig>(&text)
                .map_err(|e| anyhow::anyhow!("解析配置 {} 失败: {e}", path.display()))?
        } else {
            eprintln!(
                "未找到配置文件 {},使用内置默认值(建议先复制一份 config.toml)",
                path.display()
            );
            AppConfig::default()
        };

        cfg.config_path = path;
        cfg.apply_env()?;
        cfg.load_prompt_file()?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// 读取 `[agent].prompt_file` 指定的强提示词模板。
    /// 模板必须保留三处占位符,否则模型拿不到工具清单,启动就失败此直接报错。
    pub fn load_prompt_file(&mut self) -> anyhow::Result<()> {
        let path = self.agent.prompt_file.trim().to_string();
        if path.is_empty() {
            return Ok(());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("读取强提示词文件 {path} 失败: {e}"))?;
        for placeholder in [
            crate::agent::prompt_builder::TOOLS_PLACEHOLDER,
            crate::agent::prompt_builder::MODE_PLACEHOLDER,
            crate::agent::prompt_builder::ENV_PLACEHOLDER,
        ] {
            if !text.contains(placeholder) {
                anyhow::bail!("强提示词文件 {path} 缺少占位符 {placeholder}");
            }
        }
        tracing::info!(file = %path, chars = text.chars().count(), "已加载自定义强提示词模板");
        self.agent.system_prompt = text;
        Ok(())
    }

    /// 环境变量覆盖:只覆盖白名单项。字符串 "true"/"1"/"yes" 视为真。
    pub fn apply_env(&mut self) -> anyhow::Result<()> {
        fn get(key: &str) -> Option<String> {
            std::env::var(key).ok().filter(|v| !v.trim().is_empty())
        }
        fn parse<T: std::str::FromStr>(key: &str, raw: &str) -> anyhow::Result<T> {
            raw.parse::<T>()
                .map_err(|_| anyhow::anyhow!("环境变量 {key}={raw} 无法解析为目标类型"))
        }
        fn truthy(raw: &str) -> bool {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        }

        if let Some(v) = get("DSWA_HOST") {
            self.server.host = v;
        }
        if let Some(v) = get("DSWA_PORT") {
            self.server.port = parse("DSWA_PORT", &v)?;
        }
        if let Some(v) = get("DSWA_API_TOKEN") {
            self.server.api_token = v;
        }
        if let Some(v) = get("DSWA_MAX_CONCURRENT") {
            let max_concurrent: usize = parse::<usize>("DSWA_MAX_CONCURRENT", &v)?.max(1);
            self.server.max_concurrent_requests = max_concurrent;
        }

        // CHROME_PATH 是社区惯用名,一并接受
        if let Some(v) = get("DSWA_CHROME_EXECUTABLE").or_else(|| get("CHROME_PATH")) {
            self.browser.chrome_executable = v;
        }
        if let Some(v) = get("DSWA_HOME_URL") {
            self.browser.home_url = v;
        }
        if let Some(v) = get("DSWA_USER_DATA_DIR") {
            self.browser.user_data_dir = v;
        }
        if let Some(v) = get("DSWA_USER_AGENT") {
            self.browser.user_agent = v;
        }
        if let Some(v) = get("DSWA_HEADLESS") {
            // 允许显式设置成 false;设成 true 会在 validate() 里被拒绝
            self.browser.headless = truthy(&v);
        }
        if let Some(v) = get("DSWA_NO_SANDBOX") {
            self.browser.no_sandbox = truthy(&v);
        }
        if let Some(v) = get("DSWA_REPLY_TIMEOUT_SECS") {
            self.conversation.reply_timeout_secs = parse("DSWA_REPLY_TIMEOUT_SECS", &v)?;
        }
        if let Some(v) = get("DSWA_MAX_ROUNDS") {
            self.agent.max_rounds = parse("DSWA_MAX_ROUNDS", &v)?;
        }
        if let Some(v) = get("DSWA_MAX_CONTEXT_CHARS") {
            self.context.max_context_chars = parse("DSWA_MAX_CONTEXT_CHARS", &v)?;
        }
        if let Some(v) = get("DSWA_TOOLS") {
            self.tools.enabled = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Some(v) = get("DSWA_ALLOW_DANGEROUS") {
            self.tools.allow_dangerous = truthy(&v);
        }
        if let Some(v) = get("DSWA_HTTP_GET_ALLOWED_HOSTS") {
            self.tools.http_get.allowed_hosts = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        // 只对重放/调试有用:强制使用 mock 页面而不是真实 DeepSeek
        if let Some(v) = get("DSWA_FORCE_SELECTOR_FILE") {
            let text = std::fs::read_to_string(&v)
                .map_err(|e| anyhow::anyhow!("读取选择器配置 {v} 失败: {e}"))?;
            self.selectors = toml::from_str(&text)
                .map_err(|e| anyhow::anyhow!("解析选择器配置 {v} 失败: {e}"))?;
        }

        // 日志级别沿用 RUST_LOG 语义,便于故障时临时开 debug
        if let Ok(v) = std::env::var("RUST_LOG") {
            if !v.trim().is_empty() {
                self.logging.level = v;
            }
        }
        if let Some(v) = get("DSWA_LOG_LEVEL") {
            self.logging.level = v;
        }
        if let Some(v) = get("DSWA_STORAGE_DIR") {
            self.storage.dir = v.clone();
            self.storage.log_file = format!("{v}/deeptoolbridge.log");
            self.storage.jsonl_dir = format!("{v}/records");
            self.storage.screenshot_dir = format!("{v}/screenshots");
        }
        if let Some(v) = get("DSWA_SQLITE_PATH") {
            self.storage.sqlite_path = v;
        }
        Ok(())
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.browser.headless {
            anyhow::bail!(
                "硬性约束:必须使用有头真实浏览器,请把 [browser].headless 设为 false"
            );
        }
        if self.browser.home_url.trim().is_empty() {
            anyhow::bail!("[browser].home_url 不能为空");
        }
        if self.browser.user_data_dir.trim().is_empty() {
            anyhow::bail!("[browser].user_data_dir 不能为空(登录态需要独立 profile 目录)");
        }
        if self.server.api_token.trim().is_empty() {
            anyhow::bail!(
                "[server].api_token 不能为空:本地服务同样需要 Bearer token,否则同机任何进程都能驱动你的浏览器"
            );
        }
        if self.context.enabled && self.context.max_context_chars <= self.context.reserve_chars {
            anyhow::bail!(
                "[context].max_context_chars({}) 必须大于 reserve_chars({})",
                self.context.max_context_chars,
                self.context.reserve_chars
            );
        }
        if self.agent.max_rounds == 0 {
            anyhow::bail!("[agent].max_rounds 必须大于 0");
        }
        if self.models.variants.is_empty() {
            anyhow::bail!("[models.variants] 至少需要配置一个模型变体");
        }
        if self.models.find(&self.models.default).is_none() {
            anyhow::bail!(
                "[models].default = {:?} 不在 variants 中",
                self.models.default
            );
        }
        self.selectors.validate().map_err(|e| anyhow::anyhow!(e))?;
        if self.pricing.input_per_million < 0.0 || self.pricing.output_per_million < 0.0 {
            anyhow::bail!("[pricing] 价格不能为负");
        }
        Ok(())
    }

    /// 请求里传来的 max_rounds 会被夹到 [1, max_rounds_limit]。
    pub fn clamp_max_rounds(&self, requested: Option<usize>) -> usize {
        let want = requested.unwrap_or(self.agent.max_rounds);
        want.clamp(1, self.agent.max_rounds_limit.max(1))
    }

    /// 确保所有需要写入的目录存在。
    pub fn ensure_dirs(&self) -> anyhow::Result<()> {
        for dir in [
            self.storage.dir.as_str(),
            self.storage.jsonl_dir.as_str(),
            self.storage.screenshot_dir.as_str(),
            self.browser.user_data_dir.as_str(),
        ] {
            if dir.trim().is_empty() {
                continue;
            }
            std::fs::create_dir_all(dir)
                .map_err(|e| anyhow::anyhow!("创建目录 {dir} 失败: {e}"))?;
        }
        if let Some(parent) = Path::new(&self.storage.sqlite_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("创建目录 {} 失败: {e}", parent.display()))?;
            }
        }
        if let Some(parent) = Path::new(&self.storage.log_file).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("创建目录 {} 失败: {e}", parent.display()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        AppConfig::default().validate().expect("默认配置应合法");
    }

    #[test]
    fn headless_is_rejected() {
        let mut cfg = AppConfig::default();
        cfg.browser.headless = true;
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("有头真实浏览器"), "错误信息应指向 headless: {err}");
    }

    #[test]
    fn toml_roundtrip_keeps_models() {
        let cfg = AppConfig::default();
        let text = toml::to_string(&cfg).unwrap();
        let back: AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(back.models.variants.len(), 3);
        assert_eq!(back.models.variants[0].id, "ds-web-search");
    }

    #[test]
    fn token_estimates_follow_mode() {
        let mut ctx = ContextSettings::default();
        ctx.mode = TokenEstimateMode::CharsDiv4;
        assert_eq!(ctx.estimate_tokens(""), 0);
        assert_eq!(ctx.estimate_tokens("abcd"), 1);
        assert_eq!(ctx.estimate_tokens("abcde"), 2);

        ctx.mode = TokenEstimateMode::CjkAware;
        // 4 个汉字 ≈ 4 tokens,比 chars/4 的 1 更接近真实
        assert_eq!(ctx.estimate_tokens("你好世界"), 4);
        assert_eq!(ctx.estimate_tokens("abcd"), 1);
    }

    #[test]
    fn missing_max_rounds_is_clamped() {
        let cfg = AppConfig::default(); // max_rounds=8, limit=24
        assert_eq!(cfg.clamp_max_rounds(None), 8);
        assert_eq!(cfg.clamp_max_rounds(Some(999)), 24);
        assert_eq!(cfg.clamp_max_rounds(Some(0)), 1);
    }

    #[test]
    fn model_lookup_handles_unknown_and_empty() {
        let models = ModelsSection::default();
        assert_eq!(models.find("ds-web-think").unwrap().mode_name(), "思考不搜索");
        // 未知 / 空模型名:严格返回 None,/v1/chat 会据此报错;OpenAI 入口走默认变体
        assert!(models.find("gpt-4o").is_none());
        // 空模型名视为"用默认变体"(OpenAI 客户端偶尔会传空)
        assert_eq!(models.find("").unwrap().id, models.default);
        assert!(models.default_variant().is_some());
        assert_eq!(models.default_variant().unwrap().id, models.default);
    }
}
