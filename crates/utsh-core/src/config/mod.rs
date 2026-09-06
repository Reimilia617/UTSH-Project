//! 配置系统：解析/写入 `~/.config/ut/utsh.toml`。
//!
//! 结构与设计文档 §4.8 保持一致。解析使用 [`toml`]（serde），缺失的字段回落到
//! 内置默认值（`#[serde(default)]`），因此一个只有 `[general]` 段的文件也能正常
//! 加载。整个解析为纯内存操作，目标 < 1ms，满足“启动 < 50ms”的性能预算。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 配置相关错误。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// 底层 I/O 错误（读/写/建目录）。
    #[error("config io error: {0}")]
    Io(#[from] std::io::Error),
    /// TOML 反序列化错误。
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),
    /// TOML 序列化错误。
    #[error("config serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
    /// 别名名字不合法。
    #[error("invalid alias name `{0}`")]
    InvalidAliasName(String),
    /// 别名的命令内容不合法。
    #[error("invalid alias command for `{0}`: must not be empty or contain newlines")]
    InvalidAliasCommand(String),
}

/// 各节的字段级默认值（供 `#[serde(default = ...)]` 使用；节整体缺失时走
/// 各节的 `Default`，节存在但缺个别字段时用这里的值）。
fn default_editor() -> String {
    "nvim".into()
}
fn default_history_size() -> usize {
    10_000
}
fn default_history_ignore_dups() -> bool {
    true
}
fn default_prompt_format() -> String {
    "utsh".into()
}
fn default_show_git_status() -> bool {
    true
}
fn default_enable_defaults() -> bool {
    true
}
fn default_max_plugins() -> usize {
    50
}
fn default_webui_enabled() -> bool {
    true
}
fn default_webui_port() -> u16 {
    8787
}
fn default_auto_open() -> bool {
    true
}
fn default_theme_current() -> String {
    "none".into()
}

/// `[general]` —— 通用设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// 默认编辑器。
    #[serde(default = "default_editor")]
    pub default_editor: String,
    /// 历史记录上限。
    #[serde(default = "default_history_size")]
    pub history_size: usize,
    /// 是否忽略重复历史。
    #[serde(default = "default_history_ignore_dups")]
    pub history_ignore_dups: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_editor: "nvim".into(),
            history_size: 10_000,
            history_ignore_dups: true,
        }
    }
}

/// `[prompt]` —— 提示符设置（默认不安装任何主题，使用简单提示符）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptConfig {
    /// 提示符格式。`"utsh"` 表示使用内置默认简单提示符。
    #[serde(default = "default_prompt_format")]
    pub format: String,
    /// 是否显示 git 分支状态。
    #[serde(default = "default_show_git_status")]
    pub show_git_status: bool,
    /// 是否在提示符中显示上一条命令的退出码。
    #[serde(default)]
    pub show_exit_code: bool,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            format: "utsh".into(),
            show_git_status: true,
            show_exit_code: false,
        }
    }
}

/// `[plugins]` —— 插件配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 默认加载内置插件（zsh-syntax-highlighting、zsh-autosuggestions）。
    #[serde(default = "default_enable_defaults")]
    pub enable_defaults: bool,
    /// 是否自动更新插件。
    #[serde(default)]
    pub autoupdate: bool,
    /// 允许同时启用的最大第三方插件数（防止滥用）。
    #[serde(default = "default_max_plugins")]
    pub max_plugins: usize,
    /// 显式启用的插件名（不包含内置插件）。
    #[serde(default)]
    pub enabled: Vec<String>,
    /// 每个插件的覆盖配置。
    #[serde(default)]
    pub overrides: BTreeMap<String, toml::Value>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enable_defaults: true,
            autoupdate: false,
            max_plugins: 50,
            enabled: Vec::new(),
            overrides: BTreeMap::new(),
        }
    }
}

