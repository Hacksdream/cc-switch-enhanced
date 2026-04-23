#![allow(non_snake_case)]

use futures::StreamExt;
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::State;

use crate::app_config::{AppType, McpApps, McpServer};
use crate::claude_mcp;
use crate::services::McpService;
use crate::store::AppState;

/// 获取 Claude MCP 状态
#[tauri::command]
pub async fn get_claude_mcp_status() -> Result<claude_mcp::McpStatus, String> {
    claude_mcp::get_mcp_status().map_err(|e| e.to_string())
}

/// 读取 mcp.json 文本内容
#[tauri::command]
pub async fn read_claude_mcp_config() -> Result<Option<String>, String> {
    claude_mcp::read_mcp_json().map_err(|e| e.to_string())
}

/// 新增或更新一个 MCP 服务器条目
#[tauri::command]
pub async fn upsert_claude_mcp_server(id: String, spec: serde_json::Value) -> Result<bool, String> {
    claude_mcp::upsert_mcp_server(&id, spec).map_err(|e| e.to_string())
}

/// 删除一个 MCP 服务器条目
#[tauri::command]
pub async fn delete_claude_mcp_server(id: String) -> Result<bool, String> {
    claude_mcp::delete_mcp_server(&id).map_err(|e| e.to_string())
}

