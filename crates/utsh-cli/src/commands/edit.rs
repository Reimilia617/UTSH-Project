//! Raw 终端行编辑器：把终端按键翻译成 [`utsh_core::zle::Key`] 喂给 ZLE 状态机，
//! 并负责屏幕渲染（提示符 + 行 + 光标 + 灰色自动补全建议）。
//!
//! 能力（对应反馈逐项）：
//! - ↑/↓：按前缀/整行翻历史（engine 内置 history-search，Ctrl-R 亦可）；
//! - ←/→/Home/End/退格/Ctrl-A/E/U/K/W：光标与删除（engine 内置 Emacs 键位）；
//! - → 或 Ctrl-F 在行尾接受灰色建议（内置 autosuggestion）；
//! - Ctrl-C 取消当前行、Ctrl-D（空行）结束会话；
//! - 提示符文本由外部回调提供（shell 层传“路径提示符”）。
//!
//! 依赖：Unix termios（libc）。非 TTY 时不要调用本模块。

use std::io::{self, IsTerminal, Write};

use utsh_core::zle::{Key, ZleEngine};

use libc::{cfmakeraw, tcgetattr, tcsetattr, termios, TCSANOW};

/// 原始模式守卫：进入 raw（关闭 ICANON/ECHO），Drop 时恢复。
struct Raw {
    fd: i32,
    orig: termios,
}

impl Raw {
    fn enable() -> io::Result<Raw> {
        let fd = libc::STDIN_FILENO;
        let mut orig: termios = unsafe { std::mem::zeroed() };
        // SAFETY: fd 为 stdin，orig 由调用方管理生命周期。
        if unsafe { tcgetattr(fd, &mut orig) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut raw = orig;
        // SAFETY: raw 指向有效 termios。
        unsafe { cfmakeraw(&mut raw) };
        // 保留对收到数据的即时性；关闭 ISIG，Ctrl-C/Ctrl-D 由我们自行解释。
        // SAFETY: fd 为 stdin。
        if unsafe { tcsetattr(fd, TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Raw { fd, orig })
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        // SAFETY: 恢复原始 termios。
        unsafe { tcsetattr(self.fd, TCSANOW, &self.orig) };
    }
}

fn read_byte(fd: i32) -> io::Result<Option<u8>> {
    let mut b = [0u8; 1];
    loop {
        // SAFETY: b 有效，长度 1。
        let n = unsafe { libc::read(fd, b.as_mut_ptr() as *mut libc::c_void, 1) };
        if n == 1 {
            return Ok(Some(b[0]));
        }
        if n == 0 {
            return Ok(None); // EOF
        }
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(err);
    }
}

/// 读取一个 UTF-8 字符（首字节之后读取需要的续字节）。
fn read_char(fd: i32, first: u8) -> io::Result<char> {
    let mut buf = vec![first];
    let need = match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    };
    while buf.len() < need {
        if let Some(b) = read_byte(fd)? {
            buf.push(b);
        } else {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf)
        .chars()
        .next()
        .unwrap_or('\u{fffd}'))
}

/// 读取一个按键并归一化为 ZLE Key（含 ANSI 转义序列）。
pub fn read_key(fd: i32) -> io::Result<Option<Key>> {
    let Some(b0) = read_byte(fd)? else {
        return Ok(None);
    };
    // 控制字符
    if b0 == 0x1b {
        // 转义序列
        let Some(b1) = read_byte(fd)? else {
            return Ok(Some(Key::Esc));
        };
        if b1 == b'[' {
            let Some(b2) = read_byte(fd)? else {
                return Ok(Some(Key::Esc));
            };
            return Ok(Some(match b2 {
                b'A' => Key::Up,
                b'B' => Key::Down,
                b'C' => Key::Right,
                b'D' => Key::Left,
                b'H' => Key::Home,
                b'F' => Key::End,
                b'3' => {
                    // ^[[3~ 删除键（再读一个 ~）
                    let _ = read_byte(fd)?;
                    Key::Delete
                }
                _ => Key::Esc,
            }));
        }
        if b1 == b'O' {
            let Some(b2) = read_byte(fd)? else {
                return Ok(Some(Key::Esc));
            };
            return Ok(Some(match b2 {
                b'A' => Key::Up,
                b'B' => Key::Down,
                b'C' => Key::Right,
                b'D' => Key::Left,
                b'H' => Key::Home,
                b'F' => Key::End,
                _ => Key::Esc,
            }));
        }
        return Ok(Some(Key::Esc));
    }
    match b0 {
        0x0d | 0x0a => Ok(Some(Key::Enter)),
        0x09 => Ok(Some(Key::Tab)),
        0x7f => Ok(Some(Key::Backspace)),
        0x01..=0x1a => {
            // Ctrl-A..Z；Ctrl-I(Tab)、Ctrl-M(Enter) 已在上面处理
            Ok(Some(Key::Ctrl((b'a' + b0 - 1) as char)))
        }
        0x20..=0x7e => Ok(Some(Key::Char(b0 as char))),
        _ => read_char(fd, b0).map(|c| Some(Key::Char(c))),
    }
}

/// 展示宽度（East Asian Wide / Fullwidth 按 2 计，近似）。
pub fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    if c.is_ascii() {
        return 1;
    }
    let cp = c as u32;
    // 常见宽字符区间（CJK、全角符号、谚文等）
    if (0x1100..=0x115f).contains(&cp)
        || (0x2e80..=0xa4cf).contains(&cp)
        || (0xac00..=0xd7a3).contains(&cp)
        || (0xf900..=0xfaff).contains(&cp)
        || (0xfe30..=0xfe4f).contains(&cp)
        || (0xff00..=0xff60).contains(&cp)
        || (0x20000..=0x2fffd).contains(&cp)
        || (0x30000..=0x3fffd).contains(&cp)
    {
        2
    } else {
        1
    }
}