/// `[webui]` —— WebUI 服务配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebuiConfig {
    /// 是否允许 `utsh webui` 启动服务。
    #[serde(default = "default_webui_enabled")]
    pub enabled: bool,
    /// 监听端口（默认 8787）。
    #[serde(default = "default_webui_port")]
    pub port: u16,
    /// 启动后自动打开浏览器。
    #[serde(default = "default_auto_open")]
    pub auto_open: bool,
    /// 访问令牌（首次启动自动生成 64 位随机 hex；缺省时 WebUI 不鉴权并告警）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
}

impl Default for WebuiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8787,
            auto_open: true,
            auth_token: None,
        }
    }
}

/// `[theme]` —— 主题配置。默认 `current = "none"`（不安装任何主题）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// 当前主题名（`"none"` 表示不启用主题）。
    #[serde(default = "default_theme_current")]
    pub current: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            current: "none".into(),
        }
    }
}

/// UTSH 顶层配置。对应 `~/.config/ut/utsh.toml`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// `[general]` 通用设置。
    #[serde(default)]
    pub general: GeneralConfig,
    /// `[prompt]` 提示符设置。
    #[serde(default)]
    pub prompt: PromptConfig,
    /// `[plugins]` 插件配置。
    #[serde(default)]
    pub plugins: PluginConfig,
    /// `[aliases]` 别名表（有序）。
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// `[webui]` WebUI 服务配置。
    #[serde(default)]
    pub webui: WebuiConfig,
    /// `[theme]` 主题配置。
    #[serde(default)]
    pub theme: ThemeConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            prompt: PromptConfig::default(),
            plugins: PluginConfig::default(),
            aliases: {
                let mut m = BTreeMap::new();
                m.insert("ll".into(), "ls -alF".into());
                m.insert("gst".into(), "git status".into());
                m.insert("ga".into(), "git add".into());
                m
            },
            webui: WebuiConfig::default(),
            theme: ThemeConfig::default(),
        }
    }
}

/// 别名名字允许的字符集：字母、数字、`_`、`-`、`.`。
fn is_valid_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn is_valid_alias_command(command: &str) -> bool {
    !command.trim().is_empty() && !command.contains('\n')
}

impl Config {
    // ---------- 目录/路径约定（§4.8、§4.10 统一 UT 目录） ----------

    /// 根据 `$XDG_CONFIG_HOME`/`$HOME` 解析配置目录（`<xdg-config>/ut`）。
    pub fn config_dir() -> PathBuf {
        Self::config_dir_from(xdg_config_home(), home_dir())
    }

