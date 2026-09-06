/* grammar.y — UTSH Bash 语法（Bison 框架，WIP，尚未接入 cargo 构建）。
 *
 * =========================================================================
 * 阶段说明
 * =========================================================================
 * 当前 cargo 构建（utsh-ffi/build.rs）编译的是 bash_parser.cpp 内的
 * “最小解析器”，因此本文件**不参与当前构建**。它定义的是 Phase 1 的目标语法
 * 骨架：完整 Bash（if/for/while/[[ ]]/函数/管道/重定向/`$(...)`/`{a..b}`）。
 *
 * 接入步骤（后续阶段）：
 *   1. bison -d -o grammar.tab.cc grammar.y      （按 C++ 编译生成文件）
 *      flex -o lexer.yy.cc lexer.l
 *   2. 在 build.rs / CMakeLists.txt 中把 grammar.tab.cc、lexer.yy.cc 加入编译；
 *   3. 在 bash_parser.cpp 的 parse_bash() 中调用 yyparse()，把语法树转换为
 *      utsh_ast_node（kind/text/children），ast_to_json 随 kind 种类扩展。
 *
 * 说明：以下规则刻意保持“可读的骨架”而非可编译的完整文法——正式实现时按
 * POSIX/Bash 手册补全优先级与结合性（%left '|'、&&/|| 等）。
 * =========================================================================
 */

%{
// 生成文件将按 C++ 编译，以便动作里直接操作 utsh_ast_node。
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include "bash_parser.h"

// 语法树（临时，C++ 侧内部使用）：
// namespace utsh { ... }
// TODO: 定义中间 AST 类型并在 parse_bash() 里转换为 opaque 节点。

#define YYERROR_VERBOSE 1

/* 语法分析入口（由 bash_parser.cpp 的 parse_bash 调用）。
 * 返回值：0 成功、非 0 失败。产出树写入 out（调用方负责 free）。 */
int utsh_yyparse(utsh_ast_node** out);
%}

%token WORD
%token NEWLINE
%token IF THEN ELSE ELIF FI
%token FOR WHILE DO DONE
%token FUNCTION
%token CASE ESAC
%token AND_IF OR_IF        /* && || */
%token DSEMI               /* ;; */
%token GREAT LESS          /* > < */
%token DGREAT DLESS        /* >> << */

%start complete_command
%%

complete_command
  : list_terminator
  | list NEWLINE
  ;

list
  : pipeline
  | list AND_IF pipeline
  | list OR_IF pipeline
  ;

pipeline
  : command
  | pipeline '|' command
  ;

command
  : simple_command
  | compound_command
  ;

simple_command
  : WORD { /* TODO(Phase 1): 聚合为 argv 列表，构建 kind="command" 节点 */ }
  | simple_command WORD
  ;

compound_command
  : IF list THEN list ELSE list FI
  | IF list THEN list FI
  | WHILE list DO list DONE
  | FOR WORD DO list DONE
  | CASE WORD ESAC
  | FUNCTION WORD compound_command
  | '(' list ')'
  ;

/* [[ ]]、{a..b}、$(...) 等将在词法层折叠为 WORD 并延迟到语义分析期展开；
 * 若需保留结构，在 flex 里为 [[ ]] 单独产出 token，再添加如下规则：
 *
 * conditional_expression
 *   : LBRACKET2 expression RBRACKET2
 *   ;
 */
list_terminator
  : NEWLINE
  | ';'
  ;

%%

/* int utsh_yyparse(utsh_ast_node** out) — TODO(Phase 1)
 *
 * 由 lexer.l 提供 yylex()；这里做：
 *   1. 调用 yyparse()（若用 lalr1.cc skeleton 则直接实例化 parser）；
 *   2. 把全局/参数传入的语法树 root 拷入 *out；
 *   3. 返回 0/非 0。
 * 当前最小解析器（bash_parser.cpp）尚不依赖本文件。
 */
