import { invoke } from "@tauri-apps/api/core";

export interface MCPServerConfig {
  id: string;
  name: string;
  transport: "http";
  server_url: string;
  launch_command?: string;
  launch_args: string[];
  launch_env: Record<string, string>;
  working_dir?: string;
  startup_timeout_ms?: number;
  oauth_client_id?: string;
  oauth_client_secret?: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface MCPTool {
  server_id: string;
  name: string;
  description: string;
  input_schema: any;
}

export interface MCPServerStatus {
  id: string;
  name: string;
  transport: string;
  status: "Connected" | "Disconnected" | "Connecting" | "Error";
  tools: MCPTool[];
  last_error?: string;
  managed_process: boolean;
  pid?: number;
  endpoint?: string;
}

export interface MCPToolCall {
  server_id: string;
  tool_name: string;
  parameters: any;
}

export interface MCPToolResult {
  success: boolean;
  result: any;
  error?: string;
}

export async function listMCPServers(): Promise<MCPServerConfig[]> {
  return invoke("list_mcp_servers");
}

export async function saveMCPServer(config: MCPServerConfig): Promise<void> {
  return invoke("save_mcp_server", { config });
}

export async function deleteMCPServer(id: string): Promise<void> {
  return invoke("delete_mcp_server", { id });
}

export async function connectMCPServer(id: string): Promise<void> {
  return invoke("connect_mcp_server", { id });
}

export async function disconnectMCPServer(id: string): Promise<void> {
  return invoke("disconnect_mcp_server", { id });
}

export async function getMCPServerStatuses(): Promise<MCPServerStatus[]> {
  return invoke("get_mcp_server_statuses");
}

export async function executeMCPTool(call: MCPToolCall): Promise<MCPToolResult> {
  return invoke("execute_mcp_tool", { call });
}
