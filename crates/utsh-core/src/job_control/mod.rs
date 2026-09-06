//! 作业控制（§4.1）：作业表、前后台状态与回收。
//!
//! 交互式 shell 需要跟踪每个“作业”（一行命令行对应的进程/进程组）。本模块是
//! 纯 Rust 的作业簿记层：**只跟踪状态，不直接 fork**。执行引擎（后续 Phase 1）
//! 用 `std::os::unix::process::CommandExt::process_group` 创建进程组，再把
//! `std::process::Child` 登记进来。
//!
//! 真正的信号分发（SIGTSTP/SIGCONT/SIGCHLD 处理、`fg`/`bg` 内建）属于交互式
//! 会话层，将在 Phase 1 与执行引擎一并落地；本模块先把数据结构与轮询逻辑固定。

use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};

/// 作业状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// 前台运行。
    Foreground,
    /// 后台运行。
    Background,
    /// 停止（收到 SIGTSTP）。
    Stopped,
    /// 已结束（尚未回收退出码）。
    Done,
}

impl JobState {
    /// 是否仍在运行。
    pub fn is_running(self) -> bool {
        matches!(self, Self::Foreground | Self::Background)
    }
}

/// 一个作业。
#[derive(Debug)]
pub struct Job {
    /// shell 内作业号（从 1 开始）。
    pub id: u32,
    /// 进程组 id（= 组长 pid）。
    pub pgid: u32,
    /// 用户可见的命令行。
    pub command: String,
    /// 状态。
    pub state: JobState,
    /// 退出码（`Done` 后可用）。
    pub exit_code: Option<i32>,
    /// 底层的子进程句柄。
    pub child: Option<Child>,
}

impl Job {
    fn new(id: u32, pgid: u32, command: String, child: Child) -> Self {
        Self {
            id,
            pgid,
            command,
            state: JobState::Background,
            exit_code: None,
            child: Some(child),
        }
    }
}

/// 作业表。
#[derive(Debug, Default)]
pub struct JobControl {
    jobs: BTreeMap<u32, Job>,
    next_id: u32,
}

impl JobControl {
    /// 新建空的作业表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前作业数。
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// 只读访问作业表。
    pub fn jobs(&self) -> &BTreeMap<u32, Job> {
        &self.jobs
    }

    /// 按 id 查询作业。
    pub fn get(&self, id: u32) -> Option<&Job> {
        self.jobs.get(&id)
    }

    /// 登记一个外部命令为后台作业。
    ///
    /// `shell_out: true` 时经 `/bin/sh -c` 执行（scaffold 阶段占位，便于作业表
    /// 有真实的 `Child` 可轮询）；后续阶段替换为执行引擎的直接 spawn。
    pub fn spawn_external(&mut self, command: &str, shell_out: bool) -> std::io::Result<u32> {
        let child = if shell_out {
            Command::new("/bin/sh")
                .arg("-c")
                .arg(command)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?
        } else {
            // 拆成 argv 直接执行（scaffold 仍走 /bin/sh，argv 拆分由 parser 负责）。
            let argv: Vec<&str> = command.split_whitespace().collect();
            let mut cmd = Command::new(argv[0]);
            cmd.args(&argv[1..]);
            cmd.spawn()?
        };
        let pgid = child.id();
        self.next_id += 1;
        let id = self.next_id;
        self.jobs
            .insert(id, Job::new(id, pgid, command.to_string(), child));
        Ok(id)
    }

    /// 轮询所有作业，回收已结束进程并更新状态。
    pub fn poll(&mut self) {
        for job in self.jobs.values_mut() {
            if job.state.is_running() {
                if let Some(child) = job.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            job.state = JobState::Done;
                            job.exit_code = status.code();
                            job.child = None;
                        }
                        Ok(None) => {}
                        Err(_) => {
                            job.state = JobState::Done;
                            job.child = None;
                        }
                    }
                }
            }
        }
    }

    /// 把已结束的作业从表中移除，返回它们的退出信息。
    pub fn reap_done(&mut self) -> Vec<(u32, String, Option<i32>)> {
        let done: Vec<u32> = self
            .jobs
            .iter()
            .filter(|(_, j)| j.state == JobState::Done)
            .map(|(id, _)| *id)
            .collect();
        let mut out = Vec::new();
        for id in done {
            if let Some(j) = self.jobs.remove(&id) {
                out.push((j.id, j.command.clone(), j.exit_code));
            }
        }
        out
    }

    /// 移除所有已结束作业（快捷方式）。
    pub fn clean(&mut self) {
        self.poll();
        self.reap_done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_reap() {
        let mut jc = JobControl::new();
        let id = jc.spawn_external("echo ok", true).expect("spawn");
        assert_eq!(jc.len(), 1);
        // 轮询并回收直到作业消失（最多等 2s）。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while jc.get(id).is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            jc.poll();
            jc.reap_done();
        }
        assert!(jc.get(id).is_none(), "done job should be reaped");
        // 再 clean 一次不应出错
        jc.clean();
        assert!(jc.is_empty());
    }
}
