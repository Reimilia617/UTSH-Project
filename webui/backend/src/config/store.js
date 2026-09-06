// 配置存取：~/.config/ut/utsh.toml（§4.8）。
//
// 说明：配置的“单一事实来源”在 Rust 核心（utsh-core::config）；本模块只是
// WebUI 侧为了方便预览/编辑而做的镜像读写，所有写操作会与 Rust 侧保持同样的
// 字段结构（默认值见 DEFAULTS，与 utsh-core 一致）。若 Rust 核心在线，路由层
// 优先走 IPC 的 config_get / config_set，落盘仍在 Rust 侧完成。

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { parse, stringify } = require('smol-toml');

const HOME = os.homedir();

function defaultConfigPath() {
  return path.join(HOME, '.config', 'ut', 'utsh.toml');
}

function defaultDataDir() {
  return path.join(HOME, '.local', 'share', 'utsh');
}

// 与 Rust 侧 Config::default() 对齐的默认结构。
const DEFAULTS = {
  general: { default_editor: 'nvim', history_size: 10000, history_ignore_dups: true },
  prompt: { format: 'utsh', show_git_status: true, show_exit_code: false },
  plugins: { enable_defaults: true, autoupdate: false, max_plugins: 50, enabled: [], overrides: {} },
  aliases: { ll: 'ls -alF', gst: 'git status', ga: 'git add' },
  webui: { enabled: true, port: 8787, auto_open: true },
  theme: { current: 'none' },
};

// smol-toml 返回的对象原型与普通对象不同，这里统一深转为纯 JSON 对象。
function deepPlain(value) {
  if (Array.isArray(value)) return value.map(deepPlain);
  if (value && typeof value === 'object') {
    const out = {};
    for (const key of Object.keys(value)) out[key] = deepPlain(value[key]);
    return out;
  }
  return value;
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

let cache = { path: null, mtimeMs: 0, data: null };

function invalidate() {
  cache = { path: null, mtimeMs: 0, data: null };
}

function readRaw(cfgPath) {
  if (!fs.existsSync(cfgPath)) return null;
  const stat = fs.statSync(cfgPath);
  if (cache.path === cfgPath && cache.data && stat.mtimeMs === cache.mtimeMs) {
    return cache.data;
  }
  const text = fs.readFileSync(cfgPath, 'utf8');
  const data = deepPlain(parse(text));
  cache = { path: cfgPath, mtimeMs: stat.mtimeMs, data };
  return data;
}

// 读取并合并默认值（缺段补默认）。
function read(cfgPath) {
  const raw = readRaw(cfgPath);
  const out = clone(DEFAULTS);
  if (!raw) return out;
  for (const key of Object.keys(DEFAULTS)) {
    const v = raw[key];
    if (v === undefined) continue;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      out[key] = { ...clone(DEFAULTS[key]), ...clone(v) };
    } else {
      out[key] = clone(v);
    }
  }
  return out;
}

function write(cfgPath, data) {
  fs.mkdirSync(path.dirname(cfgPath), { recursive: true });
  const text = stringify(data);
  fs.writeFileSync(cfgPath, text.endsWith('\n') ? text : `${text}\n`);
  invalidate();
}

// 用 update 回调做“读-改-写”。
function update(cfgPath, mutate) {
  const data = read(cfgPath);
  mutate(data);
  write(cfgPath, data);
  return data;
}

function generateToken() {
  return crypto.randomBytes(32).toString('hex'); // 64 位 hex（§6.4）
}

// 首次启动：配置文件不存在时生成（含随机 Token），并确保目录就绪。
function ensure(cfgPath) {
  if (!fs.existsSync(cfgPath)) {
    const cfg = clone(DEFAULTS);
    cfg.webui.auth_token = generateToken();
    write(cfgPath, cfg);
  }
  return cfgPath;
}

function tokenOf(cfgPath) {
  const cfg = read(cfgPath);
  return (cfg.webui && cfg.webui.auth_token) || null;
}

function pluginsDir() {
  return process.env.UTSH_PLUGINS_DIR || path.join(defaultDataDir(), 'plugins');
}

function themesDir() {
  return process.env.UTSH_THEMES_DIR || path.join(defaultDataDir(), 'themes');
}

module.exports = {
  DEFAULTS,
  defaultConfigPath,
  defaultDataDir,
  pluginsDir,
  themesDir,
  read,
  write,
  update,
  ensure,
  tokenOf,
  generateToken,
};
