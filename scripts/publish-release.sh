#!/usr/bin/env bash
# 发布 UTSH Release：打 tag → 推送 → 上传 dist/ 产物到 GitHub Release。
#
# 依赖（二选一）：
#   1) gh CLI 已登录（gh auth login）
#   2) 环境变量 GH_TOKEN 或 GITHUB_TOKEN（classic token，需 repo 权限）
# 打 tag/推送代码用 SSH（无需 token）；只有“创建 Release + 上传资产”需要 token。
#
# 用法：
#   bash scripts/package.sh            # 先生成 dist/ 产物
#   bash scripts/publish-release.sh    # 默认发布当前 Cargo 版本
#   UTSH_VERSION=0.1.0 bash scripts/publish-release.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REPO="Reimilia617/UTSH-Project"
VER="${UTSH_VERSION:-$(sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)}"
TAG="v${VER}"
DIST="$ROOT/dist"

[ -f "$DIST/SHA256SUMS" ] || { echo "dist/ missing — run bash scripts/package.sh first" >&2; exit 1; }

# ---------- 1) tag + push（SSH） ----------
if ! git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "==> creating tag $TAG"
    git tag -a "$TAG" -m "UTSH $VER"
fi
echo "==> pushing tag"
git push origin "$TAG"

# ---------- 2) release notes ----------
PREV="$(git describe --abbrev=0 --tags "$TAG^" 2>/dev/null || true)"
NOTES="$DIST/RELEASE_NOTES.md"
{
    echo "# UTSH $VER"
    echo
    echo "UTSH（UT Shell）：兼容 Bash 语法、兼容 Zsh 插件/主题生态的 Shell 管理工具。"
    echo
    echo "## 安装"
    echo '```bash'
    echo 'bash <(curl -fsSL https://raw.githubusercontent.com/Reimilia617/UTSH-Project/main/scripts/install.sh)'
    echo '```'
    echo
    echo "## 变更"
    if [ -n "$PREV" ]; then
        git log --oneline --no-decorate "$PREV..HEAD"
    else
        git log --oneline --no-decorate -20
    fi
    echo
    echo "## 校验和（SHA256）"
    echo '```'
    cat "$DIST/SHA256SUMS"
    echo '```'
} > "$NOTES"

# ---------- 3) create release + upload ----------
ASSETS=()
for f in "$DIST"/utsh_*.deb "$DIST"/utsh-*.rpm "$DIST"/utsh_*_fhs.tar.gz "$DIST"/utsh_*_linux_*.tar.gz "$DIST"/SHA256SUMS; do
    [ -f "$f" ] && ASSETS+=("$f")
done
echo "==> assets: ${#ASSETS[@]}"

if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    echo "==> publishing via gh"
    gh release create "$TAG" "${ASSETS[@]}" \
        --repo "$REPO" \
        --title "UTSH $VER" \
        --notes-file "$NOTES"
    echo "==> done: https://github.com/$REPO/releases/tag/$TAG"
    exit 0
fi

if [ -n "${GH_TOKEN:-}" ] || [ -n "${GITHUB_TOKEN:-}" ]; then
    TOKEN="${GH_TOKEN:-${GITHUB_TOKEN}}"
    echo "==> publishing via REST API"
    api="https://api.github.com/repos/$REPO/releases"
    create_out="$(curl -fsSL -X POST "$api" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Accept: application/vnd.github+json" \
        -d "$(python3 -c "import json,sys; print(json.dumps({'tag_name':'$TAG','name':'UTSH $VER','body':open('$NOTES').read()}))")")"
    upload_url="$(printf '%s' "$create_out" | python3 -c "import json,sys; print(json.load(sys.stdin)['upload_url'].split('{')[0])")"
    for f in "${ASSETS[@]}"; do
        name="$(basename "$f")"
        echo "    upload: $name"
        code="$(curl -sSL -o /dev/null -w '%{http_code}' -X POST \
            -H "Authorization: Bearer $TOKEN" \
            -H "Content-Type: application/octet-stream" \
            --data-binary "@$f" \
            "$upload_url?name=$name")"
        [ "$code" = "201" ] || [ "$code" = "200" ] || { echo "upload failed for $name: http $code" >&2; exit 2; }
    done
    echo "==> done: https://github.com/$REPO/releases/tag/$TAG"
    exit 0
fi

echo
echo "!! 无法创建 GitHub Release：未检测到 gh CLI 或 GH_TOKEN/GITHUB_TOKEN。"
echo "   已完成（无需 token 的部分）：tag $TAG 已推送、发行物已就绪于 dist/" >&2
echo "   请在下面任选其一后重跑本脚本："
echo "     1) gh auth login 后再 bash scripts/publish-release.sh"
echo "     2) 提供 token： GH_TOKEN=ghp_xxx bash scripts/publish-release.sh" >&2
exit 3
