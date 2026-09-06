#!/usr/bin/env bash
# UTSH 一键发布：小修/大修版本号 → 质量门禁 → 编译打包 → 推送 Release。
#
# 用法：
#   bash scripts/release.sh minor      # 例如 26v1 → 26v1.1
#   bash scripts/release.sh major      # 例如 26v1 → 26v2
#   bash scripts/release.sh 26v1.3     # 直接发布指定版本
#
# 前提：GitHub 认证（gh 或 GH_TOKEN 环境变量）；代码已提交（脚本只提交版本号变更）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ACTION="${1:-minor}"
case "$ACTION" in
    minor|major) bash scripts/bump-version.sh "$ACTION" ;;
    26v*) bash scripts/bump-version.sh set "$ACTION" ;;
    *) echo "usage: $0 minor|major|<26vN[.M]>" >&2; exit 2 ;;
esac

echo "==> gates (fmt/clippy/test)"
export PATH="$ROOT/.tools/cargo/bin:$PATH" 2>/dev/null || true
if [ -x "$ROOT/.tools/cargo/bin/cargo" ]; then
    export RUSTUP_HOME="$ROOT/.tools/rustup"
    export CARGO_HOME="$ROOT/.tools/cargo"
fi
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

echo "==> packaging"
bash scripts/package.sh

echo "==> commit version bump"
git add VERSION Cargo.toml Cargo.lock
git commit -q -m "chore: release $(cat VERSION)" || echo "(nothing to commit)"

echo "==> publish release"
bash scripts/publish-release.sh
echo "==> done: https://github.com/Reimilia617/UTSH-Project/releases/tag/$(cat VERSION)"
