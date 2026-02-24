import { Component, For, createSignal, onMount, onCleanup, createMemo } from "solid-js";
import {
  MCPServerConfig,
  MCPServerStatus,
  listMCPServers,
  saveMCPServer,
  deleteMCPServer,
  connectMCPServer,
  disconnectMCPServer,
  getMCPServerStatuses
} from "../lib/mcp-api";
import "./MCPSettings.css";

interface MCPSettingsProps {
  onClose: () => void;
}

const MCPSettings: Component<MCPSettingsProps> = (props) => {
  const [servers, setServers] = createSignal<MCPServerConfig[]>([]);
  const [statuses, setStatuses] = createSignal<MCPServerStatus[]>([]);
  const [showAddForm, setShowAddForm] = createSignal(false);
  const [editingServer, setEditingServer] = createSignal<MCPServerConfig | null>(null);
  const [loading, setLoading] = createSignal(false);

  // Form state
  const [formData, setFormData] = createSignal({
    name: "",
    serverUrl: "",
    launchCommand: "",
    launchArgs: "",
    launchEnv: "",
    workingDir: "",
    startupTimeoutMs: "20000",
    oauthClientId: "",
    oauthClientSecret: "",
  });

  const mergedData = createMemo(() => {
    const statusMap = new Map(statuses().map(s => [s.id, s]));
    return servers().map(server => ({
      server,
      status: statusMap.get(server.id)
    }));
  });

  onMount(async () => {
    await refreshData();
    const timer = window.setInterval(() => {
      void refreshData();
    }, 5000);
    onCleanup(() => window.clearInterval(timer));
  });

  const refreshData = async () => {
    try {
      setLoading(true);
      const [serverList, statusList] = await Promise.all([
        listMCPServers(),
        getMCPServerStatuses()
      ]);
      setServers(serverList);
      setStatuses(statusList);
    } catch (err) {
      console.error("Failed to load MCP data:", err);
    } finally {
      setLoading(false);
    }
  };

  const resetForm = () => {
    setFormData({
      name: "",
      serverUrl: "",
      launchCommand: "",
      launchArgs: "",
      launchEnv: "",
      workingDir: "",
      startupTimeoutMs: "20000",
      oauthClientId: "",
      oauthClientSecret: "",
    });
    setEditingServer(null);
    setShowAddForm(false);
  };

  const startEdit = (server: MCPServerConfig) => {
    setFormData({
      name: server.name,
      serverUrl: server.server_url || "",
      launchCommand: server.launch_command || "",
      launchArgs: (server.launch_args || []).join(" "),
      launchEnv: Object.keys(server.launch_env || {}).length > 0
        ? JSON.stringify(server.launch_env, null, 2)
        : "",
      workingDir: server.working_dir || "",
      startupTimeoutMs: String(server.startup_timeout_ms ?? 20000),
      oauthClientId: server.oauth_client_id || "",
      oauthClientSecret: server.oauth_client_secret || "",
    });
    setEditingServer(server);
    setShowAddForm(true);
  };

  const handleSave = async () => {
    try {
      const data = formData();

      if (!data.name.trim()) {
        alert("Server name is required");
        return;
      }

      if (!data.serverUrl.trim()) {
        alert("Server URL is required");
        return;
      }

      let parsedEnv: Record<string, string> = {};
      if (data.launchEnv.trim()) {
        try {
          const raw = JSON.parse(data.launchEnv);
          if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
            alert("Launch env must be a JSON object");
            return;
          }
          parsedEnv = Object.fromEntries(
            Object.entries(raw).map(([k, v]) => [k, String(v)])
          );
        } catch {
          alert("Launch env JSON is invalid");
          return;
        }
      }

      const parsedTimeout = Number.parseInt(data.startupTimeoutMs.trim(), 10);
      if (Number.isNaN(parsedTimeout) || parsedTimeout < 1000 || parsedTimeout > 120000) {
        alert("Startup timeout must be between 1000 and 120000 ms");
        return;
      }

      const launchArgs = data.launchArgs
        .split(/\s+/)
        .map((s) => s.trim())
        .filter(Boolean);

      const config: MCPServerConfig = {
        id: editingServer()?.id || crypto.randomUUID(),
        name: data.name,
        transport: "http",
        server_url: data.serverUrl,
        launch_command: data.launchCommand.trim() || undefined,
        launch_args: launchArgs,
        launch_env: parsedEnv,
        working_dir: data.workingDir.trim() || undefined,
        startup_timeout_ms: parsedTimeout,
        oauth_client_id: data.oauthClientId.trim() || undefined,
        oauth_client_secret: data.oauthClientSecret.trim() || undefined,
        enabled: editingServer()?.enabled ?? true,
        created_at: editingServer()?.created_at || new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };

      await saveMCPServer(config);
      await refreshData();
      resetForm();
    } catch (err) {
      console.error("Failed to save server:", err);
      alert("Failed to save server configuration");
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this MCP server?")) {
      return;
    }

    try {
      await deleteMCPServer(id);
      await refreshData();
    } catch (err) {
      console.error("Failed to delete server:", err);
      alert("Failed to delete server");
    }
  };

  const handleToggleConnection = async (server: MCPServerConfig, currentStatus?: MCPServerStatus) => {
    try {
      if (currentStatus?.status === "Connected") {
        await disconnectMCPServer(server.id);
      } else {
        await connectMCPServer(server.id);
      }
      await refreshData();
    } catch (err) {
      console.error("Failed to toggle connection:", err);
      alert("Failed to connect/disconnect server");
    }
  };

  const getStatusColor = (status?: string) => {
    switch (status) {
      case "Connected": return "green";
      case "Connecting": return "orange";
      case "Error": return "red";
      default: return "gray";
    }
  };

  return (
    <div class="mcp-settings">
      <div class="mcp-settings-header">
        <h2>MCP Settings</h2>
        <div class="header-actions">
          <button class="add-btn" onClick={() => setShowAddForm(true)}>
            Add Server
          </button>
          <button class="refresh-btn" onClick={refreshData} disabled={loading()}>
            {loading() ? "Loading..." : "Refresh"}
          </button>
          <button class="close-btn" onClick={props.onClose}>
            Close
          </button>
        </div>
      </div>

      <div class="mcp-settings-content">
        {showAddForm() && (
          <div class="add-form">
            <h3>{editingServer() ? "Edit Server" : "Add MCP Server"}</h3>

            <div class="form-group">
              <label>Name</label>
              <input
                type="text"
                value={formData().name}
                onInput={(e) => setFormData(prev => ({ ...prev, name: e.currentTarget.value }))}
                placeholder="Server name"
              />
            </div>

            <div class="form-group">
              <label>Remote MCP server URL</label>
              <input
                type="url"
                value={formData().serverUrl}
                onInput={(e) => setFormData(prev => ({ ...prev, serverUrl: e.currentTarget.value }))}
                placeholder="https://your-mcp-server.com"
              />
              <small class="hint">
                Endpoint used for MCP HTTP transport (for local managed servers, point to localhost URL).
              </small>
            </div>

            <details class="advanced-settings">
              <summary>Managed local server launch (optional)</summary>
              <div class="advanced-content">
                <div class="form-group">
                  <label>Launch command</label>
                  <input
                    type="text"
                    value={formData().launchCommand}
                    onInput={(e) => setFormData(prev => ({ ...prev, launchCommand: e.currentTarget.value }))}
                    placeholder="npx"
                  />
                </div>

                <div class="form-group">
                  <label>Launch args (space-separated)</label>
                  <input
                    type="text"
                    value={formData().launchArgs}
                    onInput={(e) => setFormData(prev => ({ ...prev, launchArgs: e.currentTarget.value }))}
                    placeholder="-y @modelcontextprotocol/server-filesystem C:\\workspace"
                  />
                </div>

                <div class="form-group">
                  <label>Working directory (optional)</label>
                  <input
                    type="text"
                    value={formData().workingDir}
                    onInput={(e) => setFormData(prev => ({ ...prev, workingDir: e.currentTarget.value }))}
                    placeholder="C:\\path\\to\\project"
                  />
                </div>

                <div class="form-group">
                  <label>Launch env JSON (optional)</label>
                  <textarea
                    rows={4}
                    value={formData().launchEnv}
                    onInput={(e) => setFormData(prev => ({ ...prev, launchEnv: e.currentTarget.value }))}
                    placeholder={'{"NODE_ENV":"production"}'}
                  />
                </div>

                <div class="form-group">
                  <label>Startup timeout (ms)</label>
                  <input
                    type="number"
                    min="1000"
                    max="120000"
                    value={formData().startupTimeoutMs}
                    onInput={(e) => setFormData(prev => ({ ...prev, startupTimeoutMs: e.currentTarget.value }))}
                  />
                </div>
              </div>
            </details>

            <details class="advanced-settings">
              <summary>Advanced settings</summary>
              <div class="advanced-content">
                <div class="form-group">
                  <label>OAuth Client ID (optional)</label>
                  <input
                    type="text"
                    value={formData().oauthClientId}
                    onInput={(e) => setFormData(prev => ({ ...prev, oauthClientId: e.currentTarget.value }))}
                    placeholder="your-oauth-client-id"
                  />
                </div>

                <div class="form-group">
                  <label>OAuth Client Secret (optional)</label>
                  <input
                    type="password"
                    value={formData().oauthClientSecret}
                    onInput={(e) => setFormData(prev => ({ ...prev, oauthClientSecret: e.currentTarget.value }))}
                    placeholder="your-oauth-client-secret"
                  />
                </div>
              </div>
            </details>

            <div class="warning-text">
              <strong>Security Notice:</strong> Only use connectors from developers you trust.
              MCP servers have access to tools and data as configured, and this app cannot verify
              that they will work as intended or that they won't change.
            </div>

            <div class="form-actions">
              <button class="save-btn" onClick={handleSave}>
                {editingServer() ? "Update" : "Add"}
              </button>
              <button class="cancel-btn" onClick={resetForm}>Cancel</button>
            </div>
          </div>
        )}

        <div class="servers-list">
          <h3>MCP Servers</h3>

          {mergedData().length === 0 ? (
            <div class="empty-state">
              <p>No MCP servers configured.</p>
              <p>Add your first server to get started with MCP tools.</p>
            </div>
          ) : (
            <div class="servers-grid">
              <For each={mergedData()}>
                {({ server, status }) => (
                  <div class="server-card">
                    <div class="server-header">
                      <div class="server-info">
                        <h4>{server.name}</h4>
                        <p>{server.server_url}</p>
                      </div>
                      <div class="server-status">
                        <span
                          class={`status-badge ${getStatusColor(status?.status)}`}
                          title={status?.last_error}
                        >
                          {status?.status || "Disconnected"}
                        </span>
                      </div>
                    </div>

                    <div class="server-details">
                      <div class="detail-row">
                        <strong>URL:</strong> {server.server_url}
                      </div>

                      {server.launch_command && (
                        <div class="detail-row">
                          <strong>Launch:</strong> {server.launch_command} {(server.launch_args || []).join(" ")}
                        </div>
                      )}

                      {server.oauth_client_id && (
                        <div class="detail-row">
                          <strong>OAuth:</strong> Configured
                        </div>
                      )}

                      {status?.managed_process && (
                        <div class="detail-row">
                          <strong>Process:</strong> PID {status.pid || "N/A"}
                        </div>
                      )}

                      {status?.endpoint && (
                        <div class="detail-row">
                          <strong>Endpoint:</strong> {status.endpoint}
                        </div>
                      )}

                      {status?.last_error && (
                        <div class="detail-row error-row">
                          <strong>Last error:</strong> {status.last_error}
                        </div>
                      )}

                      {status?.tools && status.tools.length > 0 && (
                        <div class="detail-row">
                          <strong>Tools:</strong> {status.tools.map(t => t.name).join(", ")}
                        </div>
                      )}
                    </div>

                    <div class="server-actions">
                      <button
                        class={`toggle-btn ${status?.status === "Connected" ? "disconnect" : "connect"}`}
                        onClick={() => handleToggleConnection(server, status)}
                        disabled={status?.status === "Connecting"}
                      >
                        {status?.status === "Connected" ? "Disconnect" :
                         status?.status === "Connecting" ? "Connecting..." : "Connect"}
                      </button>
                      <button class="edit-btn" onClick={() => startEdit(server)}>
                        Edit
                      </button>
                      <button class="delete-btn" onClick={() => handleDelete(server.id)}>
                        Delete
                      </button>
                    </div>
                  </div>
                )}
              </For>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default MCPSettings;
