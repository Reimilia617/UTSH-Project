//! 历史记录管理（§4.8 `[general] history_size`、§4.3 历史搜索）。
//!
//! 存储层：每个会话追加到 `~/.local/share/utsh/history`（也可自定义路径）。
//! 内存层：供 ZLE 的 `history-search-backward` 使用（见 `zle::engine`）。

use std::io::Write;
use std::path::{Path, PathBuf};

/// 历史模块错误。
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    /// I/O 错误。
    #[error("history io error: {0}")]
    Io(#[from] std::io::Error),
}

/// 历史管理器。
#[derive(Debug, Clone)]
pub struct History {
    /// 历史行，越靠后越新。
    entries: Vec<String>,
    /// 最大条数（超过时丢弃最旧）。
    max: usize,
    /// 忽略相邻重复。
    ignore_dups: bool,
    /// 持久化文件（可选，未设置则仅内存）。
    file: Option<PathBuf>,
}

impl History {
    /// 新建仅内存历史。
    pub fn new(max: usize, ignore_dups: bool) -> Self {
        Self {
            entries: Vec::new(),
            max,
            ignore_dups,
            file: None,
        }
    }

    /// 从文件加载历史（每行一条）。文件不存在时视为空历史。
    pub fn load<P: AsRef<Path>>(
        path: P,
        max: usize,
        ignore_dups: bool,
    ) -> Result<Self, HistoryError> {
        let path = path.as_ref();
        let mut h = Self::new(max, ignore_dups);
        match std::fs::read_to_string(path) {
            Ok(content) => {
                for line in content.lines() {
                    if !line.trim().is_empty() {
                        h.push(line.to_string());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        h.file = Some(path.to_path_buf());
        Ok(h)
    }

    /// 只读历史。
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// 条数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 追加一条；应用去重与上限截断，返回是否真正加入。
    pub fn push(&mut self, line: String) -> bool {
        let line = line.trim_end_matches('\n').to_string();
        if line.trim().is_empty() {
            return false;
        }
        if self.ignore_dups && self.entries.last().map(String::as_str) == Some(line.as_str()) {
            return false;
        }
        self.entries.push(line);
        if self.entries.len() > self.max {
            let excess = self.entries.len() - self.max;
            self.entries.drain(..excess);
        }
        true
    }

    /// 删除指定行（按内容，从新到旧删除首个匹配）。
    pub fn remove(&mut self, line: &str) -> bool {
        if let Some(pos) = self.entries.iter().rposition(|e| e == line) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 持久化到 `file`（若已设置）或指定路径。返回是否写盘。
    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<bool, HistoryError> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut f = std::fs::File::create(path)?;
        for line in &self.entries {
            writeln!(f, "{line}")?;
        }
        Ok(true)
    }

    /// 持久化到构造函数/加载时使用的文件。
    pub fn save(&self) -> Result<bool, HistoryError> {
        match &self.file {
            Some(p) => self.save_to(p),
            None => Ok(false),
        }
    }

    /// 从 `from`（不含）向旧方向找以 `prefix` 开头的行，返回其下标。
    pub fn search_backward(&self, prefix: &str, from: Option<usize>) -> Option<usize> {
        let start = from.unwrap_or(self.entries.len());
        (0..start)
            .rev()
            .find(|&i| self.entries[i].starts_with(prefix))
    }

    /// 从 `from`（不含）向新方向找以 `prefix` 开头的行，返回其下标。
    pub fn search_forward(&self, prefix: &str, from: Option<usize>) -> Option<usize> {
        let start = from.unwrap_or(0);
        ((start + 1)..self.entries.len()).find(|&i| self.entries[i].starts_with(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_and_dups() {
        let mut h = History::new(3, true);
        assert!(h.push("a".into()));
        assert!(h.push("b".into()));
        assert!(!h.push("b".into()), "consecutive dup ignored");
        assert!(h.push("c".into()));
        assert!(h.push("d".into()));
        assert_eq!(h.entries(), &["b", "c", "d"]);
        assert!(h.remove("c"));
        assert_eq!(h.entries(), &["b", "d"]);
    }

    #[test]
    fn search_directions() {
        let mut h = History::new(100, false);
        for l in ["echo one", "ls", "echo two"] {
            h.push(l.into());
        }
        assert_eq!(h.search_backward("echo", None), Some(2));
        assert_eq!(h.search_backward("echo", Some(2)), Some(0));
        assert_eq!(h.search_forward("echo", Some(0)), Some(2));
        assert_eq!(h.search_backward("zzz", None), None);
    }

    #[test]
    fn file_roundtrip() {
        let dir = std::env::temp_dir().join(format!("utsh-hist-test-{}", std::process::id()));
        let path = dir.join("history");
        let mut h = History::new(100, true);
        h.push("first".into());
        h.push("second".into());
        h.save_to(&path).unwrap();
        let loaded = History::load(&path, 100, true).unwrap();
        assert_eq!(loaded.entries(), &["first", "second"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
