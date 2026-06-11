//! MCP (Model Context Protocol) server mode.
//!
//! Implements JSON-RPC 2.0 over stdio for IDE integration.
//! Exposes all axga tools as MCP tools.

use axga_core::tools::registry::ToolRegistry;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use tracing::{debug, info};

pub async fn run_mcp_server(
    _provider: &str,
    _api_key: Option<&str>,
    _model: &str,
    registry: &ToolRegistry,
) -> anyhow::Result<()> {
    info!("MCP server starting (stdio transport)");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin);

    let mut initialized = false;
    let server_name = String::from("axga");
    let server_version = String::from(env!("CARGO_PKG_VERSION"));

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                send_error(&mut stdout, None, -32700, &format!("Parse error: {e}"));
                continue;
            }
        };

        let method = request["method"].as_str().unwrap_or("");
        let id = request.get("id").cloned();

        debug!(%method, "MCP request");

        match method {
            "initialize" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": server_name,
                            "version": server_version
                        }
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                initialized = true;
                info!("MCP initialized");
            }

            "notifications/initialized" => {
                // No response needed for notifications
                debug!("MCP client ready");
            }

            "tools/list" => {
                if !initialized {
                    send_error(&mut stdout, id, -32002, "Not initialized");
                    continue;
                }

                let tools: Vec<Value> = registry
                    .names()
                    .filter_map(|name| registry.get(name))
                    .map(|tool| {
                        json!({
                            "name": tool.name(),
                            "description": tool.description(),
                            "inputSchema": tool.parameters()
                        })
                    })
                    .collect();

                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                });
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
            }

            "tools/call" => {
                if !initialized {
                    send_error(&mut stdout, id, -32002, "Not initialized");
                    continue;
                }

                let tool_name = request["params"]["name"].as_str().unwrap_or("");
                let arguments = request["params"]["arguments"].clone();

                match registry.get(tool_name) {
                    Some(tool) => {
                        let result = tool.execute(arguments).await;
                        match result {
                            Ok(content) => {
                                let response = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": [{
                                            "type": "text",
                                            "text": content
                                        }]
                                    }
                                });
                                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                            }
                            Err(e) => {
                                let response = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": [{
                                            "type": "text",
                                            "text": format!("Error: {}", e)
                                        }],
                                        "isError": true
                                    }
                                });
                                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                            }
                        }
                        stdout.flush()?;
                    }
                    None => {
                        send_error(
                            &mut stdout,
                            id,
                            -32602,
                            &format!("Unknown tool: {tool_name}"),
                        );
                    }
                }
            }

            "ping" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                });
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
            }

            _ => {
                send_error(&mut stdout, id, -32601, &format!("Method not found: {method}"));
            }
        }
    }

    info!("MCP server stopped");
    Ok(())
}

fn send_error(stdout: &mut impl Write, id: Option<Value>, code: i64, message: &str) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    });
    let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_default());
    let _ = stdout.flush();
}
