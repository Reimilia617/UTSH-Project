//! `preexec` / `precmd` 钩子系统（§4.2）。
//!
//! 语义与 zsh 对齐：
//! - `precmd`：每次显示提示符**之前**执行（可用于更新标题、git 状态等）；
//! - `preexec`：用户按下回车、命令**即将执行之前**执行（收到原始命令行）。
//!
//! 本模块只负责“按注册顺序运行回调”；回调的内容（标题刷新、状态缓存等）由上层
//! 二进制或插件注入。钩子以 `&HookEvent` 形式收到一份只读快照。

/// 传给钩子的事件快照（值语义，便于跨线程/异步）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookEvent {
    /// `preexec`：即将执行的命令行；`precmd` 下为 `None`。
    pub command: Option<String>,
    /// 上一条命令的退出码（`precmd` 时有意义）。
    pub exit_status: Option<i32>,
    /// 上一条命令的执行时长（毫秒，可选）。
    pub duration_ms: Option<u64>,
    /// 当前工作目录。
    pub cwd: Option<String>,
}

/// 钩子回调。
pub type HookFn = Box<dyn Fn(&HookEvent) + Send + Sync>;

/// precmd / preexec 钩子注册表。
#[derive(Default)]
pub struct Hooks {
    preexec: Vec<HookFn>,
    precmd: Vec<HookFn>,
}

impl std::fmt::Debug for Hooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hooks")
            .field("preexec", &self.preexec.len())
            .field("precmd", &self.precmd.len())
            .finish()
    }
}

impl Hooks {
    /// 新建空钩子表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一个 preexec 钩子（命令执行前）。
    pub fn add_preexec<F>(&mut self, f: F)
    where
        F: Fn(&HookEvent) + Send + Sync + 'static,
    {
        self.preexec.push(Box::new(f));
    }

    /// 追加一个 precmd 钩子（提示符显示前）。
    pub fn add_precmd<F>(&mut self, f: F)
    where
        F: Fn(&HookEvent) + Send + Sync + 'static,
    {
        self.precmd.push(Box::new(f));
    }

    /// 触发 preexec。按注册顺序执行所有钩子。
    pub fn run_preexec(&self, command: &str) {
        let ev = HookEvent {
            command: Some(command.to_string()),
            ..Default::default()
        };
        for f in &self.preexec {
            f(&ev);
        }
    }

    /// 触发 precmd（携带上一条命令的退出码）。
    pub fn run_precmd(&self, exit_status: i32) {
        let ev = HookEvent {
            exit_status: Some(exit_status),
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string()),
            ..Default::default()
        };
        for f in &self.precmd {
            f(&ev);
        }
    }

    /// 钩子数量统计。
    pub fn len(&self) -> (usize, usize) {
        (self.preexec.len(), self.precmd.len())
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.preexec.is_empty() && self.precmd.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn hooks_run_in_order() {
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let mut hooks = Hooks::new();
        for _ in 0..3 {
            let calls = calls.clone();
            hooks.add_preexec(move |ev| {
                assert_eq!(ev.command.as_deref(), Some("ls -la"));
                calls.fetch_add(1, Ordering::SeqCst);
            });
        }
        hooks.run_preexec("ls -la");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(hooks.len(), (3, 0));
    }

    #[test]
    fn precmd_receives_exit_code() {
        let got = std::sync::Arc::new(std::sync::Mutex::new(None::<i32>));
        let mut hooks = Hooks::new();
        let got2 = got.clone();
        hooks.add_precmd(move |ev| *got2.lock().unwrap() = ev.exit_status);
        hooks.run_precmd(42);
        assert_eq!(*got.lock().unwrap(), Some(42));
    }
}