    fn config_dir_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        xdg.or(home)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ut")
    }

    /// 配置文件默认路径：`~/.config/ut/utsh.toml`。
    pub fn default_config_path() -> PathBuf {
        Self::config_dir().join("utsh.toml")
    }

    /// 数据目录：`~/.local/share/utsh`。
    pub fn data_dir() -> PathBuf {
        Self::data_dir_from(xdg_data_home(), home_dir())
    }

    fn data_dir_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        xdg.or_else(|| home.map(|h| h.join(".local").join("share")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("utsh")
    }

    /// 第三方插件安装目录：`~/.local/share/utsh/plugins`。
    pub fn plugins_dir() -> PathBuf {
        Self::data_dir().join("plugins")
    }

    /// 主题目录：`~/.local/share/utsh/themes`。
    pub fn themes_dir() -> PathBuf {
        Self::data_dir().join("themes")
    }

    /// 缓存目录：`~/.cache/ut`。
    pub fn cache_dir() -> PathBuf {
        Self::cache_dir_from(xdg_cache_home(), home_dir())
    }

    fn cache_dir_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        xdg.or(home)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ut")
    }

    /// 与 Node.js WebUI 通信的 Unix Domain Socket 路径（§4.9）。
    pub fn socket_path() -> PathBuf {
        std::env::var_os("UTSH_SOCK")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/utsh.sock"))
    }

    // ---------- 加载 / 保存 ----------

    /// 从默认路径加载配置。文件不存在时返回内置默认配置（不视为错误）；
    /// 文件损坏（TOML 语法错误）时返回 [`ConfigError::Parse`]。
    pub fn load() -> crate::Result<Self> {
        Self::load_from(Self::default_config_path())
    }

    /// 从指定路径加载。文件不存在时返回内置默认配置。
    pub fn load_from(path: impl AsRef<Path>) -> crate::Result<Self> {
        let path = path.as_ref();
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(path = %path.display(), "config not found, using defaults");
                return Ok(Self::default());
            }
            Err(e) => return Err(e.into()),
        };
        let cfg: Self = toml::from_str(&raw)?;
        Ok(cfg)
    }

    /// 保存到默认路径（自动创建父目录）。
    pub fn save(&self) -> crate::Result<()> {
        self.save_to(Self::default_config_path())
    }

    /// 保存到指定路径（自动创建父目录）。
    pub fn save_to(&self, path: impl AsRef<Path>) -> crate::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let s = toml::to_string_pretty(self)?;
        std::fs::write(path, s)?;
        tracing::debug!(path = %path.display(), "config saved");
        Ok(())
    }

    // ---------- 别名操作（§4.6 alias 子命令） ----------

    /// 只读别名表。
    pub fn aliases(&self) -> &BTreeMap<String, String> {
        &self.aliases
    }

    /// 查询单个别名。
    pub fn alias(&self, name: &str) -> Option<&str> {
        self.aliases.get(name).map(String::as_str)
    }

    /// 新增/覆盖别名。返回 `true` 表示是新增（此前不存在）。
    pub fn add_alias(&mut self, name: &str, command: &str) -> crate::Result<bool> {
        if !is_valid_alias_name(name) {
            return Err(ConfigError::InvalidAliasName(name.into()));
        }
        if !is_valid_alias_command(command) {
            return Err(ConfigError::InvalidAliasCommand(name.into()));
        }
        let is_new = !self.aliases.contains_key(name);
        self.aliases.insert(name.to_string(), command.to_string());
        Ok(is_new)
    }

    /// 删除别名，返回是否存在。
    pub fn remove_alias(&mut self, name: &str) -> bool {
        self.aliases.remove(name).is_some()
    }

    // ---------- 主题操作（§4.6 theme 子命令） ----------

    /// 当前主题名（默认 `"none"`）。
    pub fn theme(&self) -> &str {
        &self.theme.current
    }

    /// 设置主题。
    pub fn set_theme(&mut self, name: impl Into<String>) {
        self.theme.current = name.into();
    }

    // ---------- 插件开关（§4.6 plugin 子命令） ----------

    /// 配置中显式启用的第三方插件（不含内置插件）。
    pub fn enabled_plugins(&self) -> &[String] {
        &self.plugins.enabled
    }

    /// 查询第三方插件是否启用。
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugins.enabled.iter().any(|n| n == name)
    }

    /// 启用插件；超过 `max_plugins` 上限时返回 `false`。
    pub fn enable_plugin(&mut self, name: &str) -> bool {
        if self.is_plugin_enabled(name) {
            return true;
        }
        if self.plugins.enabled.len() >= self.plugins.max_plugins {
            return false;
        }
        self.plugins.enabled.push(name.to_string());
        true
    }

    /// 禁用插件。
    pub fn disable_plugin(&mut self, name: &str) -> bool {
        let before = self.plugins.enabled.len();
        self.plugins.enabled.retain(|n| n != name);
        self.plugins.enabled.len() != before
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn xdg_config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
}

fn xdg_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from)
}

