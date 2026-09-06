// 官方插件市场数据加载（初期：静态 JSON；后续可改为远程 registry）。

'use strict';

const data = require('./data.json');

function official() {
  return data.plugins;
}

function find(name) {
  return data.plugins.find((p) => p.name === name) || null;
}

module.exports = { official, find };
