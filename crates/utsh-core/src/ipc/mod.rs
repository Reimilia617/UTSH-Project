//! Rust ↔ Node.js JSON-RPC 2.0 over Unix Domain Socket（§4.9）。
//!
//! 拓扑：**Rust 核心是服务端**（交互式会话启动时监听 `/tmp/utsh.sock`），
//! Node.js WebUI 是客户端。
//!
//! - 客户端 → 核心：带 `id` 的请求（`plugin_install`、`config_reload` …）；
//! - 核心 → 客户端：不带 `id` 的通知（`progress_update`、`config_changed` …）。
//!
//! 线上协议为“每行一个 JSON”（JSON Lines），便于用 `BufRead::lines` 处理。
//!
//! WebUI 需要的真实 handler（plugin_list / alias_* / config_* …）由上层在
//! 会话初始化时通过 [`IpcServer::register`] 注入；本模块只负责传输与分发框架。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 版本号。
pub const JSONRPC_VERSION: &str = "2.0";
/// 方法不存在错误码。
pub const ERROR_METHOD_NOT_FOUND: i64 = -32601;
/// 内部错误。
pub const ERROR_INTERNAL: i64 = -32603;

// ---------------- 消息类型 ----------------

/// JSON-RPC 请求或通知。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// 协议版本（恒为 `"2.0"`）。
    pub jsonrpc: String,
    /// 请求带 id、通知不带。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// 方法名。
    pub method: String,
    /// 方法参数。
    #[serde(default)]
    pub params: Value,
}

impl RpcRequest {
    /// 构造一个请求。
    pub fn new_request(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    /// 构造一个通知。
    pub fn new_notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.into(),
            id: None,
            method: method.into(),
            params,
        }
    }

    /// 是否为通知（无 id）。
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// 序列化为 JSON Lines 的一行。
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// JSON-RPC 错误。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcError {
    /// 错误码（见 JSON-RPC 2.0 规范）。
    pub code: i64,
    /// 人类可读的错误信息。
    pub message: String,
}

impl RpcError {
    /// 方法不存在。
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: ERROR_METHOD_NOT_FOUND,
            message: format!("method not found: {method}"),
        }
    }

    /// 内部错误。
    pub fn internal(detail: impl std::fmt::Display) -> Self {
        Self {
            code: ERROR_INTERNAL,
            message: format!("internal error: {detail}"),
        }
    }
}

/// JSON-RPC 响应。
#[derive(Debug, Clone, Serialize)]
pub struct RpcResponse {
    /// 协议版本（恒为 `"2.0"`）。
    pub jsonrpc: String,
    /// 对应请求的 id。
    pub id: u64,
    /// 成功结果（与 `error` 二选一）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// 错误信息（与 `result` 二选一）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    /// 成功响应。
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// 错误响应。
    pub fn err(id: u64, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

// ---------------- 服务端 ----------------

/// handler：`params -> result`。
pub type Handler = Box<dyn Fn(Value) -> Result<Value, RpcError> + Send + Sync>;

struct Shared {
    handlers: Mutex<HashMap<String, Handler>>,
    /// 已连接的客户端写通道（broadcast 目标）。
    clients: Mutex<Vec<mpsc::Sender<String>>>,
}

/// JSON-RPC 服务端。
#[derive(Clone)]
pub struct IpcServer {
    path: PathBuf,
    shared: Arc<Shared>,
    running: Arc<AtomicBool>,
}

impl IpcServer {
    /// 新建服务端（尚未监听）。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            shared: Arc::new(Shared {
                handlers: Mutex::new(HashMap::new()),
                clients: Mutex::new(Vec::new()),
            }),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// 使用默认 socket 路径（`UTSH_SOCK` 或 `/tmp/utsh.sock`）。
    pub fn default_path() -> Self {
        Self::new(crate::config::Config::socket_path())
    }

    /// 注册方法 handler。应在 serve/spawn 之前调用（handler 执行期间持锁，
    /// 因此 handler 内部不要调用 `register`，避免自锁）。
    pub fn register<F>(&mut self, method: impl Into<String>, handler: F)
    where
        F: Fn(Value) -> Result<Value, RpcError> + Send + Sync + 'static,
    {
        self.shared
            .handlers
            .lock()
            .unwrap()
            .insert(method.into(), Box::new(handler));
    }