/// 校验命令是否在 PATH 中可用（不执行）
#[tauri::command]
pub async fn validate_mcp_command(cmd: String) -> Result<bool, String> {
    claude_mcp::validate_command_in_path(&cmd).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct McpConfigResponse {
    pub config_path: String,
    pub servers: HashMap<String, serde_json::Value>,
}

/// 获取 MCP 配置（来自 ~/.cc-switch/config.json）
use std::str::FromStr;

#[tauri::command]
#[allow(deprecated)] // 兼容层命令，内部调用已废弃的 Service 方法
pub async fn get_mcp_config(
    state: State<'_, AppState>,
    app: String,
) -> Result<McpConfigResponse, String> {
    let config_path = crate::config::get_app_config_path()
        .to_string_lossy()
        .to_string();
    let app_ty = AppType::from_str(&app).map_err(|e| e.to_string())?;
    let servers = McpService::get_servers(&state, app_ty).map_err(|e| e.to_string())?;
    Ok(McpConfigResponse {
        config_path,
        servers,
    })
}

/// 在 config.json 中新增或更新一个 MCP 服务器定义
/// [已废弃] 该命令仍然使用旧的分应用API，会转换为统一结构
#[tauri::command]
pub async fn upsert_mcp_server_in_config(
    state: State<'_, AppState>,
    app: String,
    id: String,
    spec: serde_json::Value,
    sync_other_side: Option<bool>,
) -> Result<bool, String> {
    use crate::app_config::McpServer;

    let app_ty = AppType::from_str(&app).map_err(|e| e.to_string())?;

    // 读取现有的服务器（如果存在）
    let existing_server = {
        let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
        servers.get(&id).cloned()
    };

    // 构建新的统一服务器结构
    let mut new_server = if let Some(mut existing) = existing_server {
        // 更新现有服务器
        existing.server = spec.clone();
        existing.apps.set_enabled_for(&app_ty, true);
        existing
    } else {
        // 创建新服务器
        let mut apps = McpApps::default();
        apps.set_enabled_for(&app_ty, true);

        // 尝试从 spec 中提取 name，否则使用 id
        let name = spec
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();

        McpServer {
            id: id.clone(),
            name,
            server: spec,
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: Vec::new(),
        }
    };

    // 如果 sync_other_side 为 true，也启用其他应用
    if sync_other_side.unwrap_or(false) {
        new_server.apps.claude = true;
        new_server.apps.codex = true;
        new_server.apps.gemini = true;
        new_server.apps.opencode = true;
    }

    McpService::upsert_server(&state, new_server)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

/// 在 config.json 中删除一个 MCP 服务器定义
#[tauri::command]
pub async fn delete_mcp_server_in_config(
    state: State<'_, AppState>,
    _app: String, // 参数保留用于向后兼容，但在统一结构中不再需要
    id: String,
) -> Result<bool, String> {
    McpService::delete_server(&state, &id).map_err(|e| e.to_string())
}

/// 设置启用状态并同步到客户端配置
#[tauri::command]
#[allow(deprecated)] // 兼容层命令，内部调用已废弃的 Service 方法
pub async fn set_mcp_enabled(
    state: State<'_, AppState>,
    app: String,
    id: String,
    enabled: bool,
) -> Result<bool, String> {
    let app_ty = AppType::from_str(&app).map_err(|e| e.to_string())?;
    McpService::set_enabled(&state, app_ty, &id, enabled).map_err(|e| e.to_string())
}

// ============================================================================
// v3.7.0 新增：统一 MCP 管理命令
// ============================================================================

/// 获取所有 MCP 服务器（统一结构）
#[tauri::command]
pub async fn get_mcp_servers(
    state: State<'_, AppState>,
) -> Result<IndexMap<String, McpServer>, String> {
    McpService::get_all_servers(&state).map_err(|e| e.to_string())
}

/// 添加或更新 MCP 服务器
#[tauri::command]
pub async fn upsert_mcp_server(
    state: State<'_, AppState>,
    server: McpServer,
) -> Result<(), String> {
    McpService::upsert_server(&state, server).map_err(|e| e.to_string())
}

/// 删除 MCP 服务器
#[tauri::command]
pub async fn delete_mcp_server(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    McpService::delete_server(&state, &id).map_err(|e| e.to_string())
}

/// 切换 MCP 服务器在指定应用的启用状态
#[tauri::command]
pub async fn toggle_mcp_app(
    state: State<'_, AppState>,
    server_id: String,
    app: String,
    enabled: bool,
) -> Result<(), String> {
    let app_ty = AppType::from_str(&app).map_err(|e| e.to_string())?;
    McpService::toggle_app(&state, &server_id, app_ty, enabled).map_err(|e| e.to_string())
}

/// 从所有应用导入 MCP 服务器（复用已有的导入逻辑）
#[tauri::command]
pub async fn import_mcp_from_apps(state: State<'_, AppState>) -> Result<usize, String> {
    let mut total = 0;
    total += McpService::import_from_claude(&state).unwrap_or(0);
    total += McpService::import_from_codex(&state).unwrap_or(0);
    total += McpService::import_from_gemini(&state).unwrap_or(0);
    total += McpService::import_from_opencode(&state).unwrap_or(0);
    total += McpService::import_from_hermes(&state).unwrap_or(0);
    Ok(total)
}

/// MCP 连通性检测结果
#[derive(Debug, Serialize)]
pub struct McpConnectivityResult {
    pub ok: bool,
    pub message: String,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
}

/// 测试 MCP 服务器连通性
#[tauri::command]
pub async fn test_mcp_connectivity(
    server: serde_json::Value,
) -> Result<McpConnectivityResult, String> {
    let server_type = server
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");

    match server_type {
        "stdio" => test_stdio_mcp_connectivity(&server).await,
        "http" | "sse" => test_remote_mcp_connectivity(&server, server_type).await,
        _ => Ok(McpConnectivityResult {
            ok: false,
            message: format!("Unknown server type: {}", server_type),
            server_name: None,
            server_version: None,
        }),
    }
}

const MCP_TEST_TIMEOUT_SECS: u64 = 10;
const MCP_STDIO_PROTOCOL_VERSION: &str = "2025-06-18";

async fn test_stdio_mcp_connectivity(
    server: &serde_json::Value,
) -> Result<McpConnectivityResult, String> {
    let command = server.get("command").and_then(|v| v.as_str()).unwrap_or("");
    if command.is_empty() {
        return Ok(McpConnectivityResult {
            ok: false,
            message: "No command specified".to_string(),
            server_name: None,
            server_version: None,
        });
    }

    let command_path = match crate::claude_mcp::resolve_command_path(command) {
        Some(path) => path,
        None => {
            return Ok(McpConnectivityResult {
                ok: false,
                message: format!("Command not found in app environment: {}", command),
                server_name: None,
                server_version: None,
            });
        }
    };

    let args = parse_string_array(server.get("args"));
    let envs = parse_string_map(server.get("env"));
    let cwd = parse_cwd(server.get("cwd"));

    match run_stdio_initialize_probe(&command_path, &args, &envs, cwd.as_deref()).await {
        Ok(server_info) => {
            let message = format!(
                "MCP server responded to initialize: {} ({})",
                server_info.name, server_info.version
            );
            Ok(McpConnectivityResult {
                ok: true,
                message,
                server_name: Some(server_info.name),
                server_version: Some(server_info.version),
            })
        }
        Err(err) => Ok(McpConnectivityResult {
            ok: false,
            message: format!(
                "Failed to start MCP server: {} [{}{}]",
                err,
                command_path.display(),
                format_command_args(&args)
            ),
            server_name: None,
            server_version: None,
        }),
    }
}

async fn test_remote_mcp_connectivity(
    server: &serde_json::Value,
    server_type: &str,
) -> Result<McpConnectivityResult, String> {
    let url = server.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return Ok(McpConnectivityResult {
            ok: false,
            message: "No URL specified".to_string(),
            server_name: None,
            server_version: None,
        });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(MCP_TEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| e.to_string())?;

    let extra_headers = parse_header_map(server.get("headers"));

    match probe_streamable_http(&client, url, &extra_headers).await {
        Ok(init) => {
            match send_remote_initialized(&client, url, &extra_headers, init.session_id.as_deref())
                .await
            {
                Ok(initialized_status) => {
                    let transport = match init.response_mode {
                        RemoteResponseMode::Json => "JSON",
                        RemoteResponseMode::Sse => "SSE",
                    };
                    let session_hint = init
                        .session_id
                        .as_deref()
                        .map(|id| format!(", session {}", id))
                        .unwrap_or_default();
                    let message = format!(
                        "Remote MCP initialize + initialized succeeded via {}: {} ({}) [HTTP {}{}]",
                        transport,
                        init.server_info.name,
                        init.server_info.version,
                        initialized_status.as_u16(),
                        session_hint
                    );
                    Ok(McpConnectivityResult {
                        ok: true,
                        message,
                        server_name: Some(init.server_info.name),
                        server_version: Some(init.server_info.version),
                    })
                }
                Err(err) => Ok(McpConnectivityResult {
                    ok: false,
                    message: format!(
                        "Remote MCP initialize succeeded but initialized failed: {}",
                        err.message
                    ),
                    server_name: None,
                    server_version: None,
                }),
            }
        }
        Err(http_err)
            if server_type == "sse"
                || matches!(
                    http_err.status,
                    Some(reqwest::StatusCode::BAD_REQUEST)
                        | Some(reqwest::StatusCode::NOT_FOUND)
                        | Some(reqwest::StatusCode::METHOD_NOT_ALLOWED)
                ) =>
        {
            match probe_legacy_sse(&client, url, &extra_headers).await {
                Ok(message) => Ok(McpConnectivityResult {
                    ok: true,
                    message,
                    server_name: None,
                    server_version: None,
                }),
                Err(sse_err) => Ok(McpConnectivityResult {
                    ok: false,
                    message: format!(
                        "Remote MCP probe failed: {}{}",
                        http_err.message,
                        format_fallback_error(sse_err)
                    ),
                    server_name: None,
                    server_version: None,
                }),
            }
        }
        Err(http_err) => Ok(McpConnectivityResult {
            ok: false,
            message: format!("Remote MCP probe failed: {}", http_err.message),
            server_name: None,
            server_version: None,
        }),
    }
}

#[derive(Debug)]
struct McpServerInfo {
    name: String,
    version: String,
}

#[derive(Debug)]
enum RemoteResponseMode {
    Json,
    Sse,
}

#[derive(Debug)]
struct RemoteInitializeSuccess {
    server_info: McpServerInfo,
    session_id: Option<String>,
    response_mode: RemoteResponseMode,
}

#[derive(Debug)]
struct RemoteProbeError {
    status: Option<reqwest::StatusCode>,
    message: String,
}

async fn run_stdio_initialize_probe(
    command_path: &Path,
    args: &[String],
    envs: &HashMap<String, String>,
    cwd: Option<&Path>,
) -> Result<McpServerInfo, String> {
    let command_path = command_path.to_path_buf();
    let args = args.to_vec();
    let envs = envs.clone();
    let cwd = cwd.map(Path::to_path_buf);

    tokio::task::spawn_blocking(move || {
        let mut command = Command::new(&command_path);
        command
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = cwd.as_deref() {
            command.current_dir(cwd);
        }

        for (key, value) in &envs {
            command.env(key, value);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("spawn failed: {}", e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open stdin pipe".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to open stdout pipe".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "failed to open stderr pipe".to_string())?;

        let stderr_output = Arc::new(Mutex::new(String::new()));
        let stderr_handle = spawn_stderr_collector(stderr, Arc::clone(&stderr_output));

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(perform_initialize_handshake(stdin, stdout));
        });

        let handshake_result = rx
            .recv_timeout(Duration::from_secs(MCP_TEST_TIMEOUT_SECS))
            .unwrap_or_else(|_| {
                Err(format!(
                    "initialize timed out after {}s",
                    MCP_TEST_TIMEOUT_SECS
                ))
            });

        let _ = child.kill();
        let _ = child.wait();
        let _ = stderr_handle.join();

        let stderr_output = stderr_output
            .lock()
            .map(|output| output.clone())
            .unwrap_or_else(|_| "failed to capture stderr".to_string());

        handshake_result.map_err(|err| enrich_stdio_error(err, stderr_output))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn perform_initialize_handshake(
    mut stdin: ChildStdin,
    stdout: ChildStdout,
) -> Result<McpServerInfo, String> {
    let request = build_initialize_request();
    let payload = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    stdin
        .write_all(payload.as_bytes())
        .map_err(|e| format!("failed to write initialize request: {}", e))?;
    stdin
        .write_all(b"\n")
        .map_err(|e| format!("failed to write newline: {}", e))?;
    stdin
        .flush()
        .map_err(|e| format!("failed to flush initialize request: {}", e))?;

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let server_info = read_initialize_response(&mut reader, &mut line)?;

    let initialized = build_initialized_notification();
    let payload = serde_json::to_string(&initialized).map_err(|e| e.to_string())?;
    stdin
        .write_all(payload.as_bytes())
        .map_err(|e| format!("failed to write initialized notification: {}", e))?;
    stdin
        .write_all(b"\n")
        .map_err(|e| format!("failed to write initialized newline: {}", e))?;
    stdin
        .flush()
        .map_err(|e| format!("failed to flush initialized notification: {}", e))?;

    Ok(server_info)
}

async fn probe_streamable_http(
    client: &reqwest::Client,
    url: &str,
    extra_headers: &HashMap<String, String>,
) -> Result<RemoteInitializeSuccess, RemoteProbeError> {
    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", MCP_STDIO_PROTOCOL_VERSION)
        .json(&build_initialize_request());

    request = apply_headers_to_request(request, extra_headers);

    let response = request.send().await.map_err(map_remote_request_error)?;
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(RemoteProbeError {
            status: Some(status),
            message: format_http_error(status, &body),
        });
    }

    let session_id = response
        .headers()
        .get("MCP-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if content_type.contains("text/event-stream") {
        let mut stream = response.bytes_stream();
        if let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let body = extract_sse_json_payload(&text).ok_or_else(|| RemoteProbeError {
                        status: Some(status),
                        message: format!(
                            "streamable HTTP initialize returned SSE without JSON payload: {}",
                            text.lines()
                                .find(|line| !line.trim().is_empty())
                                .unwrap_or("<empty>")
                        ),
                    })?;

                    let server_info =
                        parse_initialize_response(&body).map_err(|err| RemoteProbeError {
                            status: Some(status),
                            message: err,
                        })?;

                    return Ok(RemoteInitializeSuccess {
                        server_info,
                        session_id,
                        response_mode: RemoteResponseMode::Sse,
                    });
                }
                Err(err) => {
                    return Err(RemoteProbeError {
                        status: Some(status),
                        message: format!("failed to read initialize stream: {}", err),
                    });
                }
            }
        }

        return Err(RemoteProbeError {
            status: Some(status),
            message: "streamable HTTP initialize returned empty SSE stream".to_string(),
        });
    }

    let body = response.text().await.map_err(|e| RemoteProbeError {
        status: Some(status),
        message: format!("failed to read initialize response: {}", e),
    })?;

    let server_info = parse_initialize_response(&body).map_err(|err| RemoteProbeError {
        status: Some(status),
        message: err,
    })?;

    Ok(RemoteInitializeSuccess {
        server_info,
        session_id,
        response_mode: RemoteResponseMode::Json,
    })
}

