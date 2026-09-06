//! ZLE 行编辑器模拟（§4.3、§4.2）。
//!
//! 本模块是一个**纯状态机**：输入按键 → 状态转移（buffer/cursor/undo/history），
//! 不含任何终端 I/O。终端层（GNU Readline / libedit 的 C++ 封装，见
//! `utsh-ffi` 的 `readline` feature）只需把原始按键翻译成 [`engine::Key`] 并调用
//! [`engine::ZleEngine::handle_key`]，再把返回的显示区内容渲染出去。
//!
//! 这也让 zsh-syntax-highlighting / zsh-autosuggestions 等插件可以拿到干净的
//! 渲染状态（buffer、cursor、suggestion、rprompt）做异步高亮与补全建议。

mod engine;

pub use engine::{Key, KeymapMode, Widget, WidgetStatus, ZleEngine};

/// 撤销栈默认上限。
pub use engine::DEFAULT_UNDO_LIMIT;

/// 常用 widget 名（与 zsh 的 `zle -N name function` 注册名对齐）。
pub mod widget_names {
    /// 自插入字符。
    pub const SELF_INSERT: &str = "self-insert";
    /// 接受当前行（回车执行）。
    pub const ACCEPT_LINE: &str = "accept-line";
    /// 光标左移。
    pub const BACKWARD_CHAR: &str = "backward-char";
    /// 光标右移。
    pub const FORWARD_CHAR: &str = "forward-char";
    /// 行首。
    pub const BEGINNING_OF_LINE: &str = "beginning-of-line";
    /// 行尾。
    pub const END_OF_LINE: &str = "end-of-line";
    /// 退格删除。
    pub const BACKWARD_DELETE_CHAR: &str = "backward-delete-char";
    /// 删除光标处字符。
    pub const DELETE_CHAR: &str = "delete-char";
    /// 删除到行尾。
    pub const KILL_LINE: &str = "kill-line";
    /// 删除整行（Ctrl-U）。
    pub const UNIX_LINE_DISCARD: &str = "unix-line-discard";
    /// 撤销。
    pub const UNDO: &str = "undo";
    /// 重做。
    pub const REDO: &str = "redo";
    /// 向后历史搜索（前缀匹配当前 buffer）。
    pub const HISTORY_SEARCH_BACKWARD: &str = "history-search-backward";
    /// 向前历史搜索。
    pub const HISTORY_SEARCH_FORWARD: &str = "history-search-forward";
    /// 切到 Vi 命令模式。
    pub const VI_CMD_MODE: &str = "vi-cmd-mode";
    /// 切回 Vi 插入模式。
    pub const VI_INSERT_MODE: &str = "vi-insert-mode";
}
