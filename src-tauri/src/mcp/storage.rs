use super::types::MCPServerConfig;
use crate::database::{Database, DbError};
use rusqlite::params;
use std::collections::HashMap;

impl Database {
    pub fn create_mcp_tables(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::Lock)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                transport TEXT NOT NULL DEFAULT 'http',
                server_url TEXT NOT NULL,
                launch_command TEXT,
                launch_args_json TEXT,
                launch_env_json TEXT,
                working_dir TEXT,
                startup_timeout_ms INTEGER,
                oauth_client_id TEXT,
                oauth_client_secret TEXT,
                enabled BOOLEAN NOT NULL DEFAULT 0,
                created_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP NOT NULL
            )",
            [],
        )?;

        // Lightweight migrations for existing databases.
        add_column_if_missing(&conn, "mcp_servers", "transport", "TEXT NOT NULL DEFAULT 'http'")?;
        add_column_if_missing(&conn, "mcp_servers", "launch_command", "TEXT")?;
        add_column_if_missing(&conn, "mcp_servers", "launch_args_json", "TEXT")?;
        add_column_if_missing(&conn, "mcp_servers", "launch_env_json", "TEXT")?;
        add_column_if_missing(&conn, "mcp_servers", "working_dir", "TEXT")?;
        add_column_if_missing(&conn, "mcp_servers", "startup_timeout_ms", "INTEGER")?;

        Ok(())
    }

    pub fn save_mcp_server(&self, config: &MCPServerConfig) -> Result<(), DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::Lock)?;

        conn.execute(
            "INSERT OR REPLACE INTO mcp_servers
             (id, name, transport, server_url, launch_command, launch_args_json, launch_env_json, working_dir, startup_timeout_ms, oauth_client_id, oauth_client_secret, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                config.id,
                config.name,
                config.transport,
                config.server_url,
                config.launch_command,
                serde_json::to_string(&config.launch_args).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&config.launch_env).unwrap_or_else(|_| "{}".to_string()),
                config.working_dir,
                config.startup_timeout_ms,
                config.oauth_client_id,
                config.oauth_client_secret,
                config.enabled,
                config.created_at,
                config.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_mcp_servers(&self) -> Result<Vec<MCPServerConfig>, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::Lock)?;

        let mut stmt = conn.prepare(
            "SELECT id, name, transport, server_url, launch_command, launch_args_json, launch_env_json, working_dir, startup_timeout_ms, oauth_client_id, oauth_client_secret, enabled, created_at, updated_at
             FROM mcp_servers ORDER BY name"
        )?;

        let server_iter = stmt.query_map([], |row| {
            let launch_args_json: Option<String> = row.get(5)?;
            let launch_env_json: Option<String> = row.get(6)?;
            Ok(MCPServerConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                transport: row.get(2)?,
                server_url: row.get(3)?,
                launch_command: row.get(4)?,
                launch_args: parse_json_vec(launch_args_json),
                launch_env: parse_json_map(launch_env_json),
                working_dir: row.get(7)?,
                startup_timeout_ms: row.get(8)?,
                oauth_client_id: row.get(9)?,
                oauth_client_secret: row.get(10)?,
                enabled: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;

        let mut servers = Vec::new();
        for server in server_iter {
            servers.push(server?);
        }
        Ok(servers)
    }

    pub fn get_mcp_server(&self, id: &str) -> Result<Option<MCPServerConfig>, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::Lock)?;

        let mut stmt = conn.prepare(
            "SELECT id, name, transport, server_url, launch_command, launch_args_json, launch_env_json, working_dir, startup_timeout_ms, oauth_client_id, oauth_client_secret, enabled, created_at, updated_at
             FROM mcp_servers WHERE id = ?1"
        )?;

        let mut server_iter = stmt.query_map([id], |row| {
            let launch_args_json: Option<String> = row.get(5)?;
            let launch_env_json: Option<String> = row.get(6)?;
            Ok(MCPServerConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                transport: row.get(2)?,
                server_url: row.get(3)?,
                launch_command: row.get(4)?,
                launch_args: parse_json_vec(launch_args_json),
                launch_env: parse_json_map(launch_env_json),
                working_dir: row.get(7)?,
                startup_timeout_ms: row.get(8)?,
                oauth_client_id: row.get(9)?,
                oauth_client_secret: row.get(10)?,
                enabled: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;

        match server_iter.next() {
            Some(server) => Ok(Some(server?)),
            None => Ok(None),
        }
    }

    pub fn delete_mcp_server(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::Lock)?;

        conn.execute(
            "DELETE FROM mcp_servers WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn update_mcp_server_enabled(&self, id: &str, enabled: bool) -> Result<(), DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::Lock)?;

        conn.execute(
            "UPDATE mcp_servers SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled, chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }
}

fn add_column_if_missing(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), DbError> {
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition);
    match conn.execute(&sql, []) {
        Ok(_) => Ok(()),
        Err(err) => {
            let msg = err.to_string().to_lowercase();
            if msg.contains("duplicate column name") {
                Ok(())
            } else {
                Err(err.into())
            }
        }
    }
}

fn parse_json_vec(value: Option<String>) -> Vec<String> {
    value
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
}

fn parse_json_map(value: Option<String>) -> HashMap<String, String> {
    value
        .and_then(|raw| serde_json::from_str::<HashMap<String, String>>(&raw).ok())
        .unwrap_or_default()
}