async fn send_remote_initialized(
    client: &reqwest::Client,
    url: &str,
    extra_headers: &HashMap<String, String>,
    session_id: Option<&str>,
) -> Result<reqwest::StatusCode, RemoteProbeError> {
    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", MCP_STDIO_PROTOCOL_VERSION)
        .json(&build_initialized_notification());

    if let Some(session_id) = session_id {
        request = request.header("MCP-Session-Id", session_id);
    }

    request = apply_headers_to_request(request, extra_headers);

    let response = request.send().await.map_err(map_remote_request_error)?;
    let status = response.status();

    if status.is_success() || status == reqwest::StatusCode::ACCEPTED {
        Ok(status)
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(RemoteProbeError {
            status: Some(status),
            message: format_http_error(status, &body),
        })
    }
}

async fn probe_legacy_sse(
    client: &reqwest::Client,
    url: &str,
    extra_headers: &HashMap<String, String>,
) -> Result<String, RemoteProbeError> {
    let mut request = client
        .get(url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("MCP-Protocol-Version", MCP_STDIO_PROTOCOL_VERSION);

    request = apply_headers_to_request(request, extra_headers);

    let response = request.send().await.map_err(map_remote_request_error)?;
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(RemoteProbeError {
            status: Some(status),
            message: format_http_error(status, &body),
        });
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !content_type.contains("text/event-stream") {
        let body = response.text().await.unwrap_or_default();
        return Err(RemoteProbeError {
            status: Some(status),
            message: format!("expected text/event-stream, got {}: {}", content_type, body),
        });
    }

    let mut stream = response.bytes_stream();
    if let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let endpoint = extract_sse_endpoint(&text);
                let first_event = summarize_sse_first_event(&text);

                Ok(match endpoint {
                    Some(endpoint) => format!(
                        "Legacy SSE endpoint reachable: discovered endpoint {}",
                        endpoint
                    ),
                    None => format!("Legacy SSE stream reachable: {}", first_event),
                })
            }
            Err(err) => Err(RemoteProbeError {
                status: Some(status),
                message: format!("failed to read SSE stream: {}", err),
            }),
        }
    } else {
        Err(RemoteProbeError {
            status: Some(status),
            message: "SSE stream opened but returned no data".to_string(),
        })
    }
}

