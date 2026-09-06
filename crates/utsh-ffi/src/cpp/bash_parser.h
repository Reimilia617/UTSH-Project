// UTSH C++ FFI 的稳定 C ABI。
//
// 结构体 `utsh_ast_node` 的定义对 C/Rust 隐藏（opaque），只在
// bash_parser.cpp 中补全，保证 FFI 面最小且稳定。
//
// Rust 侧绑定由 bindgen 从本文件生成（src/bindings.rs），
// 重新生成：`cargo run -p utsh-ffi --example gen_bindings`。

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct utsh_ast_node utsh_ast_node;

// 解析一段 Bash 命令行，返回新分配的 AST（用 free_ast 释放）。
// 输入为 NULL 或解析失败时返回 NULL。
struct utsh_ast_node* parse_bash(const char* input);

// 把 AST 序列化为堆上的 JSON C 字符串（用 free_cstr 释放）。
// node 为 NULL 时返回 NULL。
const char* ast_to_json(const struct utsh_ast_node* node);

// 释放 parse_bash 返回的节点。
void free_ast(struct utsh_ast_node* node);

// 释放 ast_to_json 返回的字符串。
void free_cstr(const char* s);

#ifdef __cplusplus
}  // extern "C"
#endif
