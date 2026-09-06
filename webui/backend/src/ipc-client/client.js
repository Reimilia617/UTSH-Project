// Unix Domain Socket + JSON-RPC 2.0 客户端（§4.9）。
//
// 协议细节：
//   * 传输：每行一个 JSON（JSON Lines）——与 Rust 侧 utsh-core::ipc 一致；
//   * 请求：{ jsonrpc: "2.0", id: <n>, method, params }；
//   * 通知（核心主动推送，无 id）：{ jsonrpc: "2.0", method, params }，
//     例如 progress_update / config_changed；
//   * 自动重连：退避 500ms → 8s。

'use strict';

const net = require('node:net');
const { EventEmitter } = require('node:events');

class IpcClient extends EventEmitter {
  constructor({ socketPath, log, timeoutMs = 5000 }) {
    super();
    this.socketPath = socketPath;
    this.log = log || console;
    this.timeoutMs = timeoutMs;
    this.pending = new Map();
    this.nextId = 1;
    this.socket = null;
    this.buffer = '';
    this.running = false;
    this.retryTimer = null;
    this.retryDelay = 500;
  }

  isConnected() {
    return Boolean(this.socket && !this.socket.destroyed && this.socket.readyState === 'open');
  }

  start() {
    if (this.running) return;
    this.running = true;
    this._connect();
  }

  stop() {
    this.running = false;
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
    }
    this._rejectAll(new Error('ipc client stopped'));
  }

  _connect() {
    if (!this.running) return;
    const socket = net.createConnection(this.socketPath);
    this.socket = socket;
    socket.setNoDelay(true);

    socket.on('connect', () => {
      this.retryDelay = 500;
      this.log.info({ path: this.socketPath }, 'connected to utsh core (Rust)');
      this.emit('connect');
    });

    socket.on('data', (chunk) => {
      this.buffer += chunk.toString('utf8');
      this._pump();
    });

    socket.on('error', (err) => {
      this.log.warn({ path: this.socketPath, err: err.message }, 'ipc socket error');
      this._rejectAll(err);
    });

    socket.on('close', () => {
      this.socket = null;
      this._scheduleReconnect();
    });
  }

  _scheduleReconnect() {
    if (!this.running || this.retryTimer) return;
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null;
      this._connect();
    }, this.retryDelay);
    this.retryDelay = Math.min(this.retryDelay * 2, 8000);
  }

  _pump() {
    let idx;
    while ((idx = this.buffer.indexOf('\n')) >= 0) {
      const line = this.buffer.slice(0, idx).trim();
      this.buffer = this.buffer.slice(idx + 1);
      if (!line) continue;
      let msg;
      try {
        msg = JSON.parse(line);
      } catch (_) {
        continue; // 忽略坏行
      }
      if (msg && msg.id !== undefined && msg.id !== null) {
        const entry = this.pending.get(msg.id);
        if (entry) {
          this.pending.delete(msg.id);
          clearTimeout(entry.timer);
          if (msg.error) {
            entry.reject(new Error(`${msg.error.code || -1}: ${msg.error.message || 'rpc error'}`));
          } else {
            entry.resolve(msg.result);
          }
        }
      } else if (msg && msg.method) {
        this.emit('notification', msg);
      }
    }
  }

  request(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      if (!this.isConnected()) {
        return reject(new Error(`utsh core is not connected (${this.socketPath})`));
      }
      const payload = { jsonrpc: '2.0', id, method, params };
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`ipc request ${method} timed out after ${this.timeoutMs}ms`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.socket.write(`${JSON.stringify(payload)}\n`);
    });
  }

  notify(method, params = {}) {
    if (!this.isConnected()) return false;
    this.socket.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
    return true;
  }

  _rejectAll(err) {
    for (const [, entry] of this.pending) {
      clearTimeout(entry.timer);
      entry.reject(err);
    }
    this.pending.clear();
  }
}

function createClient(opts) {
  return new IpcClient(opts);
}

module.exports = { IpcClient, createClient };
