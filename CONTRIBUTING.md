# 贡献指南（CONTRIBUTING）

欢迎向 UTSH 提交代码、issue 或文档。请先阅读 [README.md](README.md) 了解项目定位，
并遵守以下约定。

## 环境准备

```bash
# 依赖：Rust stable + g++/clang++ + Node ≥ 18（+ pnpm 或 npm）+ git
make setup        # 拉取内置插件 submodule（可选）
make webui-install  # 仅当要改 WebUI 后端时
```

重新生成 C++ FFI 绑定（改动了 `bash_parser.h` 时）：
`./scripts/gen-bindings.sh`（需要 libclang；普通开发无需执行）。

## 代码风格与质量门禁

- **Rust**：
  - 提交前必须通过 `cargo fmt --all`；
  - 必须通过 `cargo clippy --workspace --all-targets -- -D warnings`；
  - 所有公开项带 rustdoc 注释（`#![warn(missing_docs)]` 已开启）；
  - `utsh-core` 禁止 `unsafe`（`#![forbid(unsafe_code)]`）；FFI 的 `unsafe`
    只允许出现在 `utsh-ffi`，且每处都要写 `// SAFETY:` 说明。
  - 单元测试放各模块 `#[cfg(test)]`；纯逻辑层优先（不依赖终端/网络）。
- **Node.js（webui/backend）**：
  - 改动后运行 `npm run check`（`node --check` 各入口）与自测脚本；
  - 保持 CommonJS 风格（当前选型），异步 I/O 用 async/await。
- **C++（utsh-ffi/src/cpp）**：不引入运行时异常越界；导出函数必须在边界内
  `try/catch`。语法文件（`grammar.y` / `lexer.l`）接入构建前保持可读骨架。

## 测试

```bash
make test      # cargo test --workspace（含 C++ 解析器的 FFI 往返测试）
```

集成测试（Phase 6 规划）：`tests/` 下用 Python 模拟终端输入（pty）驱动交互式会话，
验证提示符、补全、作业控制与插件加载。

## 提交流程

1. fork 并新建分支：`feat/<描述>`、`fix/<描述>` 或 `docs/<描述>`；
2. 单一职责的原子提交，信息用英文动词开头（`feat:`, `fix:`, `docs:`, `refactor:`）；
3. 通过 `make check && make test`；
4. 开 PR 时描述动机 + 改动要点 + 测试方式，勾选 Checklist：
   - [ ] fmt/clippy 通过
   - [ ] 新增代码有测试（纯逻辑层）
   - [ ] 公开 API 有 rustdoc
   - [ ] README/文档同步更新

## 目录地图（改哪里）

| 想改什么 | 去哪个目录 |
| --- | --- |
| 配置格式 / 默认值 | `crates/utsh-core/src/config` |
| Bash 解析（FFI / Bison 文法） | `crates/utsh-ffi/src/cpp` + `crates/utsh-core/src/parser` |
| 行编辑 / bindkey / 撤销 / 历史搜索 | `crates/utsh-core/src/zle` |
| preexec/precmd 钩子 | `crates/utsh-core/src/hooks` |
| 作业控制 | `crates/utsh-core/src/job_control` |
| 插件安装 / 元数据 | `crates/utsh-core/src/plugins` |
| `utsh` CLI 子命令 | `crates/utsh-cli/src/commands` |
| WebUI REST / SSE / WS | `webui/backend/src/routes` |
| Rust↔Node IPC | `crates/utsh-core/src/ipc` + `webui/backend/src/ipc-client` |
| 插件市场静态数据 | `webui/backend/src/registry/data.json` |

## 行为准则

保持友善与建设性；对本项目任何代码与文档的贡献均视为接受 Apache-2.0 许可。
若涉及 GPL 代码（如 Bash 官方解析器），先开 issue 讨论隔离方案，勿直接并入核心。