fn build_initialize_request() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_STDIO_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "cc-switch",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn build_initialized_notification() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })
}

fn read_initialize_response<R: BufRead>(
    reader: &mut R,
    line: &mut String,
) -> Result<McpServerInfo, String> {
    loop {
        line.clear();
        let bytes = reader
            .read_line(line)
            .map_err(|e| format!("failed to read initialize response: {}", e))?;

        if bytes == 0 {
            return Err("process exited before returning initialize response".to_string());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 跳过非 JSON 行（部分 server 会向 stdout 打印启动日志）
        let response: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if response.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
            return Err("initialize response missing jsonrpc=2.0".to_string());
        }

        if response.get("id") != Some(&serde_json::json!(1)) {
            continue;
        }

        if let Some(err) = response.get("error") {
            return Err(format!("server returned initialize error: {}", err));
        }

        let result = response
            .get("result")
            .ok_or_else(|| "initialize response missing result".to_string())?;
        let server_info = result
            .get("serverInfo")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "initialize response missing serverInfo".to_string())?;

        let name = server_info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let version = server_info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        return Ok(McpServerInfo { name, version });
    }
}

fn parse_initialize_response(body: &str) -> Result<McpServerInfo, String> {
    let response: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("response is not valid JSON: {}", e))?;

    if response.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Err("initialize response missing jsonrpc=2.0".to_string());
    }

    if response.get("id") != Some(&serde_json::json!(1)) {
        return Err("initialize response missing id=1".to_string());
    }

    if let Some(err) = response.get("error") {
        return Err(format!("server returned initialize error: {}", err));
    }

    let result = response
        .get("result")
        .ok_or_else(|| "initialize response missing result".to_string())?;
    let server_info = result
        .get("serverInfo")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "initialize response missing serverInfo".to_string())?;

    let name = server_info
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let version = server_info
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(McpServerInfo { name, version })
}

