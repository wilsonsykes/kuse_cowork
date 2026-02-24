use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub launch_command: Option<String>,
    #[serde(default)]
    pub launch_args: Vec<String>,
    #[serde(default)]
    pub launch_env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub startup_timeout_ms: Option<u64>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerStatus {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub status: ConnectionStatus,
    pub tools: Vec<MCPTool>,
    pub last_error: Option<String>,
    pub managed_process: bool,
    pub pid: Option<u32>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolCall {
    pub server_id: String,
    pub tool_name: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolResult {
    pub success: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

fn default_transport() -> String {
    "http".to_string()
}
