# 内置默认插件

UTSH 默认携带两个 Zsh 生态插件（语法高亮 + 自动补全），以 Git submodule 的形式
挂载在本目录下，并在首次 `make setup`（或手动执行下方命令）时拉取：

```bash
git submodule add https://github.com/zsh-users/zsh-syntax-highlighting plugins/zsh-syntax-highlighting
git submodule add https://github.com/zsh-users/zsh-autosuggestions     plugins/zsh-autosuggestions
```

说明：

- 插件市场/`utsh plugin install` 安装的第三方插件统一放在
  `~/.local/share/utsh/plugins/`，不会写进本目录。
- 这两个插件是 UTSH **内置**插件：当 `~/.config/ut/utsh.toml` 中
  `[plugins] enable_defaults = true` 时自动加载，并接入 UTSH 的 ZLE 模拟层
  （见 `crates/utsh-core/src/zle`），而非直接 source 原生 .zsh 文件。
- 用户**不会**在 WebUI / 配置里看到需要“安装”它们——它们是开箱即用的，
  状态在 `utsh status` 中以“内置（defaults）”列出。