fn apply_headers_to_request(
    mut request: reqwest::RequestBuilder,
    headers: &HashMap<String, String>,
) -> reqwest::RequestBuilder {
    for (key, value) in headers {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            request = request.header(name, value);
        }
    }

    request
}

fn map_remote_request_error(err: reqwest::Error) -> RemoteProbeError {
    let message = if err.is_timeout() {
        format!("connection timed out ({}s)", MCP_TEST_TIMEOUT_SECS)
    } else if err.is_connect() {
        format!("connection refused: {}", err)
    } else {
        format!("request failed: {}", err)
    };

    RemoteProbeError {
        status: err.status(),
        message,
    }
}

fn format_http_error(status: reqwest::StatusCode, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        format!("server returned HTTP {}", status.as_u16())
    } else {
        format!("server returned HTTP {}: {}", status.as_u16(), body)
    }
}

fn format_fallback_error(err: RemoteProbeError) -> String {
    format!(" | fallback SSE probe failed: {}", err.message)
}

fn extract_sse_json_payload(text: &str) -> Option<String> {
    let mut data_lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("data:") {
            let data = rest.trim();
            if !data.is_empty() {
                data_lines.push(data.to_string());
            }
        }
    }

    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

fn summarize_sse_first_event(text: &str) -> String {
    let mut event_name: Option<String> = None;
    let mut first_data: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("data:") {
            let data = rest.trim();
            if !data.is_empty() {
                first_data = Some(data.to_string());
                break;
            }
        }
    }

    match (event_name, first_data) {
        (Some(event), Some(data)) => format!("event={} data={}", event, data),
        (Some(event), None) => format!("event={}", event),
        (None, Some(data)) => format!("data={}", data),
        (None, None) => text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("<empty>")
            .to_string(),
    }
}