fn xdg_cache_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 设计文档 §4.8 中的示例配置必须能够原样解析。
    #[test]
    fn parses_documented_example() {
        let example = r#"
# UTSH 主配置文件

[general]
default_editor = "nvim"
history_size = 10000
history_ignore_dups = true

[prompt]
format = "utsh"  # 默认不安装主题，使用简单提示符
show_git_status = true
show_exit_code = false

[plugins]
enable_defaults = true
autoupdate = false
max_plugins = 50

[plugins.overrides]
"zsh-syntax-highlighting" = { async = true }
"zsh-autosuggestions" = { strategy = "history" }

[aliases]
ll = "ls -alF"
gst = "git status"
ga = "git add"

[webui]
enabled = true
port = 8787
auto_open = true
auth_token = "auto_generated_random_token"

[theme]
current = "none"
"#;
        let cfg: Config = toml::from_str(example).expect("documented example must parse");
        assert_eq!(cfg.general.default_editor, "nvim");
        assert_eq!(cfg.general.history_size, 10_000);
        assert!(cfg.plugins.enable_defaults);
        assert_eq!(cfg.webui.port, 8787);
        assert_eq!(
            cfg.webui.auth_token.as_deref(),
            Some("auto_generated_random_token")
        );
        assert_eq!(cfg.theme.current, "none");
        assert_eq!(cfg.aliases.get("ll").map(String::as_str), Some("ls -alF"));
    }

    /// 只含部分字段的配置也能加载（字段回落到默认值）。
    #[test]
    fn missing_sections_fall_back_to_defaults() {
        let cfg: Config = toml::from_str("[general]\ndefault_editor = \"vim\"\n").unwrap();
        assert_eq!(cfg.general.default_editor, "vim");
        assert!(cfg.plugins.enable_defaults);
        assert_eq!(cfg.theme.current, "none");
    }

    /// TOML 序列化 → 反序列化往返保持一致。
    #[test]
    fn roundtrip() {
        let mut cfg = Config::default();
        cfg.add_alias("gc", "git checkout").unwrap();
        cfg.set_theme("starship");
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(
            back.aliases.get("gc").map(String::as_str),
            Some("git checkout")
        );
        assert_eq!(back.theme.current, "starship");
    }

    #[test]
    fn alias_validation() {
        let mut cfg = Config::default();
        assert!(cfg.add_alias("ok_name-1", "ls").is_ok());
        assert!(cfg.add_alias("has space", "ls").is_err());
        assert!(cfg.add_alias("", "ls").is_err());
        assert!(cfg.add_alias("x", "").is_err());
        assert!(cfg.add_alias("x", "a\nb").is_err());
        assert!(cfg.remove_alias("ok_name-1"));
        assert!(!cfg.remove_alias("ok_name-1"));
    }

    #[test]
    fn plugin_toggle_obeys_max() {
        let mut cfg = Config::default();
        cfg.plugins.max_plugins = 1;
        assert!(cfg.enable_plugin("a"));
        assert!(!cfg.enable_plugin("b"));
        assert!(cfg.is_plugin_enabled("a"));
        assert!(cfg.disable_plugin("a"));
        assert!(!cfg.is_plugin_enabled("a"));
    }

    /// 目录解析逻辑（纯函数，避免测试中修改进程环境变量）。
    #[test]
    fn dir_resolution() {
        let xdg = Some(PathBuf::from("/xdg"));
        let home = Some(PathBuf::from("/home/u"));
        assert_eq!(
            Config::config_dir_from(xdg.clone(), home.clone()),
            PathBuf::from("/xdg/ut")
        );
        assert_eq!(
            Config::config_dir_from(None, home.clone()),
            PathBuf::from("/home/u/ut")
        );
        assert_eq!(
            Config::data_dir_from(None, home.clone()),
            PathBuf::from("/home/u/.local/share/utsh")
        );
        assert_eq!(
            Config::data_dir_from(Some(PathBuf::from("/xdg-data")), home),
            PathBuf::from("/xdg-data/utsh")
        );
    }

    /// 写入临时目录并重新加载，验证磁盘往返。
    #[test]
    fn save_and_load_file() {
        let dir = std::env::temp_dir().join(format!("utsh-config-test-{}", std::process::id()));
        let path = dir.join("utsh.toml");
        let mut cfg = Config::default();
        cfg.add_alias("t", "true").unwrap();
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, cfg);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