    /// 阻塞式服务：监听并处理连接，直到 [`IpcServer::stop`]。
    pub fn serve(&self) -> std::io::Result<()> {
        // 清理可能残留的旧 socket 文件。
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
        let listener = UnixListener::bind(&self.path)?;
        tracing::info!(path = %self.path.display(), "utsh ipc listening");
        while self.running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    let shared = Arc::clone(&self.shared);
                    thread::spawn(move || serve_connection(stream, shared));
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    tracing::warn!(error = %e, "ipc accept failed");
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// 在后台线程运行服务。
    pub fn spawn(self) -> std::io::Result<thread::JoinHandle<std::io::Result<()>>> {
        thread::Builder::new()
            .name("utsh-ipc".into())
            .spawn(move || self.serve())
    }

    /// 停止服务（解除阻塞，best-effort 唤醒 accept）。
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        // 自连一次以唤醒阻塞中的 accept。
        let _ = UnixStream::connect(&self.path);
        let _ = std::fs::remove_file(&self.path);
    }

    /// 向所有客户端广播一条通知（如安装进度）。
    pub fn broadcast(&self, method: impl Into<String>, params: Value) -> usize {
        let req = RpcRequest::new_notification(method, params);
        let line = req.to_line();
        let mut ok = 0usize;
        let mut dead = Vec::new();
        {
            let mut clients = self.shared.clients.lock().unwrap();
            for (i, tx) in clients.iter().enumerate() {
                match tx.send(line.clone()) {
                    Ok(()) => ok += 1,
                    Err(_) => dead.push(i),
                }
            }
            // 逆序移除失效通道。
            for i in dead.iter().rev() {
                clients.remove(*i);
            }
        }
        ok
    }

    /// 同步处理一行输入并返回响应（供连接线程与测试复用）。
    ///
    /// 请求 → 响应行；通知 → `None`。
    pub fn handle_line(&self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return None, // 非法 JSON 直接忽略
        };
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let id = value.get("id").and_then(Value::as_u64);
        let params = value.get("params").cloned().unwrap_or(Value::Null);

        // 通知（无 id）无需应答。
        let id = id?;

        let resp = {
            let handlers = self.shared.handlers.lock().unwrap();
            match handlers.get(&method) {
                Some(h) => match h(params) {
                    Ok(result) => RpcResponse::ok(id, result),
                    Err(e) => RpcResponse::err(id, e),
                },
                None => RpcResponse::err(id, RpcError::method_not_found(&method)),
            }
        };
        Some(serde_json::to_string(&resp).unwrap_or_default())
    }
}

/// 服务一个客户端连接：读请求 → 应答；接收广播 → 写出。
fn serve_connection(stream: UnixStream, shared: Arc<Shared>) {
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let (tx, rx) = mpsc::channel::<String>();
    {
        let mut clients = shared.clients.lock().unwrap();
        clients.push(tx.clone());
    }
    // 读者结束标记：写线程在空闲 500ms 后据此退出，避免断连后线程泄漏。
    let done = Arc::new(AtomicBool::new(false));
    let done_w = Arc::clone(&done);
    // 写线程：消费所有待写行（广播 + 应答共用通道，保证顺序）。
    let writer_thread = thread::spawn(move || loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                if writer.write_all(line.as_bytes()).is_err()
                    || writer.write_all(b"\n").is_err()
                    || writer.flush().is_err()
                {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if done_w.load(Ordering::SeqCst) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    });

    let server = IpcServer {
        path: PathBuf::new(),
        shared: Arc::clone(&shared),
        running: Arc::new(AtomicBool::new(true)),
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if let Some(reply) = server.handle_line(&line) {
            let _ = tx.send(reply);
        }
    }
    done.store(true, Ordering::SeqCst);
    drop(tx);
    let _ = writer_thread.join();
}

// ---------------- 客户端 ----------------

/// 客户端（供 Node 之外的本进程/测试使用；WebUI 用 Node 实现，见 webui/backend）。
pub struct IpcClient {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: AtomicU64,
}

/// 客户端错误。
#[derive(Debug, thiserror::Error)]
pub enum IpcClientError {
    /// I/O 错误。
    #[error("ipc client io error: {0}")]
    Io(#[from] std::io::Error),
    /// 收到错误响应。
    #[error("ipc error {code}: {message}")]
    Remote {
        /// JSON-RPC 错误码。
        code: i64,
        /// 错误信息。
        message: String,
    },
    /// 无法从行中解析 JSON。
    #[error("ipc protocol error: {0}")]
    Protocol(String),
}

impl IpcClient {
    /// 连接到服务端 socket。
    pub fn connect(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let stream = UnixStream::connect(path.into())?;
        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);
        Ok(Self {
            writer,
            reader,
            next_id: AtomicU64::new(1),
        })
    }

