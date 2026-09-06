// bash_parser.cpp — UTSH 解析器 C++ 实现（scaffold 阶段）。
//
// 阶段说明：
//   当前实现内置一个“最小解析器”：识别简单命令（argv tokenization），
//   支持单引号 / 双引号 / 反斜杠转义，产出 `{"kind":"command",...}` JSON。
//   这样 FFI 通道、bindgen 绑定与 utsh-core::parser 可以先行跑通（含单元测试）。
//
//   完整 Bash 语法的演进路径：`grammar.y`（Bison）+ `lexer.l`（Flex）框架已经
//   就位；接入后把 `yyparse` 的结果转换为本文件的 `utsh_ast_node` 树，并扩展
//   `ast_to_json` 输出 `[[ ]]`、`{a..b}`、`$(...)`、管道/重定向等节点种类即可。
//   需要 bison/flex 的构建流程见 CMakeLists.txt 注释。
//
// 安全约定：本文件所有导出函数不得让异常越过 C 边界。

#include "bash_parser.h"

#include <cstring>
#include <string>
#include <vector>

namespace {

std::string json_escape(const std::string& s) {
    static const char hex[] = "0123456789abcdef";
    std::string out;
    out.reserve(s.size() + 8);
    for (unsigned char ch : s) {
        switch (ch) {
            case '"':
                out += "\\\"";
                break;
            case '\\':
                out += "\\\\";
                break;
            case '\n':
                out += "\\n";
                break;
            case '\r':
                out += "\\r";
                break;
            case '\t':
                out += "\\t";
                break;
            default:
                if (ch < 0x20) {
                    out += "\\u00";
                    out += hex[(ch >> 4) & 0x0F];
                    out += hex[ch & 0x0F];
                } else {
                    out += static_cast<char>(ch);
                }
        }
    }
    return out;
}

// 最小 tokenizer：引号感知地按空白切分（简单命令 → argv）。
std::vector<std::string> tokenize(const std::string& line) {
    std::vector<std::string> out;
    std::string cur;
    bool in_single = false;
    bool in_double = false;
    for (size_t i = 0; i < line.size(); ++i) {
        const char c = line[i];
        if (in_single) {
            if (c == '\'') {
                in_single = false;
            } else {
                cur += c;
            }
        } else if (in_double) {
            if (c == '"') {
                in_double = false;
            } else if (c == '\\' && i + 1 < line.size()) {
                const char n = line[i + 1];
                if (n == '"' || n == '\\' || n == '$' || n == '`') {
                    cur += n;
                    ++i;
                } else {
                    cur += c;
                    cur += n;
                    ++i;
                }
            } else {
                cur += c;
            }
        } else if (c == '\'') {
            in_single = true;
        } else if (c == '"') {
            in_double = true;
        } else if (c == '\\' && i + 1 < line.size()) {
            cur += line[i + 1];
            ++i;
        } else if (c == ' ' || c == '\t') {
            if (!cur.empty()) {
                out.push_back(cur);
                cur.clear();
            }
        } else {
            cur += c;
        }
    }
    if (!cur.empty()) {
        out.push_back(cur);
    }
    return out;
}

// 去除首尾空白。
std::string trim(const std::string& s) {
    const size_t b = s.find_first_not_of(" \t\r\n");
    if (b == std::string::npos) {
        return "";
    }
    const size_t e = s.find_last_not_of(" \t\r\n");
    return s.substr(b, e - b + 1);
}

}  // namespace

// AST 节点的实际结构（头文件中只做前置声明）。
// 注意：必须定义在全局命名空间，与 bash_parser.h 的
// `typedef struct utsh_ast_node utsh_ast_node;` 保持同一实体。
struct utsh_ast_node {
    std::string kind;
    std::string text;
    std::vector<std::string> children;
};

extern "C" {

utsh_ast_node* parse_bash(const char* input) {
    if (input == nullptr) {
        return nullptr;
    }
    try {
        std::string text(input);
        const std::string trimmed = trim(text);
        utsh_ast_node* node = new utsh_ast_node();
        node->text = trimmed;
        if (trimmed.empty()) {
            node->kind = "empty";
            return node;
        }
        // TODO(Phase 1): 完整 Bash 语法 → 由 grammar.y/lexer.l 生成的解析器接管，
        // 并把语法树递归转换为这里的节点（增加 if/for/[[ ]] 等 kind）。
        node->children = tokenize(trimmed);
        node->kind = "command";
        return node;
    } catch (...) {
        return nullptr;  // 异常不得越过 C 边界
    }
}

const char* ast_to_json(const utsh_ast_node* node) {
    if (node == nullptr) {
        return nullptr;
    }
    try {
        std::string s;
        s.reserve(node->text.size() + node->children.size() * 8 + 64);
        s += "{\"kind\":\"";
        s += json_escape(node->kind);
        s += "\",\"text\":\"";
        s += json_escape(node->text);
        s += "\",\"children\":[";
        for (size_t i = 0; i < node->children.size(); ++i) {
            if (i > 0) {
                s += ",";
            }
            s += "\"";
            s += json_escape(node->children[i]);
            s += "\"";
        }
        s += "]}";
        char* out = new char[s.size() + 1];
        std::memcpy(out, s.c_str(), s.size() + 1);
        return out;
    } catch (...) {
        return nullptr;
    }
}

void free_ast(utsh_ast_node* node) {
    delete node;
}

void free_cstr(const char* s) {
    delete[] s;  // 与 ast_to_json 中的 new[] 配对
}

}  // extern "C"
