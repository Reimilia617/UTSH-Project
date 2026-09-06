// UTSH WebUI 后端 — 目录结构与快速上手。
//
//   src/server.js           Fastify 主服务（鉴权、装饰器、生命周期）
//   src/config/store.js     ~/.config/ut/utsh.toml 读写（smol-toml）
//   src/ipc-client/client.js Unix Domain Socket + JSON-RPC 2.0 客户端（§4.9）
//   src/events.js           进程内事件总线（把核心通知转给 SSE / WebSocket）
//   src/registry/           官方插件市场静态数据（初期）
//   src/routes/             REST 路由（plugins/themes/aliases/config/progress）
//
// 快速上手：
//   pnpm install            # 或 npm install
//   node src/server.js
//
// 首次启动会生成 ~/.config/ut/utsh.toml（含随机 X-UTSH-Token，需以
// `X-UTSH-Token` 请求头访问 /api/*）。更好的启动方式是执行 `utsh webui`
// （由 Rust CLI 拉起本服务并自动打开浏览器）。
//
// 降级策略：Rust 核心（交互式会话）未运行、unix socket 不可达时，只读接口
// （插件列表/别名/配置/主题）自动回落到本地配置与注册表数据；写操作（如安装
// 插件）返回 503 并提示启动核心。安装进度通过 `POST /api/plugins/:name` 后由
// Rust 侧以 `progress_update` 通知推送（见 docs/api.md）。

'use strict';

const path = require('node:path');
const os = require('node:os');

const Fastify = require('fastify');
const websocket = require('@fastify/websocket');

const store = require('./config/store');
const ipc = require('./ipc-client/client');
const hub = require('./events');
const routes = require('./routes');
const pkg = require('../package.json');

async function main() {
  const cfgPath = process.env.UTSH_CONFIG || store.defaultConfigPath();
  store.ensure(cfgPath); // 首次启动自动生成配置文件与随机 Token（§5）
  const token = store.tokenOf(cfgPath);

  const host = process.env.UTSH_HOST || '127.0.0.1';
  const port = Number(process.env.UTSH_PORT || (store.read(cfgPath).webui && store.read(cfgPath).webui.port) || 8787);

  const app = Fastify({ logger: { level: process.env.UTSH_LOG_LEVEL || 'info' } });
  await app.register(websocket);

  // ---- Rust 核心 IPC 客户端（自动重连） ----
  const core = ipc.createClient({
    socketPath: process.env.UTSH_SOCK || '/tmp/utsh.sock',
    log: app.log,
  });
  core.start();
  core.on('notification', (msg) => {
    hub.emit('notification', { method: msg.method, params: msg.params });
  });

  // ---- 装饰器（供路由使用） ----
  app.decorate('core', core);
  app.decorate('configPath', cfgPath);
  app.decorate('authToken', token || null);
  app.decorate('hub', hub);

  // ---- 鉴权（§6.4）：除 /health 与 /ws 外，全部 /api 需 X-UTSH-Token ----
  // 注：SSE 端点 /api/install-progress 使用 EventSource，无法带自定义头，
  // 上线前应改为 ?token= 查询参数并做时长限制（见 docs/api.md 安全一节）。
  app.addHook('onRequest', async (req, reply) => {
    const url = req.raw.url || '';
    if (url === '/health' || url === '/ws' || url === '/api/install-progress') return;
    const expected = app.authToken;
    if (expected && req.headers['x-utsh-token'] !== expected) {
      return reply.code(401).send({
        error: 'unauthorized',
        hint: 'missing or wrong X-UTSH-Token header',
      });
    }
  });

  // ---- 路由 ----
  await app.register(routes);

  app.log.info({ version: pkg.version, cfg: cfgPath }, 'UTSH WebUI backend starting');

  const shutdown = async () => {
    core.stop();
    await app.close();
    process.exit(0);
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  await app.listen({ host, port });
  console.log(`UTSH WebUI: http://${host}:${port}`);
  if (token) {
    console.log(`Auth token: ${token}`);
    console.log('Send it as the X-UTSH-Token header on /api/* requests.');
  } else {
    console.log('Auth: disabled (no [webui].auth_token configured).');
  }
}

main().catch((err) => {
  console.error('failed to start UTSH WebUI backend:', err);
  process.exit(1);
});
