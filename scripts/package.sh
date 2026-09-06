#!/usr/bin/env bash
# UTSH 打包脚本：产出 .deb / .rpm / FHS tar.gz / 仅二进制 tar.gz / SHA256SUMS。
#
# 用法：
#   bash scripts/package.sh
#
# 说明：
#   * 版本号取自根 Cargo.toml [workspace.package].version；
#   * WebUI 后端（含 node_modules）打进包内，安装后无需联网即可 `utsh webui`
#     （仍需目标机装有 Node.js）；
#   * 构建 .deb 用 dpkg-deb（无需 root）；构建 .rpm 需要 rpmbuild，缺失时脚本
#     尝试用 apt-get download 自举到 dist/rpm-tools（仅 Debian 系，需联网）。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ---- 工具链：优先使用工作区内本地 Rust（若有） ----
if [ -x "$ROOT/.tools/cargo/bin/cargo" ]; then
    export RUSTUP_HOME="$ROOT/.tools/rustup"
    export CARGO_HOME="$ROOT/.tools/cargo"
    export PATH="$ROOT/.tools/cargo/bin:$PATH"
fi
CARGO="${CARGO:-cargo}"

# ---- 版本与架构 ----
VER="$(sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)"
[ -n "$VER" ] || { echo "cannot determine version from Cargo.toml" >&2; exit 1; }
DEB_ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
case "$DEB_ARCH" in
  amd64) RPM_ARCH=x86_64 ;;
  arm64) RPM_ARCH=aarch64 ;;
  *)     RPM_ARCH="$DEB_ARCH" ;;
esac

DIST="$ROOT/dist"
echo "==> version=$VER  deb-arch=$DEB_ARCH  rpm-arch=$RPM_ARCH"
rm -rf "$DIST"
mkdir -p "$DIST"

# ---- 1. release 构建 ----
echo "==> [1/6] cargo build --release -p utsh-cli"
"$CARGO" build --release -p utsh-cli
BIN="target/release/utsh"
[ -x "$BIN" ] || { echo "binary missing: $BIN" >&2; exit 1; }

# ---- 2. 组装 FHS 布局 ----
echo "==> [2/6] staging FHS layout"
STAGE="$DIST/stage"
mkdir -p "$STAGE/usr/bin" "$STAGE/usr/share/utsh" "$STAGE/etc/utsh" \
         "$STAGE/usr/share/doc/utsh" "$STAGE/usr/share/man/man1"

install -m 0755 "$BIN" "$STAGE/usr/bin/utsh"

# WebUI 后端（离线可用：携带 node_modules）
cp -a webui/backend "$STAGE/usr/share/utsh/webui"
rm -rf "$STAGE/usr/share/utsh/webui/logs"
rm -rf "$STAGE/usr/share/utsh/webui/.npm-cache"

install -m 0644 packaging/etc/utsh/utsh.toml "$STAGE/etc/utsh/utsh.toml"
install -m 0644 packaging/etc/utsh/utsh.toml "$STAGE/usr/share/utsh/utsh.example.toml"

gzip -9 -c packaging/man/utsh.1 > "$STAGE/usr/share/man/man1/utsh.1.gz"

cp LICENSE "$STAGE/usr/share/doc/utsh/LICENSE"
cp README.md "$STAGE/usr/share/doc/utsh/README.md"
cat > "$STAGE/usr/share/doc/utsh/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: utsh
Upstream-Contact: Reimilia617 <313009058+Reimilia617@users.noreply.github.com>
Source: https://github.com/Reimilia617/UTSH-Project

Files: *
Copyright: 2026 UT Ecosystem Team
License: Apache-2.0

Files: LICENSE
License: Apache-2.0
  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at
      http://www.apache.org/licenses/LICENSE-2.0
EOF

# ---- 3. FHS tarball（rpm Source0 与手动安装介质） ----
echo "==> [3/6] FHS tarball"
# 显式列出顶层目录，避免 tar 产生 "./" 前缀（install.sh 按精确成员名取文件）
tar -C "$STAGE" -czf "$DIST/utsh_${VER}_${DEB_ARCH}_fhs.tar.gz" usr etc

# ---- 4. 仅二进制 tarball ----
echo "==> [4/6] binary-only tarball"
mkdir -p "$DIST/binonly"
cp "$BIN" "$DIST/binonly/utsh"
cp LICENSE "$DIST/binonly/LICENSE"
( cd "$DIST/binonly" && tar -czf "$DIST/utsh_${VER}_linux_${DEB_ARCH}.tar.gz" utsh LICENSE )
rm -rf "$DIST/binonly"

# ---- 5. .deb ----
echo "==> [5/6] building .deb"
DEBROOT="$DIST/debroot"
mkdir -p "$DEBROOT/DEBIAN"
cp -a "$STAGE/." "$DEBROOT/"
SIZE="$(du -sk "$DEBROOT" | cut -f1)"
cat > "$DEBROOT/DEBIAN/control" <<EOF
Package: utsh
Version: $VER
Section: shells
Priority: optional
Architecture: $DEB_ARCH
Maintainer: Reimilia617 <313009058+Reimilia617@users.noreply.github.com>
Homepage: https://github.com/Reimilia617/UTSH-Project
Installed-Size: $SIZE
Depends: libc6 (>= 2.31), git (>= 2.0)
Recommends: nodejs (>= 18)
Description: UTSH - Bash-compatible shell manager with a Zsh plugin ecosystem
 UTSH（UT Shell）是一个兼容 Bash 语法、同时兼容 Zsh 插件与主题生态的交互式
 Shell 及其管理工具。
 .
 本包提供 utsh 命令行工具：插件/主题/别名管理、环境诊断（doctor）与 WebUI
 启动（需 nodejs）。默认内置语法高亮与自动补全，不安装任何主题。
