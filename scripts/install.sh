#!/usr/bin/env bash
# ============================================================================
# UTSH 安装脚本（模块：安装）
#
# 行为：从 GitHub Release 拉取对应版本的二进制/包/配置文件，安装进系统并注册：
#   1) 自动选择安装方式（root + Debian → .deb；root + RPM 系 → .rpm；
#      其它/非 root → 通用 FHS tar.gz 布局到 /usr 或 /usr/local）
#   2) 安装二进制 → PATH（注册）、系统默认配置 /etc/utsh/utsh.toml、
#      WebUI 后端（/usr/share/utsh）、man page、许可证/文档
#   3) 写入注册信息 /etc/utsh/registry.json，便于 uninstall.sh 精确反安装
#
# 用法：
#   sudo bash install.sh                  # 安装最新版
#   sudo UTSH_VERSION=0.1.0 bash install.sh
#   bash install.sh --root /tmp/stage     # 沙盒/测试：装入指定根目录（不调 dpkg/rpm）
#   bash install.sh --method tarball --prefix /usr/local
#
# 可选经 curl 一键安装（见 release notes）：
#   bash <(curl -fsSL https://raw.githubusercontent.com/Reimilia617/UTSH-Project/main/scripts/install.sh)
# ============================================================================
set -euo pipefail

REPO="${UTSH_REPO:-Reimilia617/UTSH-Project}"
REPO_URL="https://github.com/$REPO"
API_URL="https://api.github.com/repos/$REPO"
# 下载基址。测试/离线时可用 UTSH_BASE_URL 指向本地镜像（文件名为 dist/ 产物名）。
BASE="${UTSH_BASE_URL:-$REPO_URL/releases/download/}"

# ---------------- 参数解析 ----------------
VER=""
METHOD="auto"        # auto|deb|rpm|tarball
ROOTFS=""            # --root：安装根（默认 /，即真实系统）
PREFIX=""
ASSUME_YES=0
CREATE_USER_CONFIG=0

usage() {
    sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//' >&2
    echo >&2
    echo "options:" >&2
    echo "  --version VER        安装指定版本（默认 latest release）" >&2
    echo "  --method M           auto|deb|rpm|tarball（默认 auto）" >&2
    echo "  --root DIR           装入 DIR（测试/容器；此时强制 tarball 方式）" >&2
    echo "  --prefix DIR         手动安装前缀（默认 /usr，二进制在 /usr/bin）" >&2
    echo "  --user-config        为所有普通用户生成 ~/.config/ut/utsh.toml 启动配置" >&2
    echo "  -y                   跳过交互确认" >&2
}
while [ $# -gt 0 ]; do
    case "$1" in
        --version) VER="${2:?}"; shift 2 ;;
        --method) METHOD="${2:?}"; shift 2 ;;
        --root) ROOTFS="${2:?}"; shift 2 ;;
        --prefix) PREFIX="${2:?}"; shift 2 ;;
        --user-config) CREATE_USER_CONFIG=1; shift ;;
        -y) ASSUME_YES=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage; exit 2 ;;
    esac
done

need_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "required: $1" >&2; exit 1; }; }
need_cmd curl
need_cmd python3
need_cmd tar

# ---------------- 平台与版本解析 ----------------
arch_of() {
    case "$(uname -m)" in
        x86_64|amd64) echo amd64 ;;
        aarch64|arm64) echo arm64 ;;
        i686|i386) echo i386 ;;
        armv7l|armhf) echo armhf ;;
        *) echo "$(uname -m)" ;;
    esac
}
ARCH="$(arch_of)"

if [ -z "$VER" ]; then
    echo "==> resolving latest release ..."
    VER="$(curl -fsSL "$API_URL/releases/latest" | python3 -c "import json,sys; print(json.load(sys.stdin)['tag_name'].lstrip('v'))")"
fi
TAG="v${VER}"
echo "==> repo=$REPO  version=$VER  arch=$ARCH  method=$METHOD"

IS_STAGE=""
if [ -n "$ROOTFS" ]; then
    IS_STAGE=1
    METHOD="tarball"
    mkdir -p "$ROOTFS"
    ROOTFS="$(cd "$ROOTFS" && pwd)"
    echo "==> staging mode: installing under $ROOTFS"
elif [ "$(id -u)" != "0" ]; then
    echo "!! 非 root 且未指定 --root：系统级安装需要 root。" >&2
    echo "   提示：sudo bash $0，或 bash $0 --root <dir> 仅做演示/测试。" >&2
    exit 1
