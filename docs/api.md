# UTSH API 文档（框架）

> 覆盖：WebUI REST API、SSE/WebSocket 推送、Rust ↔ Node JSON-RPC（Unix Socket）、
> CLI 命令、配置格式。本文档随代码演进；标注 **TODO** 的部分为规划/待实现项。

---

## 1. 传输与鉴权

- WebUI 后端：`http://127.0.0.1:8787`（默认，**不对外**开放）。
- 鉴权（§6.4）：`~/.config/ut/utsh.toml` 中 `[webui].auth_token`（64 位 hex，
  首次启动自动生成）。所有 `/api/*` 请求必须携带请求头
  `X-UTSH-Token: <token>`。`/health` 与 `/ws` 除外。
  > TODO(安全)：SSE `/api/install-progress` 目前绕过鉴权；上线前改为
  > `?token=` 一次性短时令牌，并做连接数/时长限制。
- 错误响应统一为 `{ "error": "...", "hint": "..." }`；HTTP 状态码遵循 REST 语义
  （400 参数、401 未鉴权、404 不存在、503 核心离线）。

## 2. WebUI REST API（§4.7）

| 方法 | 路径 | 说明 | 核心在线时走 IPC | 核心离线时 |
| --- | --- | --- | --- | --- |
| GET | `/health` | 服务与核心连接状态 | — | — |
| GET | `/api/plugins` | 已安装 + 可用插件列表 | 可选 | 本地注册表 + 目录扫描 |
| GET | `/api/plugins/:name` | 插件详情 | 可选 | 同上 |
| POST | `/api/plugins/:name` | 安装插件（body: `{source?}`） | `plugin_install` | **503** |
| DELETE | `/api/plugins/:name` | 卸载插件 | `plugin_uninstall` | **503** |
| PUT | `/api/plugins/:name/enable` | 启用/禁用（body: `{enabled}`） | 通知 `config_changed` | 本地写配置 |
| GET | `/api/themes` | 主题列表（含当前） | — | 本地 |
| GET | `/api/themes/:name` | 预览（CSS） | — | 本地文件 |
| PUT | `/api/themes/:name` | 应用主题 | 通知 `config_changed` | 本地写配置 |
| GET | `/api/aliases` | 别名列表 | 可选 | 本地 |
| POST | `/api/aliases` | 新增别名（带命令校验） | 通知 | 本地写配置 |
| PUT | `/api/aliases/:name` | 更新别名 | 通知 | 本地写配置 |
| DELETE | `/api/aliases/:name` | 删除别名 | 通知 | 本地写配置 |
| GET | `/api/config` | 完整配置 | 可选 | 本地 |
| PUT | `/api/config` | 更新配置（浅合并） | 通知 | 本地写配置 |

示例：

```http
GET /api/plugins
X-UTSH-Token: <token>
```

```json
{ "source": "registry+filesystem",
  "plugins": [ { "name": "zsh-autosuggestions", "category": "completion",
                 "repo": "zsh-users/zsh-autosuggestions", "installed": true,
                 "enabled": true } ] }
```

## 3. 实时推送

### 3.1 SSE —— 安装进度

```
GET /api/install-progress        （EventSource）
event: progress_update
data: {"method":"progress_update","params":{"plugin":"zsh-autosuggestions",
       "stage":"cloning","progress":30,"log":"Cloning repository from GitHub..."}}
```

### 3.2 WebSocket

```
ws://127.0.0.1:8787/ws
→ 任何核心通知（progress_update / config_changed …）逐条 JSON 推送
```

## 4. Rust ↔ Node JSON-RPC 2.0（Unix Domain Socket，§4.9）

- Socket：`/tmp/utsh.sock`（环境变量 `UTSH_SOCK` 可覆盖）。
- 编码：**每行一个 JSON**（JSON Lines）。
- 请求（带 `id`）与通知（不带 `id`）均符合 JSON-RPC 2.0。

Node → Rust（请求）：

| method | params | 说明 | 状态 |
| --- | --- | --- | --- |
| `ping` | `{}` | 连通性 | ✅ 已实现（测试） |
| `plugin_install` | `{name, source}` | 安装并推送进度 | TODO（注册于交互式会话） |
| `plugin_uninstall` | `{name}` | 卸载 | TODO |
| `plugin_list` | `{}` | 已安装列表 | TODO |
| `config_get` / `config_set` | — | 配置读写 | TODO |
| `config_reload` | `{}` | 热重载配置 | TODO |
| `alias_add/remove/list` | — | 别名操作 | TODO |

Rust → Node（通知）：`progress_update`（插件安装进度）、`config_changed`
（配置变更：`{section, action, key, value}`）。

```json
{ "jsonrpc": "2.0", "id": 1, "method": "plugin_install",
  "params": { "name": "zsh-autosuggestions", "source": "zsh-users/zsh-autosuggestions" } }
```

标准错误码：`-32601` 方法不存在、`-32603` 内部错误（见
`utsh-core/src/ipc/mod.rs` 常量）。

## 5. CLI（`utsh`，§4.6）

| 子命令 | 说明 | 状态 |
| --- | --- | --- |
| `utsh plugin list` | 列出已安装插件 | ✅ |
| `utsh plugin browse [category]` | 浏览官方市场 | ✅ |
| `utsh plugin install <name> [--source URL]` | 安装并自动启用 | ✅（git clone） |
| `utsh plugin uninstall/enable/disable <name>` | 卸载/启用/禁用 | ✅ |
| `utsh plugin update [name]` | 更新插件 | ✅ |
| `utsh theme list/set/preview <name>` | 主题管理 | ✅ |
| `utsh alias add/remove/list` | 别名管理 | ✅ |
| `utsh status` | 状态总览 | ✅ |
| `utsh doctor` | 环境诊断（git/node/目录/端口） | ✅ |
| `utsh webui` | 启动后端 + 打开浏览器 | ✅ |
| 全局 | `--config PATH`、`-v/-vv/-vvv` | ✅ |

## 6. 配置格式（`~/.config/ut/utsh.toml`，§4.8）

```toml
[general]
default_editor = "nvim"      # 或由 $VISUAL/$EDITOR 兜底（TODO）
history_size = 10000
history_ignore_dups = true

[prompt]
format = "utsh"              # 默认不装主题；可选 prompts/"starship" 等（Phase 6）
show_git_status = true
show_exit_code = false

[plugins]
enable_defaults = true       # 内置 zsh-syntax-highlighting + zsh-autosuggestions
autoupdate = false
max_plugins = 50
enabled = []                 # 显式启用的第三方插件

[plugins.overrides]
"zsh-syntax-highlighting" = { async = true }
"zsh-autosuggestions" = { strategy = "history" }

[aliases]
ll = "ls -alF"
gst = "git status"

[webui]
enabled = true
port = 8787
auto_open = true
auth_token = "自动生成的 64 位 hex（首次启动写入，请勿提交到仓库）"

[theme]
current = "none"
```

> TODO：与 utnixos/utniri 共享的 `~/.config/ut/common.toml`（editor/theme），
> 以及 Nix 模块 `programs.utsh`（§4.10）。

## 7. 目录约定（§4.10）

| 用途 | 路径 |
| --- | --- |
| 配置 | `~/.config/ut/utsh.toml` |
| 数据/插件 | `~/.local/share/utsh/plugins/<name>/` |
| 主题 | `~/.local/share/utsh/themes/<name>/<name>.css` |
| 缓存 | `~/.cache/ut/` |
| IPC socket | `/tmp/utsh.sock` |
