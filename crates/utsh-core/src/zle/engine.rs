//! ZLE 行编辑器**状态机**核心（§4.3）。
//!
//! 与 zsh 的 zle 对应关系：
//! - `buffer` + `cursor`：当前编辑行；
//! - `widgets`：`zle -N name widget` 注册的自定义 widget 表；
//! - `keymap`：`bindkey` 键绑定表（按键 → widget 名）；
//! - `undo/redo`、`kill_ring`：编辑历史与剪切环；
//! - `history`：历史搜索的输入源；
//! - `rprompt` / `suggestion`：RPROMPT 与 POSTDISPLAY（自动补全建议）。
//!
//! 引擎不接触终端：终端驱动层把原始字节翻译成 [`Key`]，见 `utsh-ffi` 的
//! `readline` feature（GNU Readline/libedit 封装）。

use std::collections::HashMap;

/// 撤销栈默认上限。
pub const DEFAULT_UNDO_LIMIT: usize = 200;

/// 一次按键事件（已由终端适配层归一化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// 可打印字符。
    Char(char),
    /// Ctrl + 字母（内部统一存小写字母）。
    Ctrl(char),
    /// Alt/Meta + 字符。
    Meta(char),
    /// 回车。
    Enter,
    /// Tab（补全入口）。
    Tab,
    /// Esc。
    Esc,
    /// 退格。
    Backspace,
    /// 删除键。
    Delete,
    /// 上方向键。
    Up,
    /// 下方向键。
    Down,
    /// 左方向键。
    Left,
    /// 右方向键。
    Right,
    /// Home。
    Home,
    /// End。
    End,
}

impl Key {
    /// 从 readline 风格字符串解析按键：`^A`、`^[`、`^M`、`^I`、`^?`、
    /// `<Up>`/`<Down>`/`<Left>`/`<Right>`/`<Home>`/`<End>`/`<Del>`、单字符。
    pub fn from_readline_notation(s: &str) -> Option<Key> {
        if s.len() == 1 {
            let c = s.chars().next()?;
            return Some(Key::Char(c));
        }
        match s {
            "<Up>" => return Some(Key::Up),
            "<Down>" => return Some(Key::Down),
            "<Left>" => return Some(Key::Left),
            "<Right>" => return Some(Key::Right),
            "<Home>" => return Some(Key::Home),
            "<End>" => return Some(Key::End),
            "<Del>" => return Some(Key::Delete),
            _ => {}
        }
        if let Some(rest) = s.strip_prefix('^') {
            let chars: Vec<char> = rest.chars().collect();
            if chars.len() == 1 {
                let c = chars[0].to_ascii_lowercase();
                return Some(match c {
                    '[' => Key::Esc,
                    'm' | 'j' => Key::Enter,
                    'i' => Key::Tab,
                    '?' => Key::Backspace,
                    other => Key::Ctrl(other),
                });
            }
        }
        None
    }

    /// 反序列化为 readline 风格字符串（用于 `bindkey` 展示）。
    pub fn to_readline_notation(self) -> String {
        match self {
            Key::Char(c) => c.to_string(),
            Key::Ctrl('[') => "^[".into(),
            Key::Ctrl(c) => format!("^{}", c.to_ascii_uppercase()),
            Key::Meta(c) => format!("^{}", c),
            Key::Enter => "^M".into(),
            Key::Tab => "^I".into(),
            Key::Esc => "^[".into(),
            Key::Backspace => "^?".into(),
            Key::Delete => "<Del>".into(),
            Key::Up => "<Up>".into(),
            Key::Down => "<Down>".into(),
            Key::Left => "<Left>".into(),
            Key::Right => "<Right>".into(),
            Key::Home => "<Home>".into(),
            Key::End => "<End>".into(),
        }
    }
}

/// 键位模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapMode {
    /// Emacs 模式（默认）。
    Emacs,
    /// Vi 插入模式。
    ViInsert,
    /// Vi 命令模式。
    ViCommand,
}

/// widget 执行后的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetStatus {
    /// 继续编辑。
    Continue,
    /// 行被接受（回车执行），调用方应读取 [`ZleEngine::accepted`]。
    Accept,
    /// 退出编辑器（如 Ctrl-D 空行）。
    Quit,
    /// 该按键无效果（可触发提示音）。
    Ignored,
}

