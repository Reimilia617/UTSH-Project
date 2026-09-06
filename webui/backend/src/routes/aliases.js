// /api/aliases —— 别名增删改查 + 命令语法验证（§4.7）。

'use strict';

const store = require('../config/store');

// 与 Rust 侧 config 模块一致的别名名校验。
const NAME_RE = /^[A-Za-z0-9_.-]+$/;

function validate(name, command) {
  if (!NAME_RE.test(name)) {
    return `invalid alias name "${name}": only letters, digits, '_', '-', '.' allowed`;
  }
  if (!command || !String(command).trim()) {
    return 'alias command must not be empty';
  }
  if (String(command).includes('\n')) {
    return 'alias command must not contain newlines';
  }
  return null;
}

async function register(app) {
  // 列表
  app.get('/api/aliases', async () => {
    const cfg = store.read(app.configPath);
    const aliases = cfg.aliases || {};
    return {
      aliases: Object.entries(aliases).map(([name, command]) => ({ name, command })),
    };
  });

  // 新增
  app.post('/api/aliases', async (req, reply) => {
    const { name, command } = req.body || {};
    const err = validate(name, command);
    if (err) return reply.code(400).send({ error: err });
    try {
      store.update(app.configPath, (cfg) => {
        cfg.aliases = cfg.aliases || {};
        cfg.aliases[name] = String(command);
      });
      app.core.notify('config_changed', { section: 'aliases', action: 'added', key: name, value: String(command) });
      return { status: 'ok', name, command: String(command) };
    } catch (e) {
      return reply.code(500).send({ error: e.message });
    }
  });

  // 更新
  app.put('/api/aliases/:name', async (req, reply) => {
    const name = req.params.name;
    const command = req.body && req.body.command;
    const err = validate(name, command);
    if (err) return reply.code(400).send({ error: err });
    try {
      store.update(app.configPath, (cfg) => {
        cfg.aliases = cfg.aliases || {};
        cfg.aliases[name] = String(command);
      });
      app.core.notify('config_changed', { section: 'aliases', action: 'updated', key: name, value: String(command) });
      return { status: 'ok', name, command: String(command) };
    } catch (e) {
      return reply.code(500).send({ error: e.message });
    }
  });

  // 删除
  app.delete('/api/aliases/:name', async (req, reply) => {
    const name = req.params.name;
    try {
      store.update(app.configPath, (cfg) => {
        cfg.aliases = cfg.aliases || {};
        delete cfg.aliases[name];
      });
      app.core.notify('config_changed', { section: 'aliases', action: 'removed', key: name });
      return { status: 'ok', name };
    } catch (e) {
      return reply.code(500).send({ error: e.message });
    }
  });
}

module.exports = register;
