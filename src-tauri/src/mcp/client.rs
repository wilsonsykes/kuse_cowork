use super::http_client::HttpMcpClient;
use super::stdio_client::{ProtocolMode, StdioMcpClient};
use super::types::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};

enum MCPTransportClient {
    Http(HttpMcpClient),
    Stdio(StdioMcpClient),
}

pub struct MCPClient {
    transport_client: MCPTransportClient,
    #[allow(dead_code)]
    url: String,
}

struct ManagedProcess {
    child: Child,
    pid: Option<u32>,
}

pub struct MCPManager {
    clients: Arc<RwLock<HashMap<String, MCPClient>>>,
    server_status: Arc<RwLock<HashMap<String, MCPServerStatus>>>,
    managed_processes: Arc<RwLock<HashMap<String, ManagedProcess>>>,
}

impl MCPManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            server_status: Arc::new(RwLock::new(HashMap::new())),
            managed_processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn connect_server(
        &self,
        config: &MCPServerConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !config.enabled {
            return Err("Server is not enabled".into());
        }

        let transport = normalize_transport(&config.transport);

        {
            let mut status_map = self.server_status.write().await;
            status_map.insert(
                config.id.clone(),
                MCPServerStatus {
                    id: config.id.clone(),
                    name: config.name.clone(),
                    transport: transport.clone(),
                    status: ConnectionStatus::Connecting,
                    tools: vec![],
                    last_error: None,
                    managed_process: false,
                    pid: None,
                    endpoint: if config.server_url.is_empty() {
                        None
                    } else {
                        Some(config.server_url.clone())
                    },
                },
            );
        }

        let connect_result = match transport.as_str() {
            "http" => self.connect_http_server(config).await,
            "stdio" => self.connect_stdio_server(config).await,
            _ => Err(format!("Unsupported MCP transport: {}", transport).into()),
        };

        if let Err(err) = connect_result {
            let msg = err.to_string();
            self.update_status_error(&config.id, msg.clone()).await;
            return Err(msg.into());
        }

        Ok(())
    }

    async fn connect_http_server(
        &self,
        config: &MCPServerConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let managed = self.start_managed_process_if_needed(config).await?;

        let oauth_token = if let (Some(client_id), Some(client_secret)) =
            (&config.oauth_client_id, &config.oauth_client_secret)
        {
            Some(self.perform_oauth_flow(client_id, client_secret, &config.server_url).await?)
        } else {
            None
        };

        let endpoint = if config.server_url.trim().is_empty() {
            return Err("Server URL is required for HTTP transport".into());
        } else {
            config.server_url.clone()
        };

        let mut http_client = HttpMcpClient::new(endpoint.clone(), oauth_token);
        if let Err(e) = self
            .initialize_http_with_retry(&mut http_client, config.startup_timeout_ms)
            .await
        {
            self.stop_managed_process(&config.id).await;
            return Err(format!("Connection failed: {}", e).into());
        }

        let transport_client = MCPTransportClient::Http(http_client);
        let tools = self.discover_tools(&transport_client, &config.id).await?;

        let mcp_client = MCPClient {
            transport_client,
            url: endpoint.clone(),
        };

        {
            let mut clients = self.clients.write().await;
            clients.insert(config.id.clone(), mcp_client);
        }

        {
            let mut status_map = self.server_status.write().await;
            status_map.insert(
                config.id.clone(),
                MCPServerStatus {
                    id: config.id.clone(),
                    name: config.name.clone(),
                    transport: "http".to_string(),
                    status: ConnectionStatus::Connected,
                    tools,
                    last_error: None,
                    managed_process: managed.is_some(),
                    pid: managed,
                    endpoint: Some(endpoint),
                },
            );
        }

        Ok(())
    }

    async fn connect_stdio_server(
        &self,
        config: &MCPServerConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let command = config
            .launch_command
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or("Launch command is required for stdio transport")?;

        if let Some(dir) = config
            .working_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if !Path::new(dir).exists() {
                return Err(format!("Working directory not found: {}", dir).into());
            }
        }

        let mut last_error: Option<String> = None;
        let mut selected_client: Option<StdioMcpClient> = None;
        let mut pid: Option<u32> = None;

        for mode in [ProtocolMode::Framed, ProtocolMode::LineDelimited] {
            let client = StdioMcpClient::new(
                command,
                &config.launch_args,
                &config.launch_env,
                config.working_dir.as_deref(),
            )
            .await?;

            client.set_mode(mode).await;

            match client.initialize(config.startup_timeout_ms).await {
                Ok(_) => {
                    pid = client.pid();
                    selected_client = Some(client);
                    break;
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    client.shutdown().await;
                }
            }
        }

        let stdio_client = match selected_client {
            Some(client) => client,
            None => {
                return Err(format!(
                    "Failed stdio initialize in framed and line-delimited modes{}",
                    last_error
                        .map(|e| format!(" (last error: {})", e))
                        .unwrap_or_default()
                )
                .into())
            }
        };

        let transport_client = MCPTransportClient::Stdio(stdio_client);
        let tools = self.discover_tools(&transport_client, &config.id).await?;

        let endpoint = format!("stdio://{}", config.id);
        let mcp_client = MCPClient {
            transport_client,
            url: endpoint.clone(),
        };

        {
            let mut clients = self.clients.write().await;
            clients.insert(config.id.clone(), mcp_client);
        }

        {
            let mut status_map = self.server_status.write().await;
            status_map.insert(
                config.id.clone(),
                MCPServerStatus {
                    id: config.id.clone(),
                    name: config.name.clone(),
                    transport: "stdio".to_string(),
                    status: ConnectionStatus::Connected,
                    tools,
                    last_error: None,
                    managed_process: true,
                    pid,
                    endpoint: Some(endpoint),
                },
            );
        }

        Ok(())
    }

    pub async fn disconnect_server(&self, server_id: &str) {
        let removed = {
            let mut clients = self.clients.write().await;
            clients.remove(server_id)
        };

        if let Some(client) = removed {
            if let MCPTransportClient::Stdio(stdio_client) = client.transport_client {
                stdio_client.shutdown().await;
            }
        }

        self.stop_managed_process(server_id).await;

        {
            let mut status_map = self.server_status.write().await;
            if let Some(status) = status_map.get_mut(server_id) {
                status.status = ConnectionStatus::Disconnected;
                status.tools.clear();
                status.last_error = None;
                status.managed_process = false;
                status.pid = None;
            }
        }
    }

    pub async fn execute_tool(&self, call: &MCPToolCall) -> MCPToolResult {
        let clients = self.clients.read().await;

        let Some(client) = clients.get(&call.server_id) else {
            return MCPToolResult {
                success: false,
                result: serde_json::Value::Null,
                error: Some(format!("Server {} not connected", call.server_id)),
            };
        };

        let call_result = tokio::time::timeout(
            Duration::from_secs(60),
            self.execute_transport_tool(client, &call.tool_name, call.parameters.clone()),
        )
        .await;

        match call_result {
            Ok(Ok(response)) => {
                if let Some(error) = response.get("error") {
                    MCPToolResult {
                        success: false,
                        result: serde_json::Value::Null,
                        error: Some(format!("Tool execution error: {}", error)),
                    }
                } else if let Some(result) = response.get("result") {
                    MCPToolResult {
                        success: true,
                        result: result.clone(),
                        error: None,
                    }
                } else {
                    MCPToolResult {
                        success: false,
                        result: serde_json::Value::Null,
                        error: Some("Invalid response format".to_string()),
                    }
                }
            }
            Ok(Err(e)) => MCPToolResult {
                success: false,
                result: serde_json::Value::Null,
                error: Some(format!("Tool execution failed: {}", e)),
            },
            Err(_) => MCPToolResult {
                success: false,
                result: serde_json::Value::Null,
                error: Some("Tool execution timed out after 60 seconds".to_string()),
            },
        }
    }

    pub async fn get_all_tools(&self) -> Vec<MCPTool> {
        let status_map = self.server_status.read().await;
        let mut tools = Vec::new();
        for status in status_map.values() {
            if matches!(status.status, ConnectionStatus::Connected) {
                tools.extend(status.tools.clone());
            }
        }
        tools
    }

    pub async fn get_server_statuses(&self) -> Vec<MCPServerStatus> {
        let status_map = self.server_status.read().await;
        status_map.values().cloned().collect()
    }

    async fn execute_transport_tool(
        &self,
        client: &MCPClient,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        match &client.transport_client {
            MCPTransportClient::Http(http) => http.call_tool(tool_name, Some(params)).await,
            MCPTransportClient::Stdio(stdio) => stdio.call_tool(tool_name, Some(params)).await,
        }
    }

    async fn discover_tools(
        &self,
        client: &MCPTransportClient,
        server_id: &str,
    ) -> Result<Vec<MCPTool>, Box<dyn std::error::Error + Send + Sync>> {
        let tools_response = match client {
            MCPTransportClient::Http(http) => http.list_tools().await?,
            MCPTransportClient::Stdio(stdio) => stdio.list_tools().await?,
        };

        let mut mcp_tools = Vec::new();
        if let Some(result) = tools_response.get("result") {
            if let Some(tools_array) = result.get("tools") {
                if let Some(tools) = tools_array.as_array() {
                    for tool in tools {
                        let name = tool
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let description = tool
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input_schema = tool
                            .get("inputSchema")
                            .cloned()
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                        mcp_tools.push(MCPTool {
                            server_id: server_id.to_string(),
                            name,
                            description,
                            input_schema,
                        });
                    }
                }
            }
        }

        Ok(mcp_tools)
    }

    async fn update_status_error(&self, server_id: &str, error: String) {
        let mut status_map = self.server_status.write().await;
        if let Some(status) = status_map.get_mut(server_id) {
            status.status = ConnectionStatus::Error;
            status.last_error = Some(error);
            status.tools.clear();
            status.pid = None;
        }
    }

    async fn start_managed_process_if_needed(
        &self,
        config: &MCPServerConfig,
    ) -> Result<Option<u32>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(command) = config
            .launch_command
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            return Ok(None);
        };

        {
            let mut processes = self.managed_processes.write().await;
            if let Some(proc) = processes.get_mut(&config.id) {
                if proc.child.try_wait()?.is_none() {
                    return Ok(proc.pid);
                }
                processes.remove(&config.id);
            }
        }

        let mut cmd = Command::new(command);
        if !config.launch_args.is_empty() {
            cmd.args(&config.launch_args);
        }
        if !config.launch_env.is_empty() {
            cmd.envs(&config.launch_env);
        }
        if let Some(dir) = config
            .working_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if !Path::new(dir).exists() {
                return Err(format!("Working directory not found: {}", dir).into());
            }
            cmd.current_dir(dir);
        }

        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start MCP process '{}': {}", command, e))?;
        let pid = child.id();

        sleep(Duration::from_millis(250)).await;
        if let Some(status) = child.try_wait()? {
            return Err(format!("MCP process exited early with status {}", status).into());
        }

        let mut processes = self.managed_processes.write().await;
        processes.insert(config.id.clone(), ManagedProcess { child, pid });
        Ok(pid)
    }

    async fn stop_managed_process(&self, server_id: &str) {
        let maybe_proc = {
            let mut processes = self.managed_processes.write().await;
            processes.remove(server_id)
        };
        if let Some(mut proc) = maybe_proc {
            let _ = proc.child.kill().await;
            let _ = proc.child.wait().await;
        }
    }

    async fn initialize_http_with_retry(
        &self,
        client: &mut HttpMcpClient,
        startup_timeout_ms: Option<u64>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timeout = Duration::from_millis(startup_timeout_ms.unwrap_or(20_000));
        let started = Instant::now();
        let mut last_error: Option<String> = None;

        while started.elapsed() < timeout {
            match client.initialize().await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_error = Some(e.to_string());
                    sleep(Duration::from_millis(600)).await;
                }
            }
        }

        Err(format!(
            "Timed out after {} ms waiting for MCP server initialization{}",
            timeout.as_millis(),
            last_error
                .map(|e| format!(" (last error: {})", e))
                .unwrap_or_default()
        )
        .into())
    }

    async fn perform_oauth_flow(
        &self,
        client_id: &str,
        client_secret: &str,
        server_url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut oauth_url = if server_url.ends_with("/mcp") {
            server_url.trim_end_matches("/mcp").to_string()
        } else {
            server_url.to_string()
        };

        if !oauth_url.ends_with('/') {
            oauth_url.push('/');
        }
        oauth_url.push_str("oauth/token");

        let client = reqwest::Client::new();
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];

        let response = match tokio::time::timeout(
            Duration::from_secs(15),
            client
                .post(&oauth_url)
                .form(&params)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .send(),
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return Err(format!("OAuth request failed: {}", e).into());
            }
            Err(_) => {
                return Err("OAuth request timed out after 15 seconds".into());
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("OAuth request failed: {} - {}", status, error_text).into());
        }

        let oauth_response: serde_json::Value = response.json().await?;
        if let Some(access_token) = oauth_response.get("access_token").and_then(|v| v.as_str()) {
            Ok(format!("Bearer {}", access_token))
        } else {
            Err("No access_token in OAuth response".into())
        }
    }
}

impl Default for MCPManager {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_transport(transport: &str) -> String {
    match transport.trim().to_lowercase().as_str() {
        "stdio" => "stdio".to_string(),
        _ => "http".to_string(),
    }
}