/// 一个 zle widget（函数指针；无捕获，便于存储）。
pub type Widget = fn(&mut ZleEngine) -> WidgetStatus;

/// 一次编辑前的快照，用于撤销。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    buffer: Vec<char>,
    cursor: usize,
}

/// ZLE 引擎。
#[derive(Debug)]
pub struct ZleEngine {
    buffer: Vec<char>,
    cursor: usize,
    mode: KeymapMode,
    /// 当前模式下的键绑定（按键 → widget 名）。
    keymap: HashMap<Key, String>,
    /// widget 注册表（名字 → 函数）。
    widgets: HashMap<String, Widget>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    undo_limit: usize,
    /// 剪切环（kill-ring）内容。
    kill_ring: Vec<char>,
    /// 历史行，越靠后越新。
    history: Vec<String>,
    /// 历史搜索游标（增量搜索位置）。
    search_from: Option<usize>,
    /// 增量搜索的查询前缀（与 buffer 分离：命中替换 buffer 后，重复按键仍按
    /// 原始查询继续搜索；任何编辑都会清除它）。
    history_query: Option<String>,
    /// 最近一次被接受的命令。
    accepted: Option<String>,
    /// POSTDISPLAY：光标后的灰色建议（自动补全）。
    suggestion: Option<String>,
    /// RPROMPT 字符串（右侧提示，交由渲染层显示）。
    rprompt: String,
    /// PROMPT 字符串（左侧提示）。
    prompt: String,
    /// 当前正在处理的按键（供 `self-insert` 等无参 widget 读取）。
    pending_key: Option<Key>,
}

impl Default for ZleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ZleEngine {
    /// 创建 Emacs 模式引擎。
    pub fn new() -> Self {
        let mut e = Self {
            buffer: Vec::new(),
            cursor: 0,
            mode: KeymapMode::Emacs,
            keymap: HashMap::new(),
            widgets: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_limit: DEFAULT_UNDO_LIMIT,
            kill_ring: Vec::new(),
            history: Vec::new(),
            search_from: None,
            history_query: None,
            accepted: None,
            suggestion: None,
            rprompt: String::new(),
            prompt: String::new(),
            pending_key: None,
        };
        for (name, w) in BUILTIN_WIDGETS {
            e.widgets.insert((*name).to_string(), *w);
        }
        e.load_default_keymap(KeymapMode::Emacs);
        e
    }

    // ---------------- 基础访问器 ----------------

    /// 当前 buffer 内容。
    pub fn buffer(&self) -> String {
        self.buffer.iter().collect()
    }

    /// 光标位置（字符下标，`0..=len`）。
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// 当前键位模式。
    pub fn mode(&self) -> KeymapMode {
        self.mode
    }

    /// 设置提示符（由渲染层显示）。
    pub fn set_prompt(&mut self, p: impl Into<String>) {
        self.prompt = p.into();
    }

    /// 左侧提示符。
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// 设置 RPROMPT（右侧提示）。
    pub fn set_rprompt(&mut self, p: impl Into<String>) {
        self.rprompt = p.into();
    }

    /// RPROMPT 内容。
    pub fn rprompt(&self) -> &str {
        &self.rprompt
    }

    /// 设置 POSTDISPLAY 建议（自动补全）。任何编辑都会清空它。
    pub fn set_suggestion(&mut self, s: Option<String>) {
        self.suggestion = s;
    }

