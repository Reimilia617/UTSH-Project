// /api/themes —— 主题列表 / 预览 / 应用（§4.7）。

'use strict';

const fs = require('node:fs');
const path = require('node:path');

const store = require('../config/store');

function themeList() {
  const dir = store.themesDir();
  const out = [{ name: 'none', builtin: true }];
  if (fs.existsSync(dir)) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) out.push({ name: entry.name, builtin: false });
    }
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

function cssPath(name) {
  return path.join(store.themesDir(), name, `${name}.css`);
}

async function register(app) {
  // 主题列表
  app.get('/api/themes', async () => {
    const cfg = store.read(app.configPath);
    const current = cfg.theme && cfg.theme.current;
    return { current, themes: themeList() };
  });

  // 预览（返回 CSS；前端负责渲染为样式预览）
  app.get('/api/themes/:name', async (req, reply) => {
    const name = req.params.name;
    const p = cssPath(name);
    if (!fs.existsSync(p)) {
      return reply.code(404).send({
        error: `theme ${name} has no preview file`,
        hint: `expected ${p} (install themes in the WebUI; terminal preview: utsh theme preview)`,
      });
    }
    const css = fs.readFileSync(p, 'utf8');
    return reply.type('text/css').send(css);
  });

  // 应用主题
  app.put('/api/themes/:name', async (req, reply) => {
    const name = req.params.name;
    const names = themeList().map((t) => t.name);
    if (!names.includes(name)) {
      return reply.code(404).send({ error: `theme ${name} not installed` });
    }
    try {
      store.update(app.configPath, (cfg) => {
        cfg.theme = cfg.theme || {};
        cfg.theme.current = name;
      });
      app.core.notify('config_changed', { section: 'theme', action: 'set', key: name });
      return { status: 'ok', name };
    } catch (err) {
      return reply.code(500).send({ error: err.message });
    }
  });
}

module.exports = register;