fn extract_sse_endpoint(text: &str) -> Option<String> {
    let mut current_event: Option<&str> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("event:") {
            current_event = Some(rest.trim());
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("data:") {
            if current_event == Some("endpoint") {
                let data = rest.trim();
                if !data.is_empty() {
                    return Some(data.to_string());
                }
            }
        }
    }

    None
}

fn spawn_stderr_collector(
    stderr: ChildStderr,
    stderr_output: Arc<Mutex<String>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buffer = String::new();
        let mut lines = Vec::new();

        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let line = buffer.trim();
                    if !line.is_empty() {
                        lines.push(line.to_string());
                    }
                    if lines.len() >= 5 {
                        break;
                    }
                }
                Err(err) => {
                    lines.push(format!("failed to read stderr: {}", err));
                    break;
                }
            }
        }

        if let Ok(mut output) = stderr_output.lock() {
            *output = lines.join(" | ");
        }
    })
}

fn parse_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_string_map(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    value
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_header_map(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    parse_string_map(value)
}

fn parse_cwd(value: Option<&serde_json::Value>) -> Option<PathBuf> {
    value
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|cwd| !cwd.is_empty())
        .map(PathBuf::from)
}

fn format_command_args(args: &[String]) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    }
}

fn enrich_stdio_error(error: String, stderr_output: String) -> String {
    if stderr_output.trim().is_empty() {
        error
    } else {
        format!("{} | stderr: {}", error, stderr_output)
    }
}

/// 解析 JSON 文件中的 MCP 服务器配置（自动检测格式）
#[derive(Debug, Serialize)]
pub struct ParsedMcpEntry {
    pub name: String,
    pub server: serde_json::Value,
}

#[tauri::command]
pub async fn parse_mcp_json_file(path: String) -> Result<Vec<ParsedMcpEntry>, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;

    let obj = json.as_object().ok_or("JSON root must be an object")?;

    // 格式自动检测
    // 1. OpenCode 格式: { mcp: { servers: { name: { type: "local"|"remote", ... } } } }
    if let Some(mcp) = obj.get("mcp").and_then(|v| v.as_object()) {
        if let Some(servers) = mcp.get("servers").and_then(|v| v.as_object()) {
            return Ok(convert_opencode_format(servers));
        }
    }

    // 2. CC-Switch 内部格式: entries 含 server + apps 字段
    if obj
        .values()
        .any(|v| v.get("server").is_some() && v.get("apps").is_some())
    {
        return Ok(obj
            .iter()
            .filter_map(|(name, entry)| {
                entry.get("server").map(|server| ParsedMcpEntry {
                    name: name.clone(),
                    server: server.clone(),
                })
            })
            .collect());
    }

    // 3. Codex 格式: { mcp_servers: { name: { ... } } }
    if let Some(servers) = obj.get("mcp_servers").and_then(|v| v.as_object()) {
        return Ok(convert_codex_format(servers));
    }

    // 4. Claude/Gemini/标准 MCP 格式: { mcpServers: { name: { ... } } }
    if let Some(servers) = obj.get("mcpServers").and_then(|v| v.as_object()) {
        return Ok(convert_standard_format(servers));
    }

    // 5. 裸 map: 顶层对象的值含 command 或 url 字段
    if obj
        .values()
        .any(|v| v.get("command").is_some() || v.get("url").is_some())
    {
        return Ok(convert_standard_format(obj));
    }

    Err("Unrecognized MCP configuration format".to_string())
}

