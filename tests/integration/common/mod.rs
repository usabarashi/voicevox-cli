use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

/// Expected MCP protocol version supported by rmcp 0.8.x
/// This is determined by the rmcp crate version in Cargo.toml
pub const EXPECTED_PROTOCOL_VERSION: &str = "2024-11-05";

/// MCP JSON-RPC request
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    #[allow(dead_code)]
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.into(),
            params: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }

    #[allow(dead_code)]
    pub fn with_params(mut self, params: Value) -> Self {
        self.params = Some(params);
        self
    }
}

/// MCP JSON-RPC response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

/// MCP client for testing
#[allow(dead_code)]
pub struct McpClient {
    process: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl McpClient {
    /// Start MCP server process
    #[allow(dead_code)]
    pub fn start(server_path: &str) -> Result<Self> {
        let mut process = Command::new(server_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Inherit stderr for debugging
            .spawn()
            .context("Failed to spawn MCP server")?;

        let stdin = process.stdin.take().context("Failed to get stdin")?;
        let stdout = BufReader::new(process.stdout.take().context("Failed to get stdout")?);

        Ok(Self {
            process,
            stdin,
            stdout,
        })
    }

    /// Send JSON-RPC request
    #[allow(dead_code)]
    pub fn send(&mut self, request: &JsonRpcRequest) -> Result<()> {
        let json = serde_json::to_string(request).context("Failed to serialize request")?;
        writeln!(self.stdin, "{}", json).context("Failed to write request")?;
        self.stdin.flush().context("Failed to flush stdin")?;
        Ok(())
    }

    /// Read JSON-RPC response
    #[allow(dead_code)]
    pub fn read(&mut self) -> Result<JsonRpcResponse> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .context("Failed to read response")?;

        if line.is_empty() {
            anyhow::bail!("Server closed connection");
        }

        serde_json::from_str(&line).context("Failed to parse JSON response")
    }

    /// Send request and read response
    #[allow(dead_code)]
    pub fn call(&mut self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        self.send(request)?;
        self.read()
    }

    /// Initialize MCP session
    #[allow(dead_code)]
    pub fn initialize(&mut self) -> Result<JsonRpcResponse> {
        self.initialize_with_version(EXPECTED_PROTOCOL_VERSION)
    }

    /// Initialize MCP session with specific protocol version
    #[allow(dead_code)]
    pub fn initialize_with_version(&mut self, protocol_version: &str) -> Result<JsonRpcResponse> {
        let params = serde_json::json!({
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": {
                "name": "integration-test",
                "version": "1.0"
            }
        });

        let request = JsonRpcRequest::new("initialize")
            .with_id(1)
            .with_params(params);

        let response = self.call(&request)?;

        // Send initialized notification
        let initialized = JsonRpcRequest::new("notifications/initialized");
        self.send(&initialized)?;

        Ok(response)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Get path to MCP server binary
#[allow(dead_code)]
pub fn get_server_path() -> String {
    std::env::var("MCP_SERVER_PATH").unwrap_or_else(|_| {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{}/target/debug/voicevox-mcp-server", manifest_dir)
    })
}

/// Check if daemon is running (Unix-specific: uses pgrep)
#[cfg(unix)]
#[allow(dead_code)]
pub fn is_daemon_running() -> bool {
    std::process::Command::new("pgrep")
        .arg("-f")
        .arg("voicevox-daemon")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