EOF
echo "/etc/utsh/utsh.toml" > "$DEBROOT/DEBIAN/conffiles"
# 用 fakeroot 打包：让包内文件属主为 root:root
fakeroot dpkg-deb --build "$DEBROOT" "$DIST/utsh_${VER}_${DEB_ARCH}.deb" >/dev/null

# ---- 6. .rpm ----
echo "==> [6/6] building .rpm"
build_rpm() {
    local rpmbuild_bin="$1"
    local spec="$DIST/utsh.spec"
    sed "s|@FHS_TARBALL@|utsh_${VER}_${DEB_ARCH}_fhs.tar.gz|; s|@RPM_ARCH@|${RPM_ARCH}|; s|^Version: .*|Version: ${VER}|" \
        packaging/rpm/utsh.spec > "$spec"
    # 沙盒/非 root 环境无 /var/lib/rpm 与 /var/tmp 写权限：全部重定向到 dist
    mkdir -p "$DIST/rpmbuild/tmp" "$DIST/rpmbuild/BUILD" \
             "$DIST/rpmbuild/BUILDROOT" "$DIST/rpmbuild/RPMS" "$DIST/rpmdb"
    export TMPDIR="$DIST/rpmbuild/tmp"
    "$rpmbuild_bin" -bb \
        --define "_topdir $DIST/rpmbuild" \
        --define "_sourcedir $DIST" \
        --define "_specdir $DIST" \
        --define "_srcrpmdir $DIST" \
        --define "_rpmdir $DIST/rpmbuild/RPMS" \
        --define "_builddir $DIST/rpmbuild/BUILD" \
        --define "_buildrootdir $DIST/rpmbuild/BUILDROOT" \
        --define "_dbpath $DIST/rpmdb" \
        --define "_tmppath $DIST/rpmbuild/tmp" \
        "$spec" >/dev/null
    find "$DIST/rpmbuild/RPMS" -name '*.rpm' -exec mv {} "$DIST/" \;
    rmdir "$DIST/rpmbuild/RPMS"/* 2>/dev/null || true
}

if command -v rpmbuild >/dev/null 2>&1; then
    build_rpm "$(command -v rpmbuild)"
else
    echo "    rpmbuild not found; trying to bootstrap into dist/rpm-tools ..."
    mkdir -p "$DIST/rpm-tools"
    # Debian 系才支持自举；其它发行版请安装 rpm 后再跑本脚本
    pkg_rpm="$(apt-cache show rpm >/dev/null 2>&1 && echo rpm || true)"
    if [ -z "${pkg_rpm:-}" ]; then
        echo "    [!] cannot bootstrap rpm (no apt) — .rpm skipped; install rpmbuild and re-run." >&2
        echo "        spec 已保留于 packaging/rpm/utsh.spec"
    else
        # 下载 rpm 及其运行库依赖（Debian trixie 的包名；失败项仅告警）
        ( cd "$DIST/rpm-tools" && \
          for p in rpm rpm-common librpm10 librpmio10 librpmbuild10 librpmsign10 libpopt0 librpm-sequoia-1; do \
              apt-get download "$p" >/dev/null 2>&1 || echo "    warn: apt-get download $p failed"; \
          done )
        for deb in "$DIST"/rpm-tools/*.deb; do
            [ -f "$deb" ] && dpkg-deb -x "$deb" "$DIST/rpm-tools/root" >/dev/null
        done
        RPMBUILD="$DIST/rpm-tools/root/usr/bin/rpmbuild"
        export LD_LIBRARY_PATH="$DIST/rpm-tools/root/usr/lib/x86_64-linux-gnu:$DIST/rpm-tools/root/usr/lib"
        # 宏目录（/usr/lib/rpm/*）也在自举前缀里
        export RPM_CONFIGDIR="$DIST/rpm-tools/root/usr/lib/rpm"
        if [ -x "$RPMBUILD" ]; then
            build_rpm "$RPMBUILD" || echo "    [!] rpm bootstrap build failed (libs may be incomplete) — see stderr" >&2
        else
            echo "    [!] rpmbuild not found in downloaded packages — .rpm skipped." >&2
        fi
    fi
fi

# ---- 校验与校验和 ----
echo "==> verifying artifacts"
dpkg-deb --info "$DIST/utsh_${VER}_${DEB_ARCH}.deb" | sed -n '1,12p'
dpkg-deb --contents "$DIST/utsh_${VER}_${DEB_ARCH}.deb" | sed -n '1,14p'
rpm_artifact="$(ls "$DIST"/utsh-*.rpm 2>/dev/null || true)"
if [ -n "$rpm_artifact" ]; then
    echo "rpm: $rpm_artifact"
    "$DIST/rpm-tools/root/usr/bin/rpm" -qpl "$rpm_artifact" 2>/dev/null | sed -n '1,12p' \
        || tar tzf "$rpm_artifact" 2>/dev/null | head -1 >/dev/null || true
fi

(cd "$DIST" && sha256sum utsh_* > SHA256SUMS)
echo
echo "==> dist/ 产物:"
ls -lh "$DIST"/utsh_* "$DIST"/*.deb "$DIST"/*.rpm 2>/dev/null
echo "checksums: $DIST/SHA256SUMS"
