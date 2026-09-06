// 安装进度实时推送：SSE（/api/install-progress）+ WebSocket（/ws）。
//
// 事件载荷来自 Rust 核心的 JSON-RPC 通知（如 progress_update、config_changed），
// 经 src/events.js 总线广播到这里（§4.7 /api/install-progress、§5 SSE/WebSocket）。

'use strict';

async function register(app) {
  const hub = app.hub;

  // SSE：安装进度
  app.get('/api/install-progress', (req, reply) => {
    const raw = reply.raw;
    raw.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    });
    raw.write('retry: 3000\n\n');

    const onNotification = (data) => {
      const json = JSON.stringify(data);
      raw.write(`event: ${data.method || 'notification'}\ndata: ${json}\n\n`);
    };
    const ping = setInterval(() => raw.write(': ping\n\n'), 15000);
    hub.on('notification', onNotification);

    req.raw.on('close', () => {
      clearInterval(ping);
      hub.removeListener('notification', onNotification);
    });
  });

  // WebSocket：任意核心通知（供未来前端直接订阅）
  app.get('/ws', { websocket: true }, (socket, _req) => {
    const onNotification = (data) => {
      if (socket.readyState === 1 /* ws.OPEN */) {
        socket.send(JSON.stringify(data));
      }
    };
    hub.on('notification', onNotification);
    socket.on('close', () => hub.removeListener('notification', onNotification));
  });
}

module.exports = register;