fi

# ---------------- 安装方式选择 ----------------
case "$METHOD" in
    auto)
        if [ -z "$IS_STAGE" ]; then
            if command -v dpkg >/dev/null 2>&1; then METHOD=deb
            elif command -v rpm >/dev/null 2>&1; then METHOD=rpm
            else METHOD=tarball; fi
        fi ;;
    deb|rpm|tarball) ;;
    *) echo "invalid method: $METHOD" >&2; exit 2 ;;
esac

dl() { # dl <file> <dest>
    echo "    download: $BASE/$1"
    curl -fsSL -o "$2" "$BASE/$1"
}

# ---------------- 1) .deb ----------------
if [ "$METHOD" = "deb" ]; then
    need_cmd dpkg
    DEB="utsh_${VER}_${ARCH}.deb"
    TMP="$(mktemp -d)"
    dl "$TAG/$DEB" "$TMP/$DEB"
    echo "==> installing via dpkg"
    if [ "$ASSUME_YES" != "1" ]; then
        read -r -p "    will run: dpkg -i $TMP/$DEB  [y/N] " ans
        [[ "$ans" =~ ^[Yy] ]] || { echo "aborted"; rm -rf "$TMP"; exit 1; }
    fi
    dpkg -i "$TMP/$DEB"
    rm -rf "$TMP"
    echo "==> done: $(command -v utsh) ($(utsh --version | head -1))"
    exit 0
fi

# ---------------- 2) .rpm ----------------
if [ "$METHOD" = "rpm" ]; then
    need_cmd rpm
    # rpm 文件名形如 utsh-<ver>-1.x86_64.rpm
    case "$ARCH" in amd64) ra=x86_64;; arm64) ra=aarch64;; *) ra="$ARCH";; esac
    RPM="utsh-${VER}-1.${ra}.rpm"
    TMP="$(mktemp -d)"
    dl "$TAG/$RPM" "$TMP/$RPM"
    echo "==> installing via rpm"
    if [ "$ASSUME_YES" != "1" ]; then
        read -r -p "    will run: rpm -Uvh $TMP/$RPM  [y/N] " ans
        [[ "$ans" =~ ^[Yy] ]] || { echo "aborted"; rm -rf "$TMP"; exit 1; }
    fi
    rpm -Uvh "$TMP/$RPM"
    rm -rf "$TMP"
    echo "==> done: $(command -v utsh)"
    exit 0
fi

# ---------------- 3) tarball（通用 / 手动 / --root） ----------------
[ "$METHOD" = "tarball" ] || { echo "internal error: method=$METHOD" >&2; exit 2; }

PREFIX="${PREFIX:-/usr}"
BINDIR="$PREFIX/bin"
SHAREDIR="$PREFIX/share/utsh"
MANDIR="$PREFIX/share/man/man1"
DOCDIR="$PREFIX/share/doc/utsh"
ETCDIR="/etc/utsh"          # 系统配置固定于 /etc（stage 时加 ROOTFS 前缀）
[ "$PREFIX" = "/usr" ] && ETCDIR="/etc/utsh" || ETCDIR="$PREFIX/etc/utsh"

FHS="utsh_${VER}_${ARCH}_fhs.tar.gz"
TMP="$(mktemp -d -t utsh-install-XXXXXX)"
dl "$TAG/$FHS" "$TMP/$FHS"

echo "==> extracting (prefix=$PREFIX, etc=$ETCDIR)"
ROOTP="$( [ -n "$IS_STAGE" ] && echo "$ROOTFS" || echo "" )"
ROOTBIN="$ROOTP$BINDIR"
ROOTSHARE="$ROOTP$SHAREDIR"
ROOTMAN="$ROOTP$MANDIR"
ROOTDOC="$ROOTP$DOCDIR"
ROOTETC="$ROOTP$ETCDIR"

mkdir -p "$ROOTBIN" "$ROOTSHARE" "$ROOTMAN" "$ROOTDOC" "$ROOTETC"

# 文件清单（先记录再落盘，供 uninstall 用）
MANIFEST="$ROOTSHARE/.install-manifest.txt"
: > "$MANIFEST"

put_file() { # put_file <archive_path> <dest> [mode]
    tar -xOzf "$TMP/$FHS" "${1#/}" > "$2" 2>/dev/null || { echo "    missing in archive: $1" >&2; return 1; }
    chmod "${3:-0644}" "$2"
    echo "${2#$ROOTP}" >> "$MANIFEST"
}

