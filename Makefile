# UTSH —— 一键构建入口（Task 6）。
#
# 快速开始：
#   make setup     # 安装 Rust/Node 依赖 + 拉取内置插件 submodule（如尚未添加）
#   make build     # debug 构建 Rust workspace（含 C++ FFI）
#   make build-release
#   make webui-install && make webui     # 安装并启动 WebUI（等价 `utsh webui`）
#   make test      # Rust 全部测试（workspace，含 C++ 解析器测试）
#   make check     # fmt + clippy + cargo check
#
# 依赖版本见根 README.md 与 rust-toolchain.toml。

CARGO        ?= cargo
WEBUI_DIR    := webui/backend
# pnpm 优先，其次 npm
WEBUI_PKG    := $(shell command -v pnpm >/dev/null 2>&1 && echo pnpm || echo npm)

.DEFAULT_GOAL := help

.PHONY: help setup build build-release build-ffi check fmt clippy test \
        webui-install webui dev-submodules bindings clean doc

help: ## 显示帮助
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
	  awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

setup: ## 初始化：添加默认插件 submodule（幂等）
	@if [ -d plugins/zsh-syntax-highlighting/.git ]; then \
	  echo "plugins/zsh-syntax-highlighting already present"; \
	else \
	  git submodule add https://github.com/zsh-users/zsh-syntax-highlighting plugins/zsh-syntax-highlighting || true; \
	fi
	@if [ -d plugins/zsh-autosuggestions/.git ]; then \
	  echo "plugins/zsh-autosuggestions already present"; \
	else \
	  git submodule add https://github.com/zsh-users/zsh-autosuggestions plugins/zsh-autosuggestions || true; \
	fi
	@echo "setup done"

build: ## debug 构建 Rust workspace（含 C++ FFI）
	$(CARGO) build --workspace

build-release: ## release 构建
	$(CARGO) build --workspace --release

build-ffi: ## 单独编译 C++ FFI 目标
	$(CARGO) build -p utsh-ffi

bindings: ## 重新生成 C++ FFI 的 Rust 绑定（需要 libclang）
	$(CARGO) run -p utsh-ffi --example gen_bindings

fmt: ## 格式化
	$(CARGO) fmt --all

clippy: ## lint
	$(CARGO) clippy --workspace --all-targets -- -D warnings

check: fmt clippy ## fmt + clippy + check
	$(CARGO) check --workspace

test: ## 运行全部 Rust 测试（workspace）
	$(CARGO) test --workspace

webui-install: ## 安装 WebUI 后端依赖
	$(WEBUI_PKG) --dir $(WEBUI_DIR) install

webui: webui-install ## 启动 WebUI 后端（前台）
	node $(WEBUI_DIR)/src/server.js

clean: ## 清理构建产物
	$(CARGO) clean
	rm -rf $(WEBUI_DIR)/node_modules $(WEBUI_DIR)/logs

doc: ## 生成 Rust 文档
	$(CARGO) doc --workspace --no-deps
