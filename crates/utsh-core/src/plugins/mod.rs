//! 插件系统（§4.4）。
//!
//! 安装来源：官方插件市场（内置列表）/ GitHub 仓库 / 任意 Git URL。
//! 安装位置：`~/.local/share/utsh/plugins/<name>`。
//!
//! 每个插件目录可以携带 `plugin.toml`（或 `utsh-plugin.toml`）元数据；缺失时按
//! 目录名生成默认清单。Zsh 生态插件（`.plugin.zsh`/`.zsh`）由 UTSH 的
//! Zsh 兼容层在“加载”阶段翻译执行，而不是直接 source —— 真正接入在 Phase 3，
//! 本模块先完成**安装/卸载/更新与元数据读取**。
//!
//! 依赖解析与加载排序（§4.4 “插件加载流程”）在后续阶段于本模块之上实现
//! （`resolve_load_order` 的 TODO）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// 插件错误。
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// I/O 错误。
    #[error("plugin io error: {0}")]
    Io(#[from] std::io::Error),
    /// git 执行失败。
    #[error("git error: {0}")]
    Git(String),
    /// 已安装。
    #[error("plugin `{0}` is already installed")]
    AlreadyInstalled(String),
    /// 未安装。
    #[error("plugin `{0}` is not installed")]
    NotFound(String),
    /// 非法插件名。
    #[error("invalid plugin name `{0}`")]
    InvalidName(String),
    /// 无法识别的来源。
    #[error("unknown plugin source `{0}` (expected user/repo, git URL, or official name)")]
    UnknownSource(String),
    /// TOML 解析错误。
    #[error("plugin manifest parse error: {0}")]
    ManifestParse(#[from] toml::de::Error),
    /// TOML 序列化错误。
    #[error("plugin manifest serialize error: {0}")]
    ManifestSerialize(#[from] toml::ser::Error),
}

/// 插件元数据清单（`plugin.toml`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// 插件名。
    pub name: String,
    /// 版本。
    #[serde(default)]
    pub version: String,
    /// 描述。
    #[serde(default)]
    pub description: String,
    /// 源码位置（`user/repo` 或完整 URL）。
    #[serde(default)]
    pub source: String,
    /// 版本锁定的 commit。
    #[serde(default)]
    pub commit: String,
    /// 依赖的插件名。
    #[serde(default)]
    pub depends: Vec<String>,
    /// 初始化脚本（相对插件目录）。默认尝试 `<name>.plugin.zsh` / `<name>.zsh`。
    #[serde(default)]
    pub init_files: Vec<String>,
}

impl PluginManifest {
    /// 从插件目录加载清单；无清单文件时生成默认清单。
    pub fn load_from_dir(dir: &Path) -> Result<Self, PluginError> {
        for f in ["plugin.toml", "utsh-plugin.toml"] {
            let p = dir.join(f);
            if p.is_file() {
                let raw = std::fs::read_to_string(&p)?;
                return Ok(toml::from_str(&raw)?);
            }
        }
        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(Self {
            name,
            ..Self::default()
        })
    }

    /// 尝试写入一份默认清单。
    pub fn save_to_dir(&self, dir: &Path) -> Result<(), PluginError> {
        let p = dir.join("plugin.toml");
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(p, raw)?;
        Ok(())
    }
}

impl Default for PluginManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.0.0".into(),
            description: String::new(),
            source: String::new(),
            commit: String::new(),
            depends: Vec::new(),
            init_files: Vec::new(),
        }
    }
}

/// 一个已安装插件的只读视图。
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// 插件名。
    pub name: String,
    /// 是否启用（由配置决定，见 [`crate::config::Config`]）。
    pub enabled: bool,
    /// 安装目录。
    pub path: PathBuf,
    /// 元数据。
    pub manifest: PluginManifest,
}

/// 官方插件市场条目（初期数据，§8“插件市场数据”）。
#[derive(Debug, Clone, Copy)]
pub struct OfficialPlugin {
    /// 插件名。
    pub name: &'static str,
    /// GitHub 仓库（`owner/repo`）。
    pub repo: &'static str,
    /// 分类：`syntax` / `completion` / `history` / `prompt` / `utility`。
    pub category: &'static str,
    /// 一句话描述。
    pub description: &'static str,
}

