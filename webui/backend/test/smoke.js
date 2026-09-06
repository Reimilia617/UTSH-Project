// WebUI 后端冒烟测试（本地无网络依赖；需要先 `pnpm install`）。
// 运行：npm run smoke   （或 node test/smoke.js）
//
// 覆盖：启动/健康检查、配置文件自动生成、鉴权、插件列表、别名 CRUD 与校验、
// 主题应用、配置读取、SSE 端点。
// 使用独立的临时配置目录（默认在系统临时目录），不会触碰真实
// ~/.config/ut/utsh.toml。

'use strict';

const { spawn } = require('node:child_process');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const PORT = Number(process.env.SMOKE_PORT || 8799);
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'utsh-webui-smoke-'));
const CFG = path.join(TMP, 'utsh.toml');

let passed = 0;
let failed = 0;

function check(name, cond, extra) {
  if (cond) {
    passed += 1;
    console.log(`PASS ${name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${name}${extra ? ' — ' + extra : ''}`);
  }
}

function getJson(p, token) {
  return new Promise((resolve, reject) => {
    const req = http.request(
      { host: '127.0.0.1', port: PORT, path: p, method: 'GET', headers: token ? { 'X-UTSH-Token': token } : {} },
      (res) => {
        let body = '';
        res.on('data', (c) => (body += c));
        res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body }));
      }
    );
    req.on('error', reject);
    req.end();
  });
}

function sendJson(p, method, data, token) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify(data || {});
    const req = http.request(
      {
        host: '127.0.0.1',
        port: PORT,
        path: p,
        method,
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': Buffer.byteLength(body),
          ...(token ? { 'X-UTSH-Token': token } : {}),
        },
      },
      (res) => {
        let b = '';
        res.on('data', (c) => (b += c));
        res.on('end', () => resolve({ status: res.statusCode, body: b }));
      }
    );
    req.on('error', reject);
    req.end(body);
  });
}

function headOnly(p) {
  // 流式端点（SSE）不结束响应：只等响应头即断开。
  return new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port: PORT, path: p, method: 'GET' }, (res) => {
      const info = { status: res.statusCode, headers: res.headers };
      res.destroy();
      resolve(info);
    });
    req.on('error', reject);
    req.end();
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  const child = spawn(process.execPath, ['src/server.js'], {
    cwd: path.join(__dirname, '..'),
    env: {
      ...process.env,
      UTSH_CONFIG: CFG,
      UTSH_PORT: String(PORT),
      UTSH_HOST: '127.0.0.1',
      UTSH_LOG_LEVEL: 'error',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let serverOut = '';
  let serverErr = '';
  child.stdout.on('data', (d) => (serverOut += d));
  child.stderr.on('data', (d) => (serverErr += d));

  try {
    let health = null;
    for (let i = 0; i < 60; i += 1) {
      try {
        health = await getJson('/health');
        if (health.status === 200) break;
      } catch (_) {
        /* not up yet */
      }
      await sleep(250);
    }
    check('server boot /health', Boolean(health && health.status === 200), health && health.body);

    const cfgText = fs.readFileSync(CFG, 'utf8');
    const token = (cfgText.match(/auth_token\s*=\s*"([0-9a-f]{64})"/) || [])[1];
    check('config generated with 64-hex token', Boolean(token));

    const noAuth = await getJson('/api/plugins');
    check('auth: 401 without token', noAuth.status === 401, String(noAuth.status));

    const plugins = await getJson('/api/plugins', token);
    let list = [];
    try {
      list = JSON.parse(plugins.body).plugins || [];
    } catch (_) {
      /* body not json */
    }
    check('GET /api/plugins 200', plugins.status === 200, plugins.body);
    check(
      'builtin plugins listed & enabled',
      list.some((p) => p.name === 'zsh-autosuggestions' && p.enabled),
      JSON.stringify(list.map((p) => p.name))
    );

    const add = await sendJson('/api/aliases', 'POST', { name: 'utsh-smoke', command: 'echo hi' }, token);
    check('POST /api/aliases 200', add.status === 200, add.body);
    const aliases = await getJson('/api/aliases', token);
    const al = JSON.parse(aliases.body).aliases || [];
    check('alias persisted', al.some((a) => a.name === 'utsh-smoke' && a.command === 'echo hi'));
    const bad = await sendJson('/api/aliases', 'POST', { name: 'bad name!', command: 'x' }, token);
    check('alias validation 400', bad.status === 400, bad.body);
    const del = await sendJson('/api/aliases/utsh-smoke', 'DELETE', {}, token);
    check('DELETE alias 200', del.status === 200, del.body);

    const theme = await sendJson('/api/themes/none', 'PUT', {}, token);
    check('PUT /api/themes/none 200', theme.status === 200, theme.body);

    const cfg = await getJson('/api/config', token);
    const cfgBody = JSON.parse(cfg.body).config || {};
    check(
      'GET /api/config',
      cfg.status === 200 && typeof cfgBody.webui.port === 'number' && cfgBody.webui.auth_token === token,
      cfg.body
    );

    const sse = await headOnly('/api/install-progress');
    check(
      'SSE /api/install-progress reachable',
      sse.status === 200 && String(sse.headers['content-type'] || '').includes('text/event-stream')
    );
  } finally {
    child.kill('SIGTERM');
    fs.rmSync(TMP, { recursive: true, force: true });
  }

  console.log(`\n${passed} passed, ${failed} failed`);
  if (failed > 0 || serverErr.includes('failed to start')) {
    console.error('server stderr:\n', serverErr);
    process.exit(1);
  }
  process.exit(0);
}

main().catch((err) => {
  console.error('smoke error:', err);
  process.exit(1);
});
