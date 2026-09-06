# UTSH 一键构建脚本（Task 6）。
#
# 依赖：
#   - Rust stable + C++ 编译器（g++/clang++，编译 utsh-ffi 的 C++ 解析器）
#   - Node.js >= 18 + pnpm（或 npm）—— 仅 WebUI 需要
#   - git —— 插件安装需要
#
# 常用：
#   ./scripts/build.sh            # 仅 Rust（workspace 全部成员）
#   ./scripts/build.sh --webui    # Rust + 安装/构建 WebUI 后端
#   ./scripts/build.sh --release  # release 模式
#
# 日志统一走 pino/tracing（见 docs），此脚本不吞输出。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO="${CARGO:-cargo}"
WEBUI_PKG="${WEBUI_PKG:-pnpm}"

RELEASE=0
WITH_WEBUI=0

for arg in "$@"; do
  case "$arg" in
    --release) RELEASE=1 ;;
    --webui) WITH_WEBUI=1 ;;
    -h|--help)
      echo "usage: $0 [--release] [--webui]"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

echo "==> [1/3] building Rust workspace ($([ $RELEASE -eq 1 ] && echo release || echo debug))"
if [ "$RELEASE" -eq 1 ]; then
  "$CARGO" build --workspace --release
else
  "$CARGO" build --workspace
fi

if [ "$WITH_WEBUI" -eq 1 ]; then
  echo "==> [2/3] installing WebUI backend dependencies ($WEBUI_PKG)"
  if command -v "$WEBUI_PKG" >/dev/null 2>&1; then
    "$WEBUI_PKG" --dir webui/backend install
  else
    echo "    $WEBUI_PKG not found; falling back to npm"
    (cd webui/backend && npm install --no-audit --no-fund)
  fi
fi

echo "==> [3/3] done"
echo
echo "next steps:"
echo "  cargo run -p utsh-cli -- status     # or: utsh status"
echo "  cargo run -p utsh-cli -- doctor"
echo "  cargo run -p utsh-cli -- webui      # starts backend + browser"