/// 官方插件市场（内置列表，可离线浏览）。
pub const OFFICIAL_PLUGINS: &[OfficialPlugin] = &[
    OfficialPlugin {
        name: "zsh-syntax-highlighting",
        repo: "zsh-users/zsh-syntax-highlighting",
        category: "syntax",
        description: "Fish shell 风格的命令语法高亮",
    },
    OfficialPlugin {
        name: "zsh-autosuggestions",
        repo: "zsh-users/zsh-autosuggestions",
        category: "completion",
        description: "根据历史记录给出灰色自动补全建议",
    },
    OfficialPlugin {
        name: "zsh-completions",
        repo: "zsh-users/zsh-completions",
        category: "completion",
        description: "大量额外补全定义（Zsh 官方补充仓库）",
    },
    OfficialPlugin {
        name: "zsh-history-substring-search",
        repo: "zsh-users/zsh-history-substring-search",
        category: "history",
        description: "输入子串即可搜索历史（上下方向键）",
    },
    OfficialPlugin {
        name: "fast-syntax-highlighting",
        repo: "zdharma-continuum/fast-syntax-highlighting",
        category: "syntax",
        description: "高性能语法高亮（F-Sy-H）",
    },
];

/// 根据名字查官方市场。
pub fn official(name: &str) -> Option<&'static OfficialPlugin> {
    OFFICIAL_PLUGINS.iter().find(|p| p.name == name)
}

/// 把 `user/repo` 简写解析为完整 git URL；完整的 http(s)/ssh URL 原样返回。
pub fn resolve_source_url(source: &str) -> String {
    if source.starts_with("https://")
        || source.starts_with("http://")
        || source.starts_with("ssh://")
        || source.starts_with("git@")
    {
        source.to_string()
    } else {
        format!("https://github.com/{source}.git")
    }
}

/// 插件管理器。
pub struct PluginManager {
    /// 插件安装根目录。
    pub plugins_dir: PathBuf,
}

impl PluginManager {
    /// 使用默认目录（`~/.local/share/utsh/plugins`）。
    pub fn default_dir() -> Self {
        Self {
            plugins_dir: Config::plugins_dir(),
        }
    }

