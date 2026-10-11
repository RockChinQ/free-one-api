//! 浏览器层:把"启动真实浏览器 / 打开页面 / 操作 DOM"封装在 `BrowserAdapter` 之后。
//!
//! 业务代码只依赖 [`BrowserAdapter`] trait,不依赖 chromiumoxide 的具体类型;
//! 换成 headless_chrome 或其它实现时只改本模块。

pub mod browser_controller;
pub mod selector_config;

pub use browser_controller::{BrowserAdapter, ChromiumoxideAdapter, LoginState};
pub use selector_config::{SelectorConfig, SubmitMode, TypeStrategy};
