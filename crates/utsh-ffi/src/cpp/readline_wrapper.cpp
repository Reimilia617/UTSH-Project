// readline_wrapper.cpp — GNU Readline 封装（§4.3 ZLE 终端层）。
//
// 仅在 `cargo build -p utsh-ffi --features readline` 时编译并链接 -lreadline。
// 用途：把 libreadline 的终端 I/O（原始模式、按键序列、历史补全键）桥接给
// Rust 侧 ZLE 状态机（utsh-core::zle）。Rust 侧声明见 src/lib.rs 的 readline
// 模块；C 接口保持最小（函数指针回调式的完整按键转发留待 ZLE 接入阶段）。
//
// 依赖：GNU Readline（Debian/Ubuntu: libreadline-dev）。

#include <readline/history.h>
#include <readline/readline.h>

#include <cstdlib>

extern "C" {

// 读取一行；EOF 时返回 NULL。返回的 char* 由 libreadline malloc 分配，
// 调用方（Rust 侧 libc::free）负责释放。
char* utsh_rl_readline(const char* prompt) { return readline(prompt); }

// 加入历史。
void utsh_rl_add_history(const char* line) {
    if (line != nullptr && *line != '\0') {
        add_history(line);
    }
}

// 清屏并重绘当前行。
void utsh_rl_clear_screen(void) { rl_clear_screen(); }

// 库版本字符串（用于 utsh doctor 诊断）。
const char* utsh_rl_version(void) { return rl_library_version; }

// 允许 Rust 侧注册自定义绑定（ZLE 阶段使用）：
//   int utsh_rl_bind_keyseq(const char* seq, const char* fn_name);
//
// 占位：真正接入 ZLE 时在此实现 rl_bind_keyseq + rl_add_defun，把按键事件
// 通过回调转发给 Rust 状态机（见设计文档 §4.3 的“回调函数”要求）。

}  // extern "C"