/// 转换标准 MCP 格式 (Claude/Gemini/MCP Router)
fn convert_standard_format(
    servers: &serde_json::Map<String, serde_json::Value>,
) -> Vec<ParsedMcpEntry> {
    servers
        .iter()
        .map(|(name, spec)| {
            let mut server = spec.clone();
            // 确保有 type 字段
            if server.get("type").is_none() {
                let obj = server.as_object_mut().unwrap();
                if obj.contains_key("command") {
                    obj.insert("type".to_string(), serde_json::json!("stdio"));
                } else if obj.contains_key("url") {
                    obj.insert("type".to_string(), serde_json::json!("http"));
                }
            }
            ParsedMcpEntry {
                name: name.clone(),
                server,
            }
        })
        .collect()
}

/// 转换 OpenCode 格式 (local → stdio, remote → http/sse)
fn convert_opencode_format(
    servers: &serde_json::Map<String, serde_json::Value>,
) -> Vec<ParsedMcpEntry> {
    servers
        .iter()
        .map(|(name, spec)| {
            let oc_type = spec.get("type").and_then(|v| v.as_str()).unwrap_or("local");
            let mut server = serde_json::Map::new();

            match oc_type {
                "local" => {
                    server.insert("type".to_string(), serde_json::json!("stdio"));
                    // OpenCode 的 command 是 string[] (合并了 cmd + args)
                    if let Some(cmd_arr) = spec.get("command").and_then(|v| v.as_array()) {
                        if let Some(first) = cmd_arr.first().and_then(|v| v.as_str()) {
                            server.insert("command".to_string(), serde_json::json!(first));
                        }
                        if cmd_arr.len() > 1 {
                            let args: Vec<&serde_json::Value> = cmd_arr[1..].iter().collect();
                            server.insert("args".to_string(), serde_json::json!(args));
                        }
                    }
                    if let Some(env) = spec.get("environment") {
                        server.insert("env".to_string(), env.clone());
                    }
                }
                "remote" => {
                    server.insert("type".to_string(), serde_json::json!("http"));
                    if let Some(url) = spec.get("url") {
                        server.insert("url".to_string(), url.clone());
                    }
                    if let Some(headers) = spec.get("headers") {
                        server.insert("headers".to_string(), headers.clone());
                    }
                }
                other => {
                    server.insert("type".to_string(), serde_json::json!(other));
                }
            }

            ParsedMcpEntry {
                name: name.clone(),
                server: serde_json::Value::Object(server),
            }
        })
        .collect()
}

/// 转换 Codex 格式 (http_headers → headers)
fn convert_codex_format(
    servers: &serde_json::Map<String, serde_json::Value>,
) -> Vec<ParsedMcpEntry> {
    servers
        .iter()
        .map(|(name, spec)| {
            let mut server = spec.clone();
            // Codex 使用 http_headers 而非 headers
            if let Some(obj) = server.as_object_mut() {
                if let Some(http_headers) = obj.remove("http_headers") {
                    obj.insert("headers".to_string(), http_headers);
                }
                // 确保有 type 字段
                if !obj.contains_key("type") {
                    if obj.contains_key("command") {
                        obj.insert("type".to_string(), serde_json::json!("stdio"));
                    } else if obj.contains_key("url") {
                        obj.insert("type".to_string(), serde_json::json!("sse"));
                    }
                }
            }
            ParsedMcpEntry {
                name: name.clone(),
                server,
            }
        })
        .collect()
}
