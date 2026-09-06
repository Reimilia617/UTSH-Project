use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// UTSH —— 兼容 Bash 语法、兼容 Zsh 插件/主题生态的交互式 Shell 管理工具。
#[derive(Debug, Parser)]
#[command(
    name = "utsh",
    version,
    about = "UTSH 命令行管理工具",
    long_about = "UTSH（UT Shell）：一个兼容 Bash 语法、同时兼容 Zsh 插件与主题生态的\n交互式 Shell。本 CLI 用于管理插件 / 主题 / 别名、诊断环境并启动 WebUI。"
)]
pub struct Cli {
    /// 配置文件路径（默认 `~/.config/ut/utsh.toml`）
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// 日志详细级别（可叠加：-vv 为 debug，-vvv 为 trace）
    #[arg(short = 'v', long = "verbose", global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

/// 顶层子命令。
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 插件管理（安装/卸载/启用/禁用/更新）
    #[command(subcommand)]
    Plugin(PluginAction),
    /// 主题管理（默认不安装任何主题，必须主动 set）
    #[command(subcommand)]
    Theme(ThemeAction),
    /// 别名管理
    #[command(subcommand)]
    Alias(AliasAction),
    /// 显示当前状态（插件数、主题、配置路径等）
    Status,
    /// 环境诊断（Git、权限、配置完整性）
    Doctor,
    /// 启动 WebUI 服务并打开浏览器
    Webui,
}

/// `utsh plugin` 子命令。
#[derive(Debug, Subcommand)]
pub enum PluginAction {
    /// 列出已安装插件
    List,
    /// 浏览官方插件市场
    Browse {
        /// 按分类过滤（syntax / completion / history / prompt / utility）
        category: Option<String>,
    },
    /// 安装插件（自动启用）
    Install {
        /// 插件名（官方名、user/repo 或本地目录名）
        name: String,
        /// 指定来源：Git URL 或 user/repo（缺省时按官方市场或名字解析）
        #[arg(long, value_name = "SOURCE", default_value = "")]
        source: String,
    },
    /// 卸载插件
    Uninstall { name: String },
    /// 启用插件
    Enable { name: String },
    /// 禁用插件
    Disable { name: String },
    /// 更新插件（不指定则全量更新）
    Update {
        /// 只更新指定插件
        name: Option<String>,
    },
}

/// `utsh theme` 子命令。
#[derive(Debug, Subcommand)]
pub enum ThemeAction {
    /// 列出可用主题
    List,
    /// 设置主题
    Set { name: String },
    /// 预览主题（不保存）
    Preview { name: String },
}

/// `utsh alias` 子命令。
#[derive(Debug, Subcommand)]
pub enum AliasAction {
    /// 添加别名
    Add {
        /// 别名名字
        name: String,
        /// 别名内容（命令）
        command: String,
    },
    /// 删除别名
    Remove { name: String },
    /// 列出所有别名
    List,
}

/// 初始化 tracing 日志。`-v` → info，`-vv` → debug，`-vvv` → trace。
pub fn init_tracing(verbose: u8) {
    let default = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