    /// 同步请求。阻塞读取直到与 id 匹配的响应。
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, IpcClientError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = RpcRequest::new_request(id, method, params);
        let line = req.to_line();
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut buf = String::new();
        loop {
            buf.clear();
            let n = self.reader.read_line(&mut buf)?;
            if n == 0 {
                return Err(IpcClientError::Protocol("connection closed".into()));
            }
            let v: Value = serde_json::from_str(buf.trim())
                .map_err(|e| IpcClientError::Protocol(e.to_string()))?;
            if v.get("id").and_then(Value::as_u64) != Some(id) {
                continue; // 其它通知/乱序消息，跳过
            }
            if let Some(err) = v.get("error") {
                return Err(IpcClientError::Remote {
                    code: err.get("code").and_then(Value::as_i64).unwrap_or(-1),
                    message: err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                });
            }
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// 发送通知（不等待响应）。
    pub fn notify(&mut self, method: &str, params: Value) -> std::io::Result<()> {
        let req = RpcRequest::new_notification(method, params);
        self.writer.write_all(req.to_line().as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }
}

/// 便捷构造：`notify("progress_update", json!({...}))` 用于 [`IpcServer::broadcast`]。
pub fn notification(method: &str, params: Value) -> RpcRequest {
    RpcRequest::new_notification(method, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    #[allow(clippy::redundant_closure)] // 测试意图是“原样返回参数”。
    fn handle_line_request_and_notification() {
        let mut server = IpcServer::new("/tmp/utsh-test.sock");
        server.register("echo", |params| Ok(params));
        // 通知（无 id）→ 无响应
        let n = RpcRequest::new_notification("progress_update", json!({"p": 1}));
        assert_eq!(server.handle_line(&n.to_line()), None);
        // 请求 → 有响应
        let req = RpcRequest::new_request(7, "echo", json!({"x": 1}));
        let reply = server.handle_line(&req.to_line()).unwrap();
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["id"].as_u64(), Some(7));
        assert_eq!(v["result"]["x"].as_i64(), Some(1));
        // 未注册方法 → 错误响应
        let req2 = RpcRequest::new_request(8, "no_such", Value::Null);
        let reply2 = server.handle_line(&req2.to_line()).unwrap();
        let v2: Value = serde_json::from_str(&reply2).unwrap();
        assert_eq!(v2["error"]["code"].as_i64(), Some(ERROR_METHOD_NOT_FOUND));
    }

    #[test]
    fn request_serialization_shape() {
        let r = RpcRequest::new_request(
            1,
            "plugin_install",
            json!({
                "name": "zsh-autosuggestions",
                "source": "zsh-users/zsh-autosuggestions"
            }),
        );
        let parsed: Value = serde_json::from_str(&r.to_line()).unwrap();
        assert_eq!(parsed["jsonrpc"].as_str(), Some("2.0"));
        assert_eq!(parsed["method"].as_str(), Some("plugin_install"));
        assert_eq!(
            parsed["params"]["name"].as_str(),
            Some("zsh-autosuggestions")
        );
        assert!(!r.is_notification());
        let n = RpcRequest::new_notification("config_changed", json!({}));
        assert!(n.is_notification());
        assert!(!n.to_line().contains("\"id\""));
    }

    /// 端到端：spawn 服务端 + 客户端请求。注意测试期间 socket 路径唯一。
    #[test]
    fn client_server_roundtrip() {
        let path = format!("/tmp/utsh-e2e-{}.sock", std::process::id());
        let mut server = IpcServer::new(path.as_str());
        server.register("ping", |_| Ok(json!({"pong": true})));
        let handle = server.clone().spawn().expect("spawn server");
        // 等服务端就绪。
        let mut client = None;
        for _ in 0..50 {
            match IpcClient::connect(path.as_str()) {
                Ok(c) => {
                    client = Some(c);
                    break;
                }
                Err(_) => thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        let mut client = client.expect("connect to server");
        let out = client.request("ping", Value::Null).expect("rpc ok");
        assert_eq!(out["pong"].as_bool(), Some(true));
        server.stop();
        let _ = handle.join();
        let _ = std::fs::remove_file(&path);
    }
}