echo "==> installing files"
put_file usr/bin/utsh "$ROOTBIN/utsh" 0755
put_file usr/share/utsh/utsh.example.toml "$ROOTSHARE/utsh.example.toml"
put_file usr/share/man/man1/utsh.1.gz "$ROOTMAN/utsh.1.gz"
put_file usr/share/doc/utsh/LICENSE "$ROOTDOC/LICENSE"
put_file usr/share/doc/utsh/README.md "$ROOTDOC/README.md"
put_file usr/share/doc/utsh/copyright "$ROOTDOC/copyright"

# WebUI 后端（整个目录，含 node_modules）
echo "==> installing WebUI backend ($ROOTSHARE/webui)"
mkdir -p "$ROOTSHARE/webui"
if tar -tzf "$TMP/$FHS" | grep -q '^usr/share/utsh/webui/'; then
    tar -xzf "$TMP/$FHS" -C "$ROOTSHARE" usr/share/utsh/webui 2>/dev/null \
        && mv "$ROOTSHARE/usr/share/utsh/webui"/* "$ROOTSHARE/webui/" \
        && rm -rf "$ROOTSHARE/usr/share"
    # 记录后端文件清单
    ( cd "$ROOTSHARE/webui" && find . -type f | sed 's#^\./#webui/#' >> "$MANIFEST" )
    ( cd "$ROOTSHARE" && find webui -type d | sed 's#^#d #' > "$ROOTSHARE/.dirs.txt" )
fi

# 系统默认配置（已存在则不覆盖）
if [ ! -e "$ROOTETC/utsh.toml" ]; then
    put_file etc/utsh/utsh.toml "$ROOTETC/utsh.toml" || true
fi
echo "d $ETCDIR" >> "$MANIFEST"
echo "d $PREFIX/share/utsh" >> "$MANIFEST"

# 属主修正（真实安装时设为 root）
if [ -z "$IS_STAGE" ] && [ "$(id -u)" = "0" ]; then
    chown -R root:root "$ROOTBIN/utsh" "$ROOTSHARE" "$ROOTDOC" "$ROOTMAN/utsh.1.gz" "$ROOTETC" 2>/dev/null || true
fi

# 注册为合法登录 shell（chsh 只认 /etc/shells 中列出的路径）
if [ -z "$IS_STAGE" ] && [ "$(id -u)" = "0" ] && [ -f /etc/shells ]; then
    if ! grep -qxF "$BINDIR/utsh" /etc/shells; then
        echo "$BINDIR/utsh" >> /etc/shells
        echo "    registered login shell: $BINDIR/utsh  (now run: chsh -s $BINDIR/utsh)"
    fi
fi

# ---------------- 注册 ----------------
python3 - "$ROOTETC" "$VER" "$METHOD" "$PREFIX" "$ARCH" <<'PY'
import json, os, sys, time
etc, ver, method, prefix, arch = sys.argv[1:]
reg = {
    "name": "utsh", "version": ver, "method": method,
    "prefix": prefix, "arch": arch,
    "installed_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    "source": "github:Reimilia617/UTSH-Project",
}
path = os.path.join(etc, "registry.json")
with open(path, "w") as f:
    json.dump(reg, f, indent=2)
    f.write("\n")
PY
echo "registry: $ROOTETC/registry.json"

if [ "$CREATE_USER_CONFIG" = "1" ] && [ -z "$IS_STAGE" ]; then
    for h in /home/*; do
        [ -d "$h" ] || continue
        ucfg="$h/.config/ut"
        if [ ! -e "$ucfg/utsh.toml" ]; then
            mkdir -p "$ucfg"
            cp "$ROOTETC/utsh.toml" "$ucfg/utsh.toml"
            chown -R "$(basename "$h")":"$(basename "$h")" "$ucfg" 2>/dev/null || true
            echo "    user config -> $ucfg/utsh.toml"
        fi
    done
fi

rm -rf "$TMP"
echo
echo "==> install finished"
if [ -n "$IS_STAGE" ]; then
    echo "    (stage) binary: $ROOTBIN/utsh  manifest: $MANIFEST"
else
    echo "    binary   : $(command -v utsh || echo "$BINDIR/utsh (PATH 未含 $BINDIR 时请自行加入)")"
    echo "    config   : $ETCDIR/utsh.toml"
    echo "    uninstall: sudo bash <(curl -fsSL $REPO_URL/raw/main/scripts/uninstall.sh)"
fi
