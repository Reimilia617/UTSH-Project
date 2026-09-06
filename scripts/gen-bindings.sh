#!/usr/bin/env bash
# 重新生成 crates/utsh-ffi/src/bindings.rs（bindgen）。
# 需要 libclang；普通构建无需执行（绑定已提交）。
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec cargo run -p utsh-ffi --example gen_bindings
