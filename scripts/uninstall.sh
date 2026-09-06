#!/usr/bin/env bash
# ============================================================================
# UTSH 卸载脚本（模块：卸载 —— 彻底干净）
#
# 覆盖三种安装方式，全部清理：
#   1) .deb 安装 → dpkg -P utsh（purge，含 /etc/utsh conffile 与全部包文件）
#   2) .rpm 安装 → rpm -e utsh
#   3) tarball/手动安装 → 依 .install-manifest.txt 精确删除每个文件，
#      再兜底清理可能存在的路径（/usr、/usr/local 下的 bin/share/doc/man，
#      /etc/utsh），不留任何空壳目录。
#
# 用户数据（~/.config/ut、~/.local/share/utsh、~/.cache/ut）默认**不删**，
# 需要时加 --purge-user（本用户）或 --purge-all-users 清理所有普通用户。
#
# 用法：
#   sudo bash uninstall.sh
#   sudo bash uninstall.sh --purge-user
#   bash uninstall.sh --root /tmp/stage        # 对应安装时 --root 的测试根
# ============================================================================
set -euo pipefail

ROOTFS=""
PURGE_USER=0
PURGE_ALL_USERS=0
ASSUME_YES=0

while [ $# -gt 0 ]; do
    case "$1" in
        --root) ROOTFS="${2:?}"; shift 2 ;;
        --purge-user) PURGE_USER=1; shift ;;
        --purge-all-users) PURGE_ALL_USERS=1; shift ;;
        -y) ASSUME_YES=1; shift ;;
        -h|--help)
            sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

IS_STAGE=""
if [ -n "$ROOTFS" ]; then
    IS_STAGE=1
    ROOTFS="$(cd "$ROOTFS" && pwd)"
    echo "==> stage mode: operating under $ROOTFS"
elif [ "$(id -u)" != "0" ]; then
    echo "!! 卸载系统级安装需要 root（或使用 --root 指定测试根）。" >&2
    exit 1
fi

RP() { if [ -n "$IS_STAGE" ]; then echo "$ROOTFS$1"; else echo "$1"; fi; }

# ---------------- 0) 交互确认 ----------------
if [ "$ASSUME_YES" != "1" ] && [ -z "$IS_STAGE" ]; then
    read -r -p "将彻底卸载 utsh（含包注册与系统文件）。继续? [y/N] " ans
    [[ "$ans" =~ ^[Yy] ]] || { echo "aborted"; exit 1; }
fi

removed_any=0
log() { echo "    removed: $1"; removed_any=1; }

# ---------------- 1) 包管理器路径 ----------------
if [ -z "$IS_STAGE" ]; then
    if command -v dpkg >/dev/null 2>&1 && dpkg -l utsh >/dev/null 2>&1; then
        echo "==> utsh 由 dpkg 管理：purge"
        dpkg -P utsh
        log "dpkg purge utsh"
        rmdir "$(RP /etc/utsh)" 2>/dev/null && log "/etc/utsh (empty)"
    elif command -v rpm >/dev/null 2>&1 && rpm -q utsh >/dev/null 2>&1; then
        echo "==> utsh 由 rpm 管理：erase"
        rpm -e utsh
        log "rpm erase utsh"
        rmdir "$(RP /etc/utsh)" 2>/dev/null && log "/etc/utsh (empty)"
    fi
fi