    /// 自定义目录。
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
        }
    }

    /// 扫描已安装插件目录。
    pub fn scan(&self) -> Result<Vec<PluginInfo>, PluginError> {
        if !self.plugins_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let manifest = PluginManifest::load_from_dir(&path)?;
            out.push(PluginInfo {
                name,
                enabled: false,
                path,
                manifest,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// 查询单个插件。
    pub fn get(&self, name: &str) -> Result<PluginInfo, PluginError> {
        let path = self.plugins_dir.join(name);
        if !path.is_dir() {
            return Err(PluginError::NotFound(name.into()));
        }
        let manifest = PluginManifest::load_from_dir(&path)?;
        Ok(PluginInfo {
            name: name.into(),
            enabled: false,
            path,
            manifest,
        })
    }

    /// 安装插件。`source` 可为官方名、`user/repo` 或完整 git URL；
    /// `commit` 存在时安装后检出该 commit（版本锁定，§4.4）。
    pub fn install(
        &self,
        name: &str,
        source: &str,
        commit: Option<&str>,
    ) -> Result<PluginManifest, PluginError> {
        if !crate::plugins::is_valid_plugin_name(name) {
            return Err(PluginError::InvalidName(name.into()));
        }
        std::fs::create_dir_all(&self.plugins_dir)?;
        let dest = self.plugins_dir.join(name);
        if dest.exists() {
            return Err(PluginError::AlreadyInstalled(name.into()));
        }
        // 官方名 → 官方 repo；否则按 URL/简写解析。
        let url = match official(source) {
            Some(p) => resolve_source_url(p.repo),
            None => {
                if source.contains('/') || source.contains("://") || source.starts_with("git@") {
                    resolve_source_url(source)
                } else {
                    return Err(PluginError::UnknownSource(source.into()));
                }
            }
        };
        run_git(
            &["clone", "--depth", "1", url.as_str(), name],
            &self.plugins_dir,
        )?;
        if let Some(commit) = commit {
            if !commit.is_empty() {
                let dest_s = dest.to_string_lossy().into_owned();
                run_git(
                    &["-C", dest_s.as_str(), "checkout", commit],
                    &self.plugins_dir,
                )?;
            }
        }
        // 元数据：优先读取仓库自带；否则生成默认清单写入。
        let mut manifest = PluginManifest::load_from_dir(&dest)?;
        if manifest.name.is_empty() {
            manifest.name = name.into();
        }
        manifest.source = url;
        manifest.commit = commit.unwrap_or("").to_string();
        // 仓库自带元数据则不改动；否则写入生成的默认清单。
        if !dest.join("plugin.toml").is_file() {
            let _ = manifest.save_to_dir(&dest);
        }
        Ok(manifest)
    }

    /// 卸载插件（整目录删除）。
    pub fn uninstall(&self, name: &str) -> Result<(), PluginError> {
        let dest = self.plugins_dir.join(name);
        if !dest.exists() {
            return Err(PluginError::NotFound(name.into()));
        }
        std::fs::remove_dir_all(dest)?;
        Ok(())
    }

    /// 更新单个（或全部）插件到远端最新；若清单锁定了 commit 则仍锁住。
    pub fn update(&self, name: Option<&str>) -> Result<Vec<String>, PluginError> {
        let mut updated = Vec::new();
        let targets: Vec<String> = match name {
            Some(n) => vec![n.to_string()],
            None => {
                // 不存在的目录跳过，交由调用方决定是否报错
                let mut all = Vec::new();
                if self.plugins_dir.is_dir() {
                    for entry in std::fs::read_dir(&self.plugins_dir)? {
                        let entry = entry?;
                        if entry.path().is_dir() {
                            all.push(entry.file_name().to_string_lossy().into_owned());
                        }
                    }
                }
                all
            }
        };
        for t in targets {
            let dest = self.plugins_dir.join(&t);
            if !dest.is_dir() {
                return Err(PluginError::NotFound(t));
            }
            let dest_s = dest.to_string_lossy().into_owned();
            run_git(
                &["-C", dest_s.as_str(), "pull", "--ff-only"],
                &self.plugins_dir,
            )?;
            updated.push(t);
        }
        Ok(updated)
    }
}

fn is_valid_plugin_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// 在 `cwd` 下执行 git，失败时返回带 stderr 的错误。
fn run_git(args: &[&str], cwd: &Path) -> Result<(), PluginError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| PluginError::Git(format!("cannot run git: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(PluginError::Git(format!(
            "`git {}` failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }
    Ok(())
}

/// 后续阶段：根据 `depends` 做拓扑排序并输出加载顺序。
#[allow(dead_code)]
pub fn resolve_load_order(_manifests: &BTreeMap<String, PluginManifest>) -> Vec<String> {
    // TODO(Phase 3): Kahn 拓扑排序 + 环检测；当前按名字字典序返回。
    let mut names: Vec<String> = _manifests.keys().cloned().collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_lookup_and_url() {
        assert_eq!(
            official("zsh-autosuggestions").unwrap().repo,
            "zsh-users/zsh-autosuggestions"
        );
        assert!(official("nope").is_none());
        assert_eq!(
            resolve_source_url("zsh-users/zsh-autosuggestions"),
            "https://github.com/zsh-users/zsh-autosuggestions.git"
        );
        assert_eq!(
            resolve_source_url("https://example.com/x.git"),
            "https://example.com/x.git"
        );
    }

    #[test]
    fn name_validation() {
        assert!(is_valid_plugin_name("zsh-autosuggestions"));
        assert!(!is_valid_plugin_name("a b"));
        assert!(!is_valid_plugin_name(""));
        assert!(!is_valid_plugin_name("../evil"));
    }

    #[test]
    fn manifest_default_from_dir() {
        let dir = std::env::temp_dir().join(format!("utsh-plugin-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = PluginManifest::load_from_dir(&dir).unwrap();
        // 无元数据时：版本回落默认值，名字取目录名，来源为空。
        assert_eq!(m.name, dir.file_name().unwrap().to_string_lossy());
        assert_eq!(m.version, "0.0.0");
        assert!(m.source.is_empty());
        assert!(m.commit.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
