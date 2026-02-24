use crate::agent::ToolDefinition;
use serde_json::json;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "bash".to_string(),
        description: "Execute a shell command. Use for running builds, tests, git commands, etc. Commands run in a sandboxed environment with timeouts.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command (optional)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60, max: 300)"
                }
            },
            "required": ["command"]
        }),
    }
}

// Dangerous commands that should be blocked (cross-platform baseline)
const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    ":(){ :|:& };:",  // Fork bomb
    "> /dev/sda",
    "mkfs.",
    "dd if=",
    "wget | sh",
    "curl | sh",
    "wget | bash",
    "curl | bash",
];

#[cfg(target_os = "windows")]
const WINDOWS_BLOCKED_PATTERNS: &[&str] = &[
    "remove-item -recurse -force c:\\",
    "remove-item -recurse -force /",
    "format-volume",
    "diskpart",
    "reg delete hk",
    "bcdedit /delete",
];

pub fn execute(
    input: &serde_json::Value,
    project_path: Option<&str>,
) -> Result<String, String> {
    let command = input
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'command' parameter")?;

    let cwd = input
        .get("cwd")
        .and_then(|v| v.as_str())
        .or(project_path);

    let timeout_secs = input
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(60)
        .min(300);

    let command_lower = command.to_lowercase();

    // Security check
    for pattern in BLOCKED_PATTERNS {
        if command_lower.contains(&pattern.to_lowercase()) {
            return Err(format!(
                "Command blocked for safety: contains dangerous pattern '{}'",
                pattern
            ));
        }
    }

    #[cfg(target_os = "windows")]
    for pattern in WINDOWS_BLOCKED_PATTERNS {
        if command_lower.contains(pattern) {
            return Err(format!(
                "Command blocked for safety: contains dangerous Windows pattern '{}'",
                pattern
            ));
        }
    }

    // Build command with OS-appropriate shell
    let mut cmd = build_shell_command(command);
    let shell_name = shell_name();

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Execute with timeout
    let child = cmd.spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    let output = wait_with_timeout(child, Duration::from_secs(timeout_secs))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    // Format output
    let mut result = format!("[shell: {}]\n", shell_name);

    if !stdout.is_empty() {
        result.push_str(&stdout);
    }

    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str("[stderr]\n");
        result.push_str(&stderr);
    }

    result.push_str(&format!("\n[exit code: {}]", exit_code));

    // Truncate if too long
    if result.len() > 50000 {
        result = format!(
            "{}...\n\n[Output truncated. Total length: {} chars]",
            &result[..50000],
            result.len()
        );
    }

    if result.is_empty() {
        result = "[Command completed with no output]".to_string();
    }

    Ok(result)
}

fn build_shell_command(command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("powershell");
        cmd.arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(command);
        cmd
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

fn shell_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "powershell"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "sh"
    }
}

fn wait_with_timeout(
    child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    use std::thread;
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => {
            let _ = handle.join();
            result.map_err(|e| format!("Command failed: {}", e))
        }
        Err(_) => {
            // Timeout - try to kill the process
            Err(format!(
                "Command timed out after {} seconds",
                timeout.as_secs()
            ))
        }
    }
}
