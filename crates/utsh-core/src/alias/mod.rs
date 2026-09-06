//! 别名管理（§4.6 alias 子命令、§4.8 `[aliases]`）。
//!
//! 持久化由 [`crate::config::Config`] 负责（写回 `utsh.toml`）；本模块提供：
//! - 名字合法性校验（与配置层一致）；
//! - 命令行展开：把首词为别名的行替换为别名值；
//! - 循环引用保护（a→b→a 时按固定深度截断）。
//!
//! 展开发生在解析之前（与 bash/zsh 一致：别名在词法展开阶段生效）。

use std::collections::BTreeMap;

/// 别名展开最大深度（防止 a→b→a 死循环）。
pub const MAX_EXPANSION_DEPTH: usize = 10;

/// 名字是否合法：非空、由字母/数字/`_`/`-`/`.` 组成。
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// 提取一行命令的首词（命令名）。不做引号解析——别名展开阶段的近似规则。
pub fn first_word(line: &str) -> &str {
    let trimmed = line.trim_start();
    let end = trimmed
        .find(|c: char| c.is_whitespace())
        .unwrap_or(trimmed.len());
    &trimmed[..end]
}

/// 别名表（有序，便于稳定输出与序列化）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasTable(pub BTreeMap<String, String>);

impl AliasTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从键值对构建。
    pub fn from_entries<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self(iter.into_iter().collect())
    }

    /// 新增/覆盖别名。
    pub fn insert(&mut self, name: String, command: String) -> Result<(), String> {
        if !is_valid_name(&name) {
            return Err(format!("invalid alias name: {name:?}"));
        }
        if command.trim().is_empty() || command.contains('\n') {
            return Err(format!("invalid alias command for {name:?}"));
        }
        self.0.insert(name, command);
        Ok(())
    }

    /// 删除别名，返回是否存在。
    pub fn remove(&mut self, name: &str) -> bool {
        self.0.remove(name).is_some()
    }

    /// 查询。
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    /// 别名数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 只读底层表。
    pub fn inner(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    /// 展开一行命令中的首词别名（含循环保护）。
    pub fn expand(&self, line: &str) -> String {
        let mut current = line.trim_start().to_string();
        for _ in 0..MAX_EXPANSION_DEPTH {
            let trimmed = current.trim_start();
            let word = first_word(trimmed);
            match self.get(word) {
                Some(replacement) => {
                    let rest = trimmed[word.len()..].trim_start();
                    let mut next = String::with_capacity(replacement.len() + rest.len() + 1);
                    next.push_str(replacement);
                    if !rest.is_empty() {
                        next.push(' ');
                        next.push_str(rest);
                    }
                    if next == current {
                        break;
                    }
                    current = next;
                }
                None => break,
            }
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_expansion() {
        let mut t = AliasTable::new();
        t.insert("ll".into(), "ls -alF".into()).unwrap();
        t.insert("gst".into(), "git status".into()).unwrap();
        assert_eq!(t.expand("ll /tmp"), "ls -alF /tmp");
        assert_eq!(t.expand("gst"), "git status");
        assert_eq!(t.expand("ls /tmp"), "ls /tmp"); // 非别名不动
        assert_eq!(t.expand("   ll /tmp"), "ls -alF /tmp"); // 行首空白安全
    }

    #[test]
    fn recursive_expansion_is_bounded() {
        let mut t = AliasTable::new();
        t.insert("a".into(), "b".into()).unwrap();
        t.insert("b".into(), "a".into()).unwrap();
        // 不会死循环
        let _ = t.expand("a x");
    }

    #[test]
    fn name_validation() {
        let mut t = AliasTable::new();
        assert!(t.insert("has space".into(), "ls".into()).is_err());
        assert!(t.insert("".into(), "ls".into()).is_err());
        assert!(t.insert("x".into(), "".into()).is_err());
        assert!(t.insert("ok-name.1".into(), "ls".into()).is_ok());
        assert!(t.remove("ok-name.1"));
    }
}
