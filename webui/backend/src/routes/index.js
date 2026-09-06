// REST 路由汇总：挂载所有资源路由 + /health。

'use strict';

const pkg = require('../../package.json');

async function register(app) {
  app.get('/health', async () => ({
    ok: true,
    service: 'utsh-webui-backend',
    version: pkg.version,
    coreConnected: app.core.isConnected(),
    coreSocket: process.env.UTSH_SOCK || '/tmp/utsh.sock',
  }));

  await app.register(require('./plugins'));
  await app.register(require('./themes'));
  await app.register(require('./aliases'));
  await app.register(require('./config'));
  await app.register(require('./progress'));
}

module.exports = register;
