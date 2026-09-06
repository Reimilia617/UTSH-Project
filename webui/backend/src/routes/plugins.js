// /api/plugins —— 插件列表 / 详情 / 安装 / 卸载 / 启用 / 禁用（§4.7）。

'use strict';

const fs = require('node:fs');
const path = require('node:path');

const store = require('../config/store');
const registry = require('../registry');

// 内置插件（对应 [plugins] enable_defaults = true）
const BUILTIN = [
  {
    name: 'zsh-syntax-highlighting',
    category: 'syntax',
    repo: 'zsh-users/zsh-syntax-highlighting',
    description: 'Fish 风格命令语法高亮（内置）',
    builtin: true,
  },
  {
    name: 'zsh-autosuggestions',
    category: 'completion',
    repo: 'zsh-users/zsh-autosuggestions',
    description: '灰色自动补全建议（内置）',
    builtin: true,
  },
];

function installedNames() {
  const dir = store.pluginsDir();
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name);
}

function coreUnavailable(reply, what) {
  return reply.code(503).send({
    error: 'utsh-core unavailable',
    hint: `${what} requires the Rust core. Start an interactive UTSH session (which listens on the unix socket) and retry.`,
  });
}

async function register(app) {
  // 汇总：内置 + 官方市场 + 已安装（读本地配置与文件系统，离线可用）
  const summarize = () => {
    const cfg = store.read(app.configPath);
    const installed = installedNames();
    const enabled = (cfg.plugins && cfg.plugins.enabled) || [];
    const useDefaults = !cfg.plugins || cfg.plugins.enable_defaults !== false;

    const out = [];
    const seen = new Set();
    if (useDefaults) {
      for (const b of BUILTIN) {
        out.push({ ...b, installed: installed.includes(b.name), enabled: true });
        seen.add(b.name);
      }
    }
    for (const p of registry.official()) {
      if (seen.has(p.name)) continue;
      out.push({
        ...p,
        installed: installed.includes(p.name),
        enabled: enabled.includes(p.name),
      });
    }
    // 已安装但不在官方市场/内置列表的插件
    for (const name of installed) {
      if (!out.some((x) => x.name === name)) {
        out.push({
          name,
          category: 'git',
          repo: '',
          description: 'installed from a git source',
          installed: true,
          enabled: enabled.includes(name),
        });
      }
    }
    return out;
  };

  // 列出已安装 + 可用插件
  app.get('/api/plugins', async () => ({
    source: 'registry+filesystem',
    plugins: summarize(),
  }));

  // 插件详情
  app.get('/api/plugins/:name', async (req, reply) => {
    const name = req.params.name;
    const found = summarize().find((p) => p.name === name);
    if (!found) {
      return reply.code(404).send({ error: `plugin ${name} not found` });
    }
    return found;
  });

  // 安装插件（需要 Rust 核心执行 git clone 并推送进度）
  app.post('/api/plugins/:name', async (req, reply) => {
    const name = req.params.name;
    const source = (req.body && (req.body.source || req.body.repo)) || '';
    if (!app.core.isConnected()) {
      return coreUnavailable(reply, 'plugin install');
    }
    const result = await app.core.request('plugin_install', { name, source });
    return { status: 'accepted', requestId: result.requestId || null };
  });

  // 卸载插件
  app.delete('/api/plugins/:name', async (req, reply) => {
    const name = req.params.name;
    if (!app.core.isConnected()) {
      return coreUnavailable(reply, 'plugin uninstall');
    }
    const result = await app.core.request('plugin_uninstall', { name });
    return { status: 'ok', name, detail: result };
  });

  // 启用 / 禁用（离线可用：直接更新配置）
  app.put('/api/plugins/:name/enable', async (req, reply) => {
    const name = req.params.name;
    const enabled = Boolean(req.body && req.body.enabled);
    try {
      store.update(app.configPath, (cfg) => {
        if (!cfg.plugins.enabled) cfg.plugins.enabled = [];
        const idx = cfg.plugins.enabled.indexOf(name);
        if (enabled && idx < 0) cfg.plugins.enabled.push(name);
        if (!enabled && idx >= 0) cfg.plugins.enabled.splice(idx, 1);
      });
      app.core.notify('config_changed', {
        section: 'plugins.enabled',
        action: enabled ? 'added' : 'removed',
        key: name,
      });
      return { status: 'ok', name, enabled };
    } catch (err) {
      return reply.code(500).send({ error: err.message });
    }
  });
}

module.exports = register;
