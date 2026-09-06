# UTSH（UT Shell）

> 一个兼容 **Bash 语法**、同时具备 **Zsh 插件与主题生态**兼容能力的交互式 Shell。
> UTSH 是 UT 系列生态（utnixos、utniri 等）的核心组件之一。

## 特性

- **Bash 语法兼容**：目标支持 `[[ ]]`、`{a..b}` 展开、数组语法、`$(...)` 命令替换、
  重定向与管道、`shopt` 选项等（Bash 4.0+ 特性，见 [Roadmap](#roadmap)）。
- **Zsh 插件生态**：能加载 `zsh-syntax-highlighting`、`zsh-autosuggestions` 等主流
  插件，提供 `bindkey` / `zle -N` / `preexec` / `precmd` / `fpath` + `autoload` 兼容层。
- **内置语法高亮与自动补全**：默认集成以上两个插件（`enable_defaults = true`），
  **不安装任何主题**（`theme.current = "none"`，必须显式 `utsh theme set`）。
- **WebUI 可视化**：插件市场 / 主题 / 别名 / 配置管理，安装进度实时推送。
- **CLI 工具 `utsh`**：`plugin` / `theme` / `alias` / `status` / `doctor` / `webui`。

## 架构总览

| 层 | 技术 | 说明 |
| --- | --- | --- |
| Shell 核心 | Rust（`crates/utsh-core`） | 系统交互、进程/信号、配置、Zsh 兼容层、IPC |
| Bash 语法解析 | Rust + C++（`crates/utsh-ffi`） | Rust 主控，C++ 最小解析器 + Bison/Flex 演进框架 |
| ZLE 行编辑器 | Rust 状态机 | buffer/cursor/undo/历史搜索/Emacs+Vi 模式 |
| 终端 I/O | C++（GNU Readline，可选 feature） | `--features readline` |
| 配置 | TOML | `~/.config/ut/utsh.toml`（<50ms 启动预算） |
| CLI | Rust + clap（`crates/utsh-cli`） | `utsh` 命令 |
| WebUI 后端 | Node.js Fastify（`webui/backend`） | REST + SSE + WebSocket |
| Rust ↔ Node | Unix Domain Socket | JSON-RPC 2.0，`/tmp/utsh.sock` |

```
utsh/
├── crates/
│   ├── utsh-core/   # 核心库：parser / zle / hooks / job_control / plugins /
│   │                # config / alias / history / ipc
│   ├── utsh-cli/    # `utsh` 命令（clap）
│   └── utsh-ffi/    # C++ FFI：bash_parser + readline + build.rs(bindgen)
├── webui/backend/   # Fastify 后端（IPC 客户端、REST 路由、SSE/WS 进度）
├── plugins/         # 内置插件（git submodule，见 plugins/README.md）
├── docs/            # 文档（API 等）
├── scripts/         # build.sh / gen-bindings.sh
└── tests/           # 集成测试（计划）
```

## 快速开始

前置依赖：

- Rust **stable**（见 `rust-toolchain.toml`）+ C++ 编译器（`g++`/`clang++`）
- Node.js ≥ 18（仅 WebUI）+ `pnpm` 或 `npm`
- git ≥ 2.0（插件安装）
- （可选）GNU Readline —— ZLE 终端层；libclang —— 重新生成 FFI 绑定

构建与运行：

```bash
make build                      # cargo build --workspace（含 C++ 解析器 FFI）
make build-release              # release（启动性能优先）

# 查看状态 / 诊断
cargo run -p utsh-cli -- status
cargo run -p utsh-cli -- doctor

# 别名与主题
cargo run -p utsh-cli -- alias list
cargo run -p utsh-cli -- alias add gst "git status"
cargo run -p utsh-cli -- theme list

# WebUI（首次会生成 ~/.config/ut/utsh.toml 与随机 Token）
make webui-install
cargo run -p utsh-cli -- webui     # 自动打开浏览器
```

配置：`~/.config/ut/utsh.toml`（结构与示例见 [docs/api.md](docs/api.md#配置格式)）。

## Roadmap

1. **Phase 1** 核心引擎：Rust workspace + CMake、Bash 解析器（Bison+Flex+FFI）、
   命令执行、作业控制、配置加载、重定向/管道。当前状态：**脚手架就绪**（配置、
   FFI 通道、作业表框架完成）。
2. **Phase 2** Zsh 兼容层 + ZLE：ZleEngine 状态机、`zle -N`、`bindkey`、
   `preexec/precmd`、`fpath`+`autoload`、`POSTDISPLAY`/`RPROMPT`。当前状态：
   ZLE 状态机核心已实现（Emacs/Vi、undo/redo、历史搜索、建议）。
3. **Phase 3** 插件系统 + 内置插件：安装/卸载/启用/禁用、依赖解析、版本锁定，
   适配 zsh-syntax-highlighting 与 zsh-autosuggestions。当前状态：安装器与元数据
   已实现，加载适配待接入。
4. **Phase 4** CLI：全部子命令已建骨架（status/doctor 可运行）。
5. **Phase 5** WebUI：后端骨架已就绪（REST/SSE/WS/IPC 客户端）。
6. **Phase 6** 生态整合：统一 `~/.config/ut/`、Nix 模块、性能、文档与自动化测试。

## 性能与兼容性目标

- 启动：空配置 < 50ms；10 个插件 < 200ms；单字符输入延迟 < 10ms。
- Bash 4.0+ 语法；Zsh 5.0+ 插件 API；优先 Linux，其次 macOS；
  终端：xterm-256color / iTerm2 / Alacritty / Kitty。
- 安全：WebUI 默认绑定 127.0.0.1 + 随机 64 位 Token；插件安装 git 签名（可选）。

## 发布与安装（GitHub Release）

每个版本发布到 GitHub Release，包含：`.deb`、`.rpm`、FHS 全量 `tar.gz`、
仅二进制 `tar.gz` 与 `SHA256SUMS`。

```bash
# 一键安装最新版（Debian→dpkg、RPM 系→rpm、其余→通用 FHS 布局；需 root）
bash <(curl -fsSL https://raw.githubusercontent.com/Reimilia617/UTSH-Project/main/scripts/install.sh)

# 指定版本 / 手动方式 / 测试根
sudo UTSH_VERSION=0.1.1 bash scripts/install.sh
sudo bash scripts/install.sh --method tarball

# 卸载（彻底干净：包注册 + 全部文件；用户数据默认保留）
sudo bash <(curl -fsSL https://raw.githubusercontent.com/Reimilia617/UTSH-Project/main/scripts/uninstall.sh)
sudo bash scripts/uninstall.sh --purge-user     # 连 ~/.config/ut 等用户数据一并清除
```

本地制作发行物与发布：

```bash
bash scripts/package.sh          # → dist/: .deb / .rpm / tar.gz / SHA256SUMS
bash scripts/publish-release.sh  # 打 tag + 上传（需 gh 或 GH_TOKEN）
```

安装后 `utsh` 位于 PATH，系统默认配置 `/etc/utsh/utsh.toml`，WebUI 后端
`/usr/share/utsh/webui`（离线可用）。详见 `packaging/README.md` 与 `scripts/`。

## 许可证

核心采用 **Apache-2.0**（见各 `Cargo.toml`）。注意：若后续引入 Bash 官方解析器
（GPL），需明确隔离 GPL 部分；当前 C++ 解析器为本项目自研文法，不引入 GPL 依赖。

## 更多

- [贡献指南](CONTRIBUTING.md)
- [API 文档](docs/api.md)
- [内置插件说明](plugins/README.md)