# ---------------- 2) tarball/手动安装（清单精确删除） ----------------
echo "==> scanning install roots"
for prefix in /usr /usr/local; do
    ROOTP="$( [ -n "$IS_STAGE" ] && echo "$ROOTFS" || echo "" )"
    MAN="$ROOTP$prefix/share/utsh/.install-manifest.txt"
    [ -f "$MAN" ] || continue
    echo "==> cleaning from manifest: $MAN"
    # 清单行格式（由 install.sh 写入）：
    #   /usr/bin/utsh            绝对路径（相对真实根）
    #   webui/server.js          相对 $prefix/share/utsh/
    #   d /etc/utsh              目录标记（跳过，目录最后统一清空处理）
    while IFS= read -r line; do
        case "$line" in
            "" | "d "*) continue ;;
            /*)
                f="$ROOTP$line" ;;
            *)
                f="$ROOTP$prefix/share/utsh/$line" ;;
        esac
        if [ -f "$f" ] || [ -L "$f" ]; then
            rm -f "$f" && log "${f#$ROOTP}"
        fi
    done < "$MAN"
    rm -f "$MAN" 2>/dev/null || true
done

# ---------------- 3) 兜底清理（不依赖清单） ----------------
echo "==> removing utsh-owned paths (best effort)"
rm -f "$(RP /etc/utsh/registry.json)" 2>/dev/null && log "/etc/utsh/registry.json" || true
rm -f "$(RP /etc/utsh/utsh.toml)" 2>/dev/null && log "/etc/utsh/utsh.toml" || true
rm -f "$(RP /usr/share/utsh/.dirs.txt)" "$(RP /usr/local/share/utsh/.dirs.txt)" 2>/dev/null || true

CLEAN_FILES=(
    "$(RP /usr/bin/utsh)"
    "$(RP /usr/local/bin/utsh)"
    "$(RP /usr/share/man/man1/utsh.1.gz)"
    "$(RP /usr/local/share/man/man1/utsh.1.gz)"
)
for f in "${CLEAN_FILES[@]}"; do
    if [ -f "$f" ] || [ -L "$f" ]; then
        rm -f "$f"
        log "${f#$ROOTFS}"
    fi
done

# 撤销登录 shell 注册（/etc/shells 中的 utsh 行）
if [ -z "$IS_STAGE" ] && [ -f /etc/shells ]; then
    if grep -q '^/usr/bin/utsh$\|^/usr/local/bin/utsh$' /etc/shells; then
        sed -i '\|^/usr/bin/utsh$\|^/usr/local/bin/utsh$|d' /etc/shells
        log "/etc/shells (utsh entries)"
    fi
fi

CLEAN_DIRS=(
    "$(RP /usr/share/utsh)"
    "$(RP /usr/local/share/utsh)"
    "$(RP /usr/share/doc/utsh)"
    "$(RP /usr/local/share/doc/utsh)"
    "$(RP /etc/utsh)"
)
# 目录从深到浅删除（仅删空目录，避免误删他人文件）
for d in "${CLEAN_DIRS[@]}"; do
    if [ -d "$d" ]; then
        find "$d" -depth -type d -empty -delete 2>/dev/null || true
        if [ -d "$d" ] && [ -z "$(ls -A "$d" 2>/dev/null)" ]; then
            rmdir "$d" 2>/dev/null && log "${d#$ROOTFS}"
        fi
    fi
done
# 重新执行一次（find 删除嵌套后父目录可能刚空）
for d in $(printf '%s\n' "${CLEAN_DIRS[@]}" | sort -r); do
    [ -d "$d" ] && rmdir "$d" 2>/dev/null && log "${d#$ROOTFS}" || true
done

# ---------------- 4) 用户数据（可选） ----------------
purge_user_data() { # purge_user_data <home>
    local h="$1"
    [ -d "$h" ] || return 0
    rm -rf "$h/.config/ut"  && log "$h/.config/ut"
    rm -rf "$h/.local/share/utsh" && log "$h/.local/share/utsh"
    rm -rf "$h/.cache/ut" && log "$h/.cache/ut"
}
if [ "$PURGE_USER" = "1" ] || [ "$PURGE_ALL_USERS" = "1" ]; then
    echo "==> purging user-level data"
    if [ "$PURGE_ALL_USERS" = "1" ]; then
        for h in /home/* /root; do purge_user_data "$h"; done
    else
        purge_user_data "${HOME:-/root}"
    fi
else
    echo "==> 保留用户数据（~/.config/ut、~/.local/share/utsh、~/.cache/ut）。"
    echo "    需要一并清除时加 --purge-user 或 --purge-all-users"
fi

# ---------------- 5) 最终核验 ----------------
echo
echo "==> verify"
ok=1
if [ -z "$IS_STAGE" ]; then
    if command -v utsh >/dev/null 2>&1; then
        echo "    [FAIL] utsh 仍在 PATH: $(command -v utsh)"; ok=0
    else
        echo "    [PASS] utsh 不在 PATH"
    fi
    if dpkg -l utsh >/dev/null 2>&1; then echo "    [FAIL] dpkg 仍注册 utsh"; ok=0; else echo "    [PASS] dpkg 无注册"; fi
    if rpm -q utsh >/dev/null 2>&1; then echo "    [FAIL] rpm 仍注册 utsh"; ok=0; else echo "    [PASS] rpm 无注册"; fi
fi
for p in "$(RP /usr/bin/utsh)" "$(RP /usr/local/bin/utsh)"; do
    [ -e "$p" ] && { echo "    [FAIL] 残留: $p"; ok=0; }
done
for d in "$(RP /usr/share/utsh)" "$(RP /usr/local/share/utsh)" "$(RP /etc/utsh)"; do
    [ -d "$d" ] && [ -n "$(ls -A "$d" 2>/dev/null)" ] && { echo "    [FAIL] 残留目录非空: $d"; ok=0; }
done

[ "$ok" = "1" ] && [ "$removed_any" = "1" ] && echo "==> 卸载完成且干净。" && exit 0
[ "$ok" = "1" ] && echo "==> 未发现需要卸载的内容（可能已卸载）。" && exit 0
echo "==> 仍有残留，请人工检查上述 [FAIL] 项。" >&2
exit 1
