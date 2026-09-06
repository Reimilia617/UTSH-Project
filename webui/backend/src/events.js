// 进程内事件总线：Rust 核心推送的通知 → SSE / WebSocket 订阅者。
'use strict';

const { EventEmitter } = require('node:events');

module.exports = new EventEmitter();
module.exports.setMaxListeners(0);