    /// 当前建议。
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }

    /// 最近一次被接受（回车）的命令行。
    pub fn accepted(&self) -> Option<&str> {
        self.accepted.as_deref()
    }

    /// 只读历史（越靠后越新）。
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// 从外部导入历史（如启动时从历史文件加载）。
    pub fn set_history<I>(&mut self, lines: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.history = lines.into_iter().collect();
    }

    /// 当前 mode 下的键绑定表快照（用于 `bindkey` 展示）。
    pub fn bindings(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self
            .keymap
            .iter()
            .map(|(k, w)| (k.to_readline_notation(), w.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    // ---------------- 编辑操作（内部原语） ----------------

    fn push_undo(&mut self) {
        let snap = Snapshot {
            buffer: self.buffer.clone(),
            cursor: self.cursor,
        };
        if self.undo_stack.len() >= self.undo_limit {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snap);
        // 任何新编辑都使重做栈失效。
        self.redo_stack.clear();
        self.reset_history_search();
    }

    /// 编辑会中断历史搜索（清除搜索游标与查询前缀）。
    fn reset_history_search(&mut self) {
        self.search_from = None;
        self.history_query = None;
    }

    fn clear_suggestion(&mut self) {
        self.suggestion = None;
    }

    fn insert_char(&mut self, c: char) {
        self.push_undo();
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
        self.clear_suggestion();
    }

    /// 批量插入一串字符（粘贴/补全接受场景）。当前未被调用方使用，
    /// 保留给后续补全接受 widget。
    #[allow(dead_code)]
    fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.push_undo();
        for c in s.chars() {
            self.buffer.insert(self.cursor, c);
            self.cursor += 1;
        }
        self.clear_suggestion();
    }

    fn delete_before(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.push_undo();
        self.cursor -= 1;
        self.buffer.remove(self.cursor);
        self.clear_suggestion();
    }

    fn delete_at_cursor(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.push_undo();
        self.buffer.remove(self.cursor);
        self.clear_suggestion();
    }

    fn kill_to_end(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.push_undo();
        self.kill_ring = self.buffer.split_off(self.cursor);
        self.clear_suggestion();
    }

    fn kill_to_start(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.push_undo();
        let removed: Vec<char> = self.buffer.drain(..self.cursor).collect();
        self.kill_ring = removed;
        self.cursor = 0;
        self.clear_suggestion();
    }

    fn kill_word_before(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self.buffer[..self.cursor]
            .iter()
            .rposition(|c| !is_word_char(*c))
            .map(|i| i + 1)
            .unwrap_or(0);
        self.push_undo();
        let removed: Vec<char> = self.buffer.drain(start..self.cursor).collect();
        self.kill_ring = removed;
        self.cursor = start;
        self.clear_suggestion();
    }

    fn move_word_back(&mut self) {
        while self.cursor > 0 && !is_word_char(self.buffer[self.cursor - 1]) {
            self.cursor -= 1;
        }
        while self.cursor > 0 && is_word_char(self.buffer[self.cursor - 1]) {
            self.cursor -= 1;
        }
    }

    fn move_word_forward(&mut self) {
        while self.cursor < self.buffer.len() && is_word_char(self.buffer[self.cursor]) {
            self.cursor += 1;
        }
        while self.cursor < self.buffer.len() && !is_word_char(self.buffer[self.cursor]) {
            self.cursor += 1;
        }
    }

    // ---------------- widget 注册 / 键绑定 ----------------

    /// 注册（或覆盖）widget，对应 `zle -N <name> <widget>`。
    pub fn register_widget(&mut self, name: impl Into<String>, widget: Widget) {
        self.widgets.insert(name.into(), widget);
    }

    /// `bindkey <key> <widget-name>`：把按键绑定到当前模式的 widget。
    pub fn bind_key(&mut self, key: Key, widget: impl Into<String>) {
        self.keymap.insert(key, widget.into());
    }

    /// 按 readline 记法绑定：`bind_readline_notation("^R", "history-search-backward")`。
    /// 解析失败返回 `false`。
    pub fn bind_readline_notation(&mut self, notation: &str, widget: &str) -> bool {
        match Key::from_readline_notation(notation) {
            Some(k) => {
                self.bind_key(k, widget);
                true
            }
            None => false,
        }
    }

    /// 清空当前模式的绑定并载入该模式的默认键位表。
    fn load_default_keymap(&mut self, mode: KeymapMode) {
        use crate::zle::widget_names::*;
        self.keymap.clear();
        match mode {
            KeymapMode::Emacs | KeymapMode::ViInsert => {
                self.keymap.insert(Key::Ctrl('a'), BEGINNING_OF_LINE.into());
                self.keymap.insert(Key::Ctrl('e'), END_OF_LINE.into());
                self.keymap.insert(Key::Ctrl('b'), BACKWARD_CHAR.into());
                self.keymap.insert(Key::Ctrl('f'), FORWARD_CHAR.into());
                self.keymap.insert(Key::Ctrl('d'), DELETE_CHAR.into());
                self.keymap.insert(Key::Ctrl('u'), UNIX_LINE_DISCARD.into());
                self.keymap.insert(Key::Ctrl('k'), KILL_LINE.into());
                self.keymap
                    .insert(Key::Ctrl('w'), "backward-kill-word".into());
                self.keymap
                    .insert(Key::Ctrl('r'), HISTORY_SEARCH_BACKWARD.into());
                self.keymap
                    .insert(Key::Ctrl('s'), HISTORY_SEARCH_FORWARD.into());
                self.keymap.insert(Key::Ctrl('_'), UNDO.into());
                self.keymap.insert(Key::Left, BACKWARD_CHAR.into());
                self.keymap.insert(Key::Right, FORWARD_CHAR.into());
                self.keymap.insert(Key::Home, BEGINNING_OF_LINE.into());
                self.keymap.insert(Key::End, END_OF_LINE.into());
                self.keymap
                    .insert(Key::Backspace, BACKWARD_DELETE_CHAR.into());
                self.keymap.insert(Key::Delete, DELETE_CHAR.into());
                self.keymap.insert(Key::Enter, ACCEPT_LINE.into());
                self.keymap.insert(Key::Up, HISTORY_SEARCH_BACKWARD.into());
                self.keymap.insert(Key::Down, HISTORY_SEARCH_FORWARD.into());
                if mode == KeymapMode::ViInsert {
                    self.keymap.insert(Key::Esc, VI_CMD_MODE.into());
                }
            }
            KeymapMode::ViCommand => {
                self.keymap
                    .insert(Key::Char('h'), "vi-backward-char".into());
                self.keymap.insert(Key::Char('l'), "vi-forward-char".into());
                self.keymap.insert(Key::Char('^'), BEGINNING_OF_LINE.into());
                self.keymap.insert(Key::Char('$'), END_OF_LINE.into());
                self.keymap.insert(Key::Char('i'), VI_INSERT_MODE.into());
                self.keymap.insert(Key::Char('a'), "vi-append".into());
                self.keymap.insert(Key::Char('A'), "vi-append-eol".into());
                self.keymap.insert(Key::Char('x'), DELETE_CHAR.into());
                self.keymap.insert(Key::Char('u'), UNDO.into());
                self.keymap
                    .insert(Key::Char('b'), "vi-backward-word".into());
                self.keymap.insert(Key::Char('w'), "vi-forward-word".into());
                self.keymap.insert(Key::Left, BACKWARD_CHAR.into());
                self.keymap.insert(Key::Right, FORWARD_CHAR.into());
            }
        }
    }

    /// 切换键位模式（会重载对应默认键位表）。
    pub fn set_mode(&mut self, mode: KeymapMode) {
        self.mode = mode;
        self.load_default_keymap(mode);
        if mode == KeymapMode::ViCommand
            && self.cursor == self.buffer.len()
            && !self.buffer.is_empty()
        {
            self.cursor -= 1;
        }
    }

    /// 执行已注册 widget 并返回状态。
    pub fn run_widget(&mut self, name: &str) -> WidgetStatus {
        match self.widgets.get(name).copied() {
            Some(w) => w(self),
            None => WidgetStatus::Ignored,
        }
    }

    /// 处理一个按键（终端适配层每次输入调用一次）。
    pub fn handle_key(&mut self, key: Key) -> WidgetStatus {
        self.pending_key = Some(key);
        if let Some(widget) = self.keymap.get(&key).cloned() {
            if let Some(w) = self.widgets.get(&widget).copied() {
                return w(self);
            }
        }
        self.default_action(key)
    }

    fn default_action(&mut self, key: Key) -> WidgetStatus {
        match (self.mode, key) {
            (KeymapMode::ViCommand, _) => WidgetStatus::Ignored,
            (_, Key::Char(c)) => {
                self.insert_char(c);
                WidgetStatus::Continue
            }
            (_, Key::Enter) => self.run_widget("accept-line"),
            (_, Key::Backspace) => self.run_widget("backward-delete-char"),
            (_, Key::Delete) => self.run_widget("delete-char"),
            (_, Key::Tab) => WidgetStatus::Ignored, // 补全 widget 在后续阶段接入
            (_, Key::Esc) if self.mode != KeymapMode::ViCommand => {
                self.set_mode(KeymapMode::ViCommand);
                WidgetStatus::Continue
            }
            (_, Key::Left) => self.run_widget("backward-char"),
            (_, Key::Right) => self.run_widget("forward-char"),
            (_, Key::Home) => self.run_widget("beginning-of-line"),
            (_, Key::End) => self.run_widget("end-of-line"),
            (_, Key::Up) => self.run_widget("history-search-backward"),
            (_, Key::Down) => self.run_widget("history-search-forward"),
            _ => WidgetStatus::Ignored,
        }
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// ---------------- 内置 widgets ----------------

fn w_self_insert(e: &mut ZleEngine) -> WidgetStatus {
    if let Some(Key::Char(c)) = e.pending_key {
        e.insert_char(c);
        WidgetStatus::Continue
    } else {
        WidgetStatus::Ignored
    }
}

fn w_accept_line(e: &mut ZleEngine) -> WidgetStatus {
    let line: String = e.buffer.iter().collect();
    if !line.is_empty() && e.history.last().map(String::as_str) != Some(line.as_str()) {
        e.history.push(line.clone());
    }
    e.accepted = Some(line);
    e.buffer.clear();
    e.cursor = 0;
    e.clear_suggestion();
    e.reset_history_search();
    WidgetStatus::Accept
}

fn w_backward_char(e: &mut ZleEngine) -> WidgetStatus {
    e.cursor = e.cursor.saturating_sub(1);
    WidgetStatus::Continue
}

fn w_forward_char(e: &mut ZleEngine) -> WidgetStatus {
    if e.cursor < e.buffer.len() {
        e.cursor += 1;
    }
    WidgetStatus::Continue
}

fn w_beginning_of_line(e: &mut ZleEngine) -> WidgetStatus {
    e.cursor = 0;
    WidgetStatus::Continue
}

fn w_end_of_line(e: &mut ZleEngine) -> WidgetStatus {
    e.cursor = e.buffer.len();
    WidgetStatus::Continue
}

fn w_backward_delete_char(e: &mut ZleEngine) -> WidgetStatus {
    e.delete_before();
    WidgetStatus::Continue
}

fn w_delete_char(e: &mut ZleEngine) -> WidgetStatus {
    e.delete_at_cursor();
    WidgetStatus::Continue
}

fn w_kill_line(e: &mut ZleEngine) -> WidgetStatus {
    e.kill_to_end();
    WidgetStatus::Continue
}

fn w_unix_line_discard(e: &mut ZleEngine) -> WidgetStatus {
    e.kill_to_start();
    WidgetStatus::Continue
}

fn w_backward_kill_word(e: &mut ZleEngine) -> WidgetStatus {
    e.kill_word_before();
    WidgetStatus::Continue
}

fn w_undo(e: &mut ZleEngine) -> WidgetStatus {
    if let Some(snap) = e.undo_stack.pop() {
        e.redo_stack.push(Snapshot {
            buffer: e.buffer.clone(),
            cursor: e.cursor,
        });
        e.buffer = snap.buffer;
        e.cursor = snap.cursor;
        e.clear_suggestion();
    }
    WidgetStatus::Continue
}

fn w_redo(e: &mut ZleEngine) -> WidgetStatus {
    if let Some(snap) = e.redo_stack.pop() {
        e.push_undo();
        e.buffer = snap.buffer;
        e.cursor = snap.cursor;
    }
    WidgetStatus::Continue
}

fn history_search(e: &mut ZleEngine, forward: bool) -> WidgetStatus {
    // 查询前缀与 buffer 分离：首次按键时记录；命中会替换 buffer，再次按键时
    // 仍沿用首次的查询继续向下/向上寻找。
    let prefix = match &e.history_query {
        Some(q) => q.clone(),
        None => {
            let q: String = e.buffer.iter().collect();
            e.history_query = Some(q.clone());
            q
        }
    };
    if e.history.is_empty() {
        return WidgetStatus::Ignored;
    }
    // 从 search_from 向相应方向搜索；首次从末尾（最新）开始。
    let mut i: isize = match e.search_from {
        Some(pos) => {
            if forward {
                (pos + 1).min(e.history.len().saturating_sub(1)) as isize
            } else {
                (pos as isize) - 1
            }
        }
        None => {
            if forward {
                -1
            } else {
                e.history.len() as isize - 1
            }
        }
    };
    while i >= 0 && (i as usize) < e.history.len() {
        let line = &e.history[i as usize];
        if line.starts_with(&prefix) {
            e.search_from = Some(i as usize);
            e.buffer = line.chars().collect();
            e.cursor = e.buffer.len();
            e.clear_suggestion();
            return WidgetStatus::Continue;
        }
        i += if forward { 1 } else { -1 };
    }
    WidgetStatus::Ignored
}

fn w_history_search_backward(e: &mut ZleEngine) -> WidgetStatus {
    history_search(e, false)
}

fn w_history_search_forward(e: &mut ZleEngine) -> WidgetStatus {
    history_search(e, true)
}

fn w_vi_cmd_mode(e: &mut ZleEngine) -> WidgetStatus {
    e.set_mode(KeymapMode::ViCommand);
    WidgetStatus::Continue
}

fn w_vi_insert_mode(e: &mut ZleEngine) -> WidgetStatus {
    e.set_mode(KeymapMode::ViInsert);
    WidgetStatus::Continue
}

fn w_vi_append(e: &mut ZleEngine) -> WidgetStatus {
    if e.cursor < e.buffer.len() {
        e.cursor += 1;
    }
    e.set_mode(KeymapMode::ViInsert);
    WidgetStatus::Continue
}

fn w_vi_append_eol(e: &mut ZleEngine) -> WidgetStatus {
    e.cursor = e.buffer.len();
    e.set_mode(KeymapMode::ViInsert);
    WidgetStatus::Continue
}

fn w_vi_backward_word(e: &mut ZleEngine) -> WidgetStatus {
    e.move_word_back();
    WidgetStatus::Continue
}

fn w_vi_forward_word(e: &mut ZleEngine) -> WidgetStatus {
    e.move_word_forward();
    WidgetStatus::Continue
}

fn w_vi_backward_char(e: &mut ZleEngine) -> WidgetStatus {
    e.cursor = e.cursor.saturating_sub(1);
    WidgetStatus::Continue
}

fn w_vi_forward_char(e: &mut ZleEngine) -> WidgetStatus {
    if e.cursor < e.buffer.len().saturating_sub(1) {
        e.cursor += 1;
    }
    WidgetStatus::Continue
}

/// 内置 widget 表。名字与 `widget_names` 模块及 zsh `zle -N` 注册名对齐。
static BUILTIN_WIDGETS: &[(&str, Widget)] = &[
    ("self-insert", w_self_insert),
    ("accept-line", w_accept_line),
    ("backward-char", w_backward_char),
    ("forward-char", w_forward_char),
    ("beginning-of-line", w_beginning_of_line),
    ("end-of-line", w_end_of_line),
    ("backward-delete-char", w_backward_delete_char),
    ("delete-char", w_delete_char),
    ("kill-line", w_kill_line),
    ("unix-line-discard", w_unix_line_discard),
    ("backward-kill-word", w_backward_kill_word),
    ("undo", w_undo),
    ("redo", w_redo),
    ("history-search-backward", w_history_search_backward),
    ("history-search-forward", w_history_search_forward),
    ("vi-cmd-mode", w_vi_cmd_mode),
    ("vi-insert-mode", w_vi_insert_mode),
    ("vi-append", w_vi_append),
    ("vi-append-eol", w_vi_append_eol),
    ("vi-backward-word", w_vi_backward_word),
    ("vi-forward-word", w_vi_forward_word),
    ("vi-backward-char", w_vi_backward_char),
    ("vi-forward-char", w_vi_forward_char),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(e: &mut ZleEngine, s: &str) {
        for c in s.chars() {
            assert_eq!(e.handle_key(Key::Char(c)), WidgetStatus::Continue);
        }
    }

    #[test]
    fn insert_and_move_cursor() {
        let mut e = ZleEngine::new();
        type_str(&mut e, "abc");
        assert_eq!(e.buffer(), "abc");
        assert_eq!(e.cursor(), 3);
        // 光标左移 → 插入到中间
        e.handle_key(Key::Left);
        e.handle_key(Key::Left);
        assert_eq!(e.cursor(), 1);
        type_str(&mut e, "X");
        assert_eq!(e.buffer(), "aXbc");
        // Emacs 移动：Ctrl-A / Ctrl-F / Ctrl-E
        e.handle_key(Key::Ctrl('a'));
        assert_eq!(e.cursor(), 0);
        e.handle_key(Key::Ctrl('f'));
        assert_eq!(e.cursor(), 1);
        e.handle_key(Key::Ctrl('e'));
        assert_eq!(e.cursor(), e.buffer().len());
    }

    #[test]
    fn deletion_ops() {
        let mut e = ZleEngine::new();
        type_str(&mut e, "hello");
        e.handle_key(Key::Backspace);
        assert_eq!(e.buffer(), "hell");
        // Ctrl-U 删除光标到行首；光标位于行尾时即清空整行
        e.handle_key(Key::Ctrl('e'));
        e.handle_key(Key::Ctrl('u')); // 整行删除
        assert_eq!(e.buffer(), "");
        type_str(&mut e, "foo bar baz");
        e.handle_key(Key::Ctrl('w')); // 删除光标前一个词
        assert_eq!(e.buffer(), "foo bar ");
        e.handle_key(Key::Ctrl('a'));
        e.handle_key(Key::Ctrl('k')); // 删到行尾
        assert_eq!(e.buffer(), "");
    }

    #[test]
    fn undo_redo() {
        let mut e = ZleEngine::new();
        type_str(&mut e, "ab");
        type_str(&mut e, "cd");
        e.handle_key(Key::Ctrl('_'));
        assert_eq!(e.buffer(), "abc");
        e.handle_key(Key::Ctrl('_'));
        assert_eq!(e.buffer(), "ab");
        e.handle_key(Key::Ctrl('_'));
        assert_eq!(e.buffer(), "a");
        // 引擎没有默认 redo 绑定，通过注册表执行
        e.register_widget("t-redo", w_redo);
        e.bind_key(Key::Meta('r'), "t-redo");
        e.handle_key(Key::Meta('r'));
        assert_eq!(e.buffer(), "ab");
    }

    #[test]
    fn accept_line_pushes_history() {
        let mut e = ZleEngine::new();
        type_str(&mut e, "echo hi");
        assert_eq!(e.handle_key(Key::Enter), WidgetStatus::Accept);
        assert_eq!(e.accepted(), Some("echo hi"));
        assert_eq!(e.buffer(), "");
        assert_eq!(e.history(), &["echo hi".to_string()]);
    }

    #[test]
    fn history_prefix_search() {
        let mut e = ZleEngine::new();
        for line in ["echo one", "echo two", "ls"] {
            type_str(&mut e, line);
            e.handle_key(Key::Enter);
        }
        type_str(&mut e, "echo ");
        // 先按 Ctrl-R：命中较新的 "echo two"；再按一次命中 "echo one"
        assert_eq!(e.handle_key(Key::Ctrl('r')), WidgetStatus::Continue);
        assert_eq!(e.buffer(), "echo two");
        assert_eq!(e.handle_key(Key::Ctrl('r')), WidgetStatus::Continue);
        assert_eq!(e.buffer(), "echo one");
        // Ctrl-S 回到 "echo two"
        assert_eq!(e.handle_key(Key::Ctrl('s')), WidgetStatus::Continue);
        assert_eq!(e.buffer(), "echo two");
    }

    #[test]
    fn vi_modes() {
        let mut e = ZleEngine::new();
        type_str(&mut e, "abc");
        e.handle_key(Key::Esc); // → 命令模式
        assert_eq!(e.mode(), KeymapMode::ViCommand);
        e.handle_key(Key::Char('h'));
        assert_eq!(e.cursor(), 1);
        e.handle_key(Key::Char('x')); // 删除光标处字符
        assert_eq!(e.buffer(), "ac");
        e.handle_key(Key::Char('A')); // 行尾插入
        assert_eq!(e.mode(), KeymapMode::ViInsert);
        assert_eq!(e.cursor(), e.buffer().len());
        type_str(&mut e, "!");
        assert_eq!(e.buffer(), "ac!");
    }

    #[test]
    fn suggestion_lifecycle() {
        let mut e = ZleEngine::new();
        type_str(&mut e, "git st");
        e.set_suggestion(Some("atus".into()));
        assert_eq!(e.suggestion(), Some("atus"));
        type_str(&mut e, "x"); // 编辑清空建议
        assert_eq!(e.suggestion(), None);
    }

    #[test]
    fn key_notation_roundtrip() {
        assert_eq!(Key::from_readline_notation("^R"), Some(Key::Ctrl('r')));
        assert_eq!(Key::from_readline_notation("<Up>"), Some(Key::Up));
        assert_eq!(Key::from_readline_notation("x"), Some(Key::Char('x')));
        assert_eq!(Key::Ctrl('r').to_readline_notation(), "^R");
        assert_eq!(Key::from_readline_notation("not-a-key"), None);
    }
}
