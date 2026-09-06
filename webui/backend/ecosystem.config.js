// PM2 配置（§目录结构 ecosystem.config.js）。
// 用法：
//   pnpm add -g pm2
//   pm2 start webui/backend/ecosystem.config.js
module.exports = {
  apps: [
    {
      name: 'utsh-webui',
      cwd: __dirname,
      script: 'src/server.js',
      interpreter: 'node',
      instances: 1,
      autorestart: true,
      max_memory_restart: '300M',
      time: true,
      env: {
        NODE_ENV: 'production',
        UTSH_HOST: '127.0.0.1', // 默认不对外开放（§6.4 安全）
        // UTSH_PORT: 8787,
        // UTSH_SOCK: '/tmp/utsh.sock',
        // UTSH_CONFIG: '<home>/.config/ut/utsh.toml',
      },
      out_file: 'logs/out.log',
      error_file: 'logs/error.log',
      merge_logs: true,
    },
  ],
};
