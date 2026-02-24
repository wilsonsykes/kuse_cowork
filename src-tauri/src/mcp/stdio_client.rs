use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

struct StdioInner {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

pub struct StdioMcpClient {
    inner: Mutex<StdioInner>,
    message_id: AtomicU64,
}

impl StdioMcpClient {
    pub async fn new(
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
        working_dir: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new(command);
        if !args.is_empty() {
            cmd.args(args);
        }
        if !env.is_empty() {
            cmd.envs(env);
        }
        if let Some(dir) = working_dir {
            if !dir.trim().is_empty() {
                cmd.current_dir(dir);
            }
        }

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn stdio MCP process '{}': {}", command, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to capture MCP process stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture MCP process stdout")?;

        Ok(Self {
            inner: Mutex::new(StdioInner {
                child,
                stdin,
                stdout: BufReader::new(stdout),
            }),
            message_id: AtomicU64::new(1),
        })
    }

    pub fn pid(&self) -> Option<u32> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.child.id()
        } else {
            None
        }
    }

    pub async fn initialize(
        &self,
        startup_timeout_ms: Option<u64>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let timeout_ms = startup_timeout_ms.unwrap_or(20_000);
        let init = timeout(Duration::from_millis(timeout_ms), async {
            self.send_request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "kuse-cowork",
                        "title": "Kuse Cowork Desktop",
                        "version": "0.1.0"
                    }
                }),
            )
            .await
        })
        .await;

        let response = match init {
            Ok(res) => res?,
            Err(_) => {
                return Err(format!(
                    "Timed out after {} ms waiting for stdio MCP initialize",
                    timeout_ms
                )
                .into())
            }
        };

        self.send_notification("notifications/initialized", json!({}))
            .await?;
        Ok(response)
    }

    pub async fn list_tools(&self) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.send_request("tools/list", json!({})).await
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.send_request(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments.unwrap_or(json!({}))
            }),
        )
        .await
    }

    pub async fn shutdown(&self) {
        let mut inner = self.inner.lock().await;
        let _ = inner.child.kill().await;
        let _ = inner.child.wait().await;
    }

    async fn send_notification(
        &self,
        method: &str,
        params: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let mut inner = self.inner.lock().await;
        write_framed_json(&mut inner.stdin, &msg).await?;
        Ok(())
    }

    async fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.message_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut inner = self.inner.lock().await;
        write_framed_json(&mut inner.stdin, &req).await?;

        loop {
            let message = read_framed_json(&mut inner.stdout).await?;
            let Some(obj) = message.as_object() else {
                continue;
            };

            if obj.get("id").and_then(|v| v.as_u64()) == Some(id) {
                return Ok(message);
            }
        }
    }
}

async fn write_framed_json(
    stdin: &mut ChildStdin,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let payload = serde_json::to_vec(value)?;
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    stdin.write_all(header.as_bytes()).await?;
    stdin.write_all(&payload).await?;
    stdin.flush().await?;
    Ok(())
}

async fn read_framed_json(
    stdout: &mut BufReader<ChildStdout>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut content_length: Option<usize> = None;
    let mut first_line = String::new();
    let n = stdout.read_line(&mut first_line).await?;
    if n == 0 {
        return Err("MCP stdio stream closed".into());
    }

    if first_line.trim_start().starts_with('{') {
        return Ok(serde_json::from_str(first_line.trim())?);
    }

    if let Some((k, v)) = first_line.split_once(':') {
        if k.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(v.trim().parse::<usize>()?);
        }
    }

    loop {
        let mut line = String::new();
        let n = stdout.read_line(&mut line).await?;
        if n == 0 {
            return Err("MCP stdio stream closed while reading headers".into());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("Content-Length") {
                content_length = Some(v.trim().parse::<usize>()?);
            }
        }
    }

    let Some(len) = content_length else {
        return Err("Missing Content-Length in stdio MCP response".into());
    };
    let mut buf = vec![0_u8; len];
    stdout.read_exact(&mut buf).await?;
    Ok(serde_json::from_slice(&buf)?)
}
