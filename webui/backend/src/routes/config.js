// /api/config —— 完整配置读取 / 更新（§4.7 配置编辑器）。

'use strict';

const store = require('../config/store');

async function register(app) {
  // 获取完整配置
  app.get('/api/config', async () => {
    const cfg = store.read(app.configPath);
    return { config: cfg, path: app.configPath };
  });

  // 更新配置：body 顶层字段与现有配置做浅合并后整体写回。
  app.put('/api/config', async (req, reply) => {
    const patch = (req.body && req.body.config) || req.body || {};
    if (typeof patch !== 'object' || Array.isArray(patch)) {
      return reply.code(400).send({ error: 'expected a config object' });
    }
    try {
      const saved = store.update(app.configPath, (cfg) => {
        for (const [key, value] of Object.entries(patch)) {
          if (value === null || value === undefined) continue;
          if (value && typeof value === 'object' && !Array.isArray(value) && cfg[key] && typeof cfg[key] === 'object' && !Array.isArray(cfg[key])) {
            cfg[key] = { ...cfg[key], ...value };
          } else {
            cfg[key] = value;
          }
        }
      });
      app.core.notify('config_changed', { section: '*', action: 'replaced', key: 'config' });
      return { status: 'ok', config: saved };
    } catch (err) {
      return reply.code(500).send({ error: err.message });
    }
  });
}

module.exports = register;
