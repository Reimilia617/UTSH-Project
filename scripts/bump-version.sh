#!/usr/bin/env bash
# UTSH 版本管理：用户可见版本（26v1 / 26v1.1 / 26v2 …）为单一来源（根 VERSION 文件），
# Cargo 内部 semver 由映射规则同步（26v1 → 26.1.0，26v1.1 → 26.1.1，26v2 → 26.2.0）。
#
# 用法：
#   bash scripts/bump-version.sh minor     # 26v1 → 26v1.1（+0.1）
#   bash scripts/bump-version.sh major     # 26v1.9 → 26v2（+1，清小版本）
#   bash scripts/bump-version.sh set 26v5  # 直接指定
#   bash scripts/bump-version.sh           # 打印当前版本
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VFILE="$ROOT/VERSION"
CARGO_FILE="$ROOT/Cargo.toml"

python3 - "$ROOT" "$VFILE" "$CARGO_FILE" "${1:-}" "${2:-}" <<'PY'
import re, sys

root, vfile, cargo_file, action, arg = sys.argv[1:]

def read_ver():
    with open(vfile, encoding="utf-8") as f:
        return f.read().strip()

def write_ver(v):
    with open(vfile, "w", encoding="utf-8") as f:
        f.write(v + "\n")

def sync_cargo(v):
    m = re.fullmatch(r"26v(\d+)(?:\.(\d+))?", v)
    if not m:
        raise SystemExit(f"invalid version format: {v!r} (expected 26vN[.M])")
    major, minor = int(m.group(1)), int(m.group(2) or "0")
    semver = f"26.{major}.{minor}"
    with open(cargo_file, encoding="utf-8") as f:
        text = f.read()
    text2, n = re.subn(r'(?m)^version = "\d+\.\d+\.\d+"$',
                       f'version = "{semver}"', text, count=1)
    if n != 1:
        raise SystemExit("cannot locate version in Cargo.toml")
    with open(cargo_file, "w", encoding="utf-8") as f:
        f.write(text2)
    return semver

current = read_ver()
if not action or action == "current":
    print(current)
    raise SystemExit(0)

m = re.fullmatch(r"26v(\d+)(?:\.(\d+))?", current)
if not m:
    raise SystemExit(f"invalid current version {current!r}")
major, minor = int(m.group(1)), int(m.group(2) or "0")

if action == "minor":
    minor += 1                      # 小修 +0.1
    if minor > 9:                   # 10 个 0.1 进位到大修（保守规则，可自行 set）
        major += 1
        minor = 0
    nxt = f"26v{major}" if minor == 0 else f"26v{major}.{minor}"
elif action == "major":
    major += 1                      # 大修 +1，小版本清零
    minor = 0
    nxt = f"26v{major}"
elif action == "set":
    nxt = arg
    if not re.fullmatch(r"26v\d+(\.\d+)?", nxt or ""):
        raise SystemExit(f"invalid version {arg!r}")
else:
    raise SystemExit(f"unknown action: {action}")

semver = sync_cargo(nxt)
write_ver(nxt)
print(f"VERSION {current} -> {nxt}  (cargo semver -> {semver})")
PY
