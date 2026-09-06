/* lexer.l — UTSH Bash 词法（Flex 框架，WIP，与 grammar.y 配套）。
 *
 * 当前构建不使用本文件（见 grammar.y 头注释）。目标词法：
 *   - WORD（含引号拼接、$VAR、$(...) 、{a..b} 的展开留待语义层）
 *   - 操作符：| & ; && || > >> < << ( ) [[ ]]
 *   - NEWLINE
 * 使用方式：flex -o lexer.yy.cc lexer.l
 */

%option noyywrap nounput noinput
%option yylineno

%{
#include <string>
// 生成文件按 C++ 编译；与 grammar.y 的 token 定义保持一致。
// 在 bison -d 生成 grammar.tab.hh 后 #include 它。
/* #include "grammar.tab.hh" */
%}

/* 空白与注释 */
blank       [ \t]
comment     #[^\n]*
nl          \n

/* 词 */
word_char   [^ \t\n|&;()<>]
word        {word_char}+

%%

{blank}+        { /* 忽略 */ }
{comment}       { /* 忽略 */ }
"&&"            { return AND_IF; }
"||"            { return OR_IF; }
"|"             { return '|'; }
";;"            { return DSEMI; }
";"             { return ';'; }
">>"            { return DGREAT; }
">"             { return GREAT; }
"<<"            { return DLESS; }
"<"             { return LESS; }
"("             { return '('; }
")"             { return ')'; }
{nl}            { return NEWLINE; }

/* 关键字（在识别出 WORD 之后区分） */
"if"            { return IF; }
"then"          { return THEN; }
"else"          { return ELSE; }
"elif"          { return ELIF; }
"fi"            { return FI; }
"for"           { return FOR; }
"while"         { return WHILE; }
"do"            { return DO; }
"done"          { return DONE; }
"function"      { return FUNCTION; }
"case"          { return CASE; }
"esac"          { return ESAC; }

{word}          {
                    // TODO(Phase 1): 维护 token 文本与位置，供语义分析展开
                    // ${...} / $(...) / {a..b} / 波浪号等。
                    return WORD;
                }

%%

/* int yylex(void) 由 flex 生成。接入时把 yylval 与 grammar.y 的语义类型绑定。
 * 本骨架文件与 grammar.y 一起在 Phase 1 接入构建（见 CMakeLists.txt 注释）。 */