/// 向上找“以 buffer 开头”的最接近的历史行，返回其后缀建议。
fn suggest(engine: &ZleEngine) -> Option<String> {
    let buf = engine.buffer();
    if buf.is_empty() {
        return None;
    }
    for line in engine.history().iter().rev() {
        if line.len() > buf.len() && line.starts_with(&buf) {
            return Some(line[buf.len()..].to_string());
        }
    }
    None
}

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// 依据提示符与引擎状态重绘当前行。
fn draw(stdout: &mut impl Write, prompt: &str, engine: &ZleEngine) -> io::Result<()> {
    let line: String = engine.buffer();
    let cursor = engine.cursor();
    let sugg = engine.suggestion().map(str::to_owned).unwrap_or_default();

    // \r + 清到行尾 + 重画
    write!(stdout, "\r\x1b[2K{prompt}")?;
    write!(stdout, "{line}")?;
    if !sugg.is_empty() {
        write!(stdout, "{DIM}{sugg}{RESET}")?;
    }
    // 把光标移回提示符之后、line[cursor] 之前：
    // 右侧剩余 = 行内 [cursor..] + 建议 的显示宽度
    let tail_chars: String = line.chars().skip(cursor).collect();
    let back = display_width(&tail_chars) + display_width(&sugg);
    if back > 0 {
        write!(stdout, "\x1b[{back}D")?;
    }
    stdout.flush()
}

/// 交互编辑会话。
///
/// - `initial_history`：会话开始时的历史行（含上次会话落盘的）；
/// - `make_prompt`：每条命令输入前调用，返回提示符文本（可含 ANSI 色）；
/// - `on_line`：回车收到完整命令行后执行（返回退出码）。
///
/// 返回：`Ok(最终退出码)`。
pub fn run_interactive<F, G>(
    initial_history: Vec<String>,
    mut make_prompt: F,
    mut on_line: G,
) -> io::Result<i32>
where
    F: FnMut() -> String,
    G: FnMut(&str) -> i32,
{
    if !io::stdin().is_terminal() {
        return Ok(0);
    }
    let _raw = Raw::enable()?;
    let fd = libc::STDIN_FILENO;
    let mut stdout = io::stdout();
    let mut engine = ZleEngine::new();
    engine.set_history(initial_history);
    let mut last_code = 0;

    loop {
        engine.new_prompt();
        let prompt = make_prompt();
        draw(&mut stdout, &prompt, &engine)?;

        // 处理按键直到本行被接受或会话结束
        loop {
            let Some(key) = read_key(fd)? else {
                // EOF：如同 Ctrl-D
                if engine.buffer().is_empty() {
                    writeln!(stdout)?;
                    return Ok(last_code);
                }
                continue;
            };

            // Ctrl-C：取消当前行
            if key == Key::Ctrl('c') {
                writeln!(stdout, "^C")?;
                break;
            }
            // Ctrl-D：空行退出
            if key == Key::Ctrl('d') && engine.buffer().is_empty() {
                writeln!(stdout)?;
                return Ok(last_code);
            }

            engine.handle_key(key);

            // 行尾按 →/Ctrl-F/End 接受灰色建议
            let accept_sugg = matches!(key, Key::Right | Key::End | Key::Ctrl('f'));
            if accept_sugg && engine.cursor() == engine.buffer().chars().count() {
                if let Some(s) = suggest(&engine) {
                    engine.insert_str(&s);
                }
            }

            // 回车执行
            if let Some(line) = engine.take_accepted() {
                writeln!(stdout)?; // 换到新行再执行命令（bash/zsh 语义）
                let code = on_line(line.trim_end());
                last_code = code;
                let t = line.trim();
                let is_exit = t == "exit"
                    || t.strip_prefix("exit ")
                        .is_some_and(|r| r.trim().parse::<i32>().is_ok());
                if is_exit {
                    return Ok(code);
                }
                break; // 回到外层：换新提示符
            }

            // 更新灰色建议并重绘
            let sugg = suggest(&engine);
            engine.set_suggestion(sugg);
            draw(&mut stdout, &prompt, &engine)?;
        }
    }
}
