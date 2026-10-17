//! 页面选择器配置。
//!
//! 所有 DOM 选择器都来自 `config.toml` 的 `[selectors]` 表,业务代码只按候选列表顺序
//! 尝试命中。页面改版时改配置即可,不需要动核心逻辑。
//!
//! 每个关键位置都是**候选列表**而不是单条选择器:DeepSeek 的 class 名带构建哈希,
//! 多候选 + 按序命中是这里唯一现实可维护的做法。

use serde::{Deserialize, Serialize};

/// 提交消息的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitMode {
    /// 回车发送(DeepSeek 的默认行为)
    Enter,
    /// 点击发送按钮
    Button,
    /// 先点按钮,按钮不可用时回退回车
    EnterThenButton,
}

impl Default for SubmitMode {
    fn default() -> Self {
        SubmitMode::Enter
    }
}

/// 文本输入方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeStrategy {
    /// 直接用 CDP 输入文本事件
    TypeStr,
    /// 输入后读回输入框值做校验,为空则回退到 JS 原生 setter + input 事件
    /// (React 受控组件对非原生赋值不触发 onChange,需要这个兜底)
    TypeStrVerify,
}

impl Default for TypeStrategy {
    fn default() -> Self {
        TypeStrategy::TypeStrVerify
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectorConfig {
    pub submit_with: SubmitMode,
    pub type_strategy: TypeStrategy,

    // ── 登录检测 ──
    pub login_input: Vec<String>,
    pub login_indicator: Vec<String>,
    pub logged_in_url_markers: Vec<String>,
    /// 命中这些 URL 片段视为"还在登录页"(未登录)。登录页也有手机号输入框,
    /// 所以判定顺序是:先排除登录页,再看聊天输入框是否可用。
    pub not_logged_in_url_markers: Vec<String>,

    // ── 对话操作 ──
    pub new_chat_button: Vec<String>,
    pub chat_input: Vec<String>,
    pub send_button: Vec<String>,
    pub stop_button: Vec<String>,
    /// 「继续生成」按钮。网页在思考或生成中途掉线时会停在那里等用户点它才接着写;
    /// 认不出来就会把"暂停"当成"已答完"(返回半句话)或"没发出去"(重发同一条消息)。
    pub continue_button: Vec<String>,
    /// 思考中标志(思考块)。只用于发送确认阶段判断"消息确实进去了、正在想",不参与文本提取。
    pub thinking_indicator: Vec<String>,

    // ── 消息读取 ──
    pub assistant_message: Vec<String>,
    pub assistant_message_exclude: Vec<String>,
    pub user_message: Vec<String>,

    // ── 开关(联网搜索 / 深度思考)──
    pub toggle_checked_attr: Vec<String>,
    pub toggle_checked_values: Vec<String>,
    pub toggle_checked_class_markers: Vec<String>,
    pub toggle_search: Vec<String>,
    pub toggle_think: Vec<String>,

    // ── 文件/图片上传 ──
    /// 上传按钮(附件/图片图标),点击后触发文件选择器
    pub upload_button: Vec<String>,
    /// 隐藏的文件输入框(点击上传按钮后出现)
    pub file_input: Vec<String>,
    /// 上传区域(拖放区域或点击区域)
    pub upload_area: Vec<String>,

    /// 可选:覆盖 assistant 文本提取脚本,需返回字符串。
    pub assistant_extract_js: String,
}

impl Default for SelectorConfig {
    fn default() -> Self {
        Self {
            submit_with: SubmitMode::default(),
            type_strategy: TypeStrategy::default(),
            login_input: vec![
                "textarea#chat-input".into(),
                "textarea[placeholder*='DeepSeek']".into(),
                "div[contenteditable='true'][role='textbox']".into(),
                "textarea".into(),
            ],
            login_indicator: vec![
                "div[class*='avatar']".into(),
                "img[class*='avatar']".into(),
                "[data-testid='user-avatar']".into(),
                "div[class*='user-info']".into(),
            ],
            logged_in_url_markers: vec!["/a/chat".into(), "/chat/".into()],
            not_logged_in_url_markers: vec!["sign_in".into()],
            new_chat_button: vec![
                "div[class*='new-chat']".into(),
                "button[class*='new-chat']".into(),
                "[data-testid='new-chat']".into(),
            ],
            chat_input: vec![
                "textarea#chat-input".into(),
                "textarea[placeholder*='DeepSeek']".into(),
                "div[contenteditable='true'][role='textbox']".into(),
                "textarea".into(),
            ],
            send_button: vec![
                "div[role='button'][class*='send']".into(),
                "button[class*='send']".into(),
                "[data-testid='send-button']".into(),
            ],
            stop_button: vec![
                "div[role='button'][class*='stop']".into(),
                "button[class*='stop']".into(),
                "[data-testid='stop-button']".into(),
            ],
            continue_button: vec![
                "text=继续生成".into(),
                "text=Continue generating".into(),
                "text=Continue".into(),
                "div[role='button'][class*='continue']".into(),
                "button[class*='continue']".into(),
            ],
            thinking_indicator: vec![
                "div[class*='thinking']".into(),
                "div[class*='reasoning']".into(),
            ],
            assistant_message: vec![
                "div[class*='ds-markdown']".into(),
                "div[class*='markdown']".into(),
            ],
            assistant_message_exclude: vec![
                "div[class*='thinking']".into(),
                "div[class*='reasoning']".into(),
                "div[class*='toolbar']".into(),
            ],
            user_message: vec!["div[class*='user'] div[class*='message']".into()],
            toggle_checked_attr: vec!["aria-checked".into(), "data-state".into()],
            toggle_checked_values: vec!["true".into(), "checked".into(), "on".into()],
            toggle_checked_class_markers: vec![
                "checked".into(),
                "active".into(),
                "selected".into(),
            ],
            toggle_search: vec![
                "button[aria-label*='联网']".into(),
                "[data-testid='search-toggle']".into(),
            ],
            toggle_think: vec![
                "button[aria-label*='深度思考']".into(),
                "[data-testid='think-toggle']".into(),
            ],
            upload_button: vec![
                "button[aria-label*='上传']".into(),
                "button[aria-label*='附件']".into(),
                "button[aria-label*='图片']".into(),
                "div[class*='upload']".into(),
                "div[class*='attach']".into(),
                "label[for*='file']".into(),
                "input[type='file']".into(),
            ],
            file_input: vec![
                "input[type='file']".into(),
                "input[accept*='image']".into(),
                "input[accept*='file']".into(),
            ],
            upload_area: vec![
                "div[class*='upload-area']".into(),
                "div[class*='drop-zone']".into(),
                "div[class*='file-input']".into(),
            ],
            assistant_extract_js: String::new(),
        }
    }
}

impl SelectorConfig {
    /// 校验关键选择器非空——空列表意味着永远不可能命中,属于配置错误而非页面漂移。
    pub fn validate(&self) -> std::result::Result<(), String> {
        let required: [(&str, &Vec<String>); 4] = [
            ("login_input", &self.login_input),
            ("chat_input", &self.chat_input),
            ("assistant_message", &self.assistant_message),
            ("new_chat_button", &self.new_chat_button),
        ];
        for (name, list) in required {
            if list.iter().all(|s| s.trim().is_empty()) {
                return Err(format!(
                    "[selectors].{name} 至少要配置一条选择器,否则无法与页面交互"
                ));
            }
        }
        if self.submit_with != SubmitMode::Enter
            && self.send_button.iter().all(|s| s.trim().is_empty())
        {
            return Err(
                "[selectors].submit_with 选择了按钮发送,但 send_button 为空".to_string(),
            );
        }
        Ok(())
    }

    /// 把候选列表渲染成 JS 数组字面量,供注入脚本使用。
    pub fn js_array(list: &[String]) -> String {
        serde_json::to_string(list).unwrap_or_else(|_| "[]".to_string())
    }
}
