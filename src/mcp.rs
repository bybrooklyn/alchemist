use crate::Agent;
use crate::db::Db;
use crate::error::Result;
use crate::system::scanner::LibraryScanner;
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

const JSONRPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
    id: Value,
}

#[derive(Debug, Serialize)]
struct McpError {
    code: i32,
    message: String,
}

pub struct McpServer {
    db: Arc<Db>,
    agent: Arc<Agent>,
    library_scanner: Option<Arc<LibraryScanner>>,
}

impl McpServer {
    pub fn new(
        db: Arc<Db>,
        agent: Arc<Agent>,
        library_scanner: Option<Arc<LibraryScanner>>,
    ) -> Self {
        Self {
            db,
            agent,
            library_scanner,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting MCP server over stdio");
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = reader
            .next_line()
            .await
            .map_err(crate::error::AlchemistError::Io)?
        {
            let Some(response) = self.handle_json_line(&line).await else {
                continue;
            };
            let response_json = match serde_json::to_string(&response) {
                Ok(json) => json,
                Err(err) => {
                    error!("Failed to serialize MCP response: {err}");
                    continue;
                }
            };
            stdout
                .write_all(response_json.as_bytes())
                .await
                .map_err(crate::error::AlchemistError::Io)?;
            stdout
                .write_all(b"\n")
                .await
                .map_err(crate::error::AlchemistError::Io)?;
            stdout
                .flush()
                .await
                .map_err(crate::error::AlchemistError::Io)?;
        }

        Ok(())
    }

    async fn handle_json_line(&self, line: &str) -> Option<McpResponse> {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to parse MCP request: {err}");
                return Some(error_response(PARSE_ERROR, "Parse error", Value::Null));
            }
        };
        self.handle_value(value).await
    }

    async fn handle_value(&self, value: Value) -> Option<McpResponse> {
        let id = value.get("id").cloned();
        let response_id = id.clone().unwrap_or(Value::Null);

        if value
            .get("jsonrpc")
            .and_then(Value::as_str)
            .is_some_and(|version| version != JSONRPC_VERSION)
        {
            return Some(error_response(
                INVALID_REQUEST,
                "Invalid JSON-RPC version",
                response_id,
            ));
        }

        let Some(method) = value.get("method").and_then(Value::as_str) else {
            return Some(error_response(
                INVALID_REQUEST,
                "Request is missing method",
                response_id,
            ));
        };

        if id.is_none() {
            return match method {
                "notifications/initialized" => None,
                _ if method.starts_with("notifications/") => None,
                _ => Some(error_response(
                    INVALID_REQUEST,
                    "Requests must include an id",
                    Value::Null,
                )),
            };
        }

        let params = value.get("params").cloned().unwrap_or(Value::Null);
        let response = match method {
            "initialize" => ok_response(initialize_result(), response_id),
            "ping" => ok_response(json!({}), response_id),
            "tools/list" => ok_response(json!({ "tools": tool_definitions() }), response_id),
            "tools/call" => self.handle_tool_call(params, response_id).await,
            _ => error_response(METHOD_NOT_FOUND, "Method not found", response_id),
        };

        Some(response)
    }

    async fn handle_tool_call(&self, params: Value, id: Value) -> McpResponse {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_response(INVALID_PARAMS, "tools/call requires params.name", id);
        };
        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
        let result = match name {
            "alchemist_engine_status" => self.tool_engine_status().await,
            "alchemist_job_summary" => self.tool_job_summary().await,
            "alchemist_recent_jobs" => self.tool_recent_jobs(arguments).await,
            "alchemist_savings_summary" => self.tool_savings_summary().await,
            "alchemist_scan_status" => self.tool_scan_status().await,
            "alchemist_system_health" => self.tool_system_health().await,
            _ => {
                return error_response(INVALID_PARAMS, format!("Unknown tool: {name}"), id);
            }
        };

        match result {
            Ok(value) => ok_response(tool_result(value, false), id),
            Err(message) => ok_response(tool_error_result(message), id),
        }
    }

    async fn tool_engine_status(&self) -> std::result::Result<Value, String> {
        let mode = self.agent.current_mode().await;
        Ok(json!({
            "engine_mode": mode.as_str(),
            "is_paused": self.agent.is_paused(),
            "is_manual_paused": self.agent.is_manual_paused(),
            "is_scheduler_paused": self.agent.is_scheduler_paused(),
            "is_draining": self.agent.is_draining(),
            "concurrent_jobs_limit": self.agent.concurrent_jobs_limit()
        }))
    }

    async fn tool_job_summary(&self) -> std::result::Result<Value, String> {
        let stats = self
            .db
            .get_job_stats()
            .await
            .map_err(|err| err.to_string())?;
        Ok(json!(stats))
    }

    async fn tool_recent_jobs(&self, arguments: Value) -> std::result::Result<Value, String> {
        let limit = parse_limit(arguments, 10, 50)?;
        let jobs = self
            .db
            .get_all_jobs()
            .await
            .map_err(|err| err.to_string())?;
        let jobs: Vec<Value> = jobs
            .into_iter()
            .take(limit)
            .map(|job| {
                json!({
                    "id": job.id,
                    "input_path": job.input_path,
                    "output_path": job.output_path,
                    "status": job.status,
                    "priority": job.priority,
                    "progress": job.progress,
                    "attempt_count": job.attempt_count,
                    "decision_reason": job.decision_reason,
                    "updated_at": job.updated_at,
                })
            })
            .collect();
        Ok(json!({
            "limit": limit,
            "jobs": jobs
        }))
    }

    async fn tool_savings_summary(&self) -> std::result::Result<Value, String> {
        let summary = self
            .db
            .get_savings_summary()
            .await
            .map_err(|err| err.to_string())?;
        Ok(json!(summary))
    }

    async fn tool_scan_status(&self) -> std::result::Result<Value, String> {
        let Some(scanner) = &self.library_scanner else {
            return Ok(json!({
                "available": false,
                "is_running": false,
                "files_found": 0,
                "files_added": 0,
                "current_folder": null
            }));
        };
        let status = scanner.get_status().await;
        Ok(json!({
            "available": true,
            "is_running": status.is_running,
            "files_found": status.files_found,
            "files_added": status.files_added,
            "current_folder": status.current_folder
        }))
    }

    async fn tool_system_health(&self) -> std::result::Result<Value, String> {
        let db_ready = self.db.get_stats().await.is_ok();
        Ok(json!({
            "version": crate::version::current(),
            "database_ready": db_ready,
            "mcp_mode": "read_only",
            "protocol_version": MCP_PROTOCOL_VERSION,
            "tools": tool_definitions()
                .into_iter()
                .filter_map(|tool| tool.get("name").cloned())
                .collect::<Vec<_>>()
        }))
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "alchemist",
            "title": "Alchemist MCP Server",
            "version": crate::version::current()
        },
        "instructions": "Read-only Alchemist server. Tools may inspect status, jobs, scan state, savings, and health, but do not mutate queue, engine, or configuration state."
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool_definition(
            "alchemist_engine_status",
            "Engine Status",
            "Read Alchemist engine mode, pause/drain state, and concurrency limit.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool_definition(
            "alchemist_job_summary",
            "Job Summary",
            "Read aggregate counts for active, queued, completed, and failed jobs.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool_definition(
            "alchemist_recent_jobs",
            "Recent Jobs",
            "Read recently updated jobs without changing the queue.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 50,
                        "default": 10
                    }
                },
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "alchemist_savings_summary",
            "Savings Summary",
            "Read storage savings and codec savings metrics.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool_definition(
            "alchemist_scan_status",
            "Scan Status",
            "Read current library scan progress.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool_definition(
            "alchemist_system_health",
            "System Health",
            "Read Alchemist version, MCP mode, and database readiness.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
    ]
}

fn tool_definition(name: &str, title: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema,
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false
        }
    })
}

fn parse_limit(arguments: Value, default: usize, max: usize) -> std::result::Result<usize, String> {
    if arguments.is_null() {
        return Ok(default);
    }
    let limit = match arguments.get("limit") {
        Some(value) => value
            .as_u64()
            .ok_or_else(|| "limit must be a positive integer".to_string())?,
        None => return Ok(default),
    };
    if limit == 0 || limit > max as u64 {
        return Err(format!("limit must be between 1 and {max}"));
    }
    usize::try_from(limit).map_err(|err| err.to_string())
}

fn tool_result(structured: Value, is_error: bool) -> Value {
    let text = serde_json::to_string_pretty(&structured).unwrap_or_else(|err| {
        format!(
            "{{\"error\":\"failed to serialize structured content\",\"message\":\"{}\"}}",
            err
        )
    });
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured,
        "isError": is_error
    })
}

fn tool_error_result(message: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    })
}

fn ok_response(result: Value, id: Value) -> McpResponse {
    McpResponse {
        jsonrpc: JSONRPC_VERSION,
        result: Some(result),
        error: None,
        id,
    }
}

fn error_response(code: i32, message: impl Into<String>, id: Value) -> McpResponse {
    McpResponse {
        jsonrpc: JSONRPC_VERSION,
        result: None,
        error: Some(McpError {
            code,
            message: message.into(),
        }),
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transcoder;
    use crate::config::Config;
    use crate::db::{EventChannels, Job, JobState};
    use crate::system::hardware::HardwareState;
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use tokio::sync::RwLock;
    use tokio::sync::{Mutex, OwnedMutexGuard};

    static TEST_DB_LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();

    struct TestFixture {
        server: McpServer,
        path: PathBuf,
        _guard: OwnedMutexGuard<()>,
    }

    impl TestFixture {
        fn cleanup(self) {
            let TestFixture {
                server,
                path,
                _guard,
            } = self;
            drop(server);
            cleanup_db(&path);
            drop(_guard);
        }
    }

    async fn test_server() -> Result<TestFixture> {
        let guard = TEST_DB_LOCK
            .get_or_init(|| Arc::new(Mutex::new(())))
            .clone()
            .lock_owned()
            .await;
        let db_path = temp_db_path();
        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let config = Arc::new(RwLock::new(Config::default()));
        let agent = Arc::new(
            Agent::new(
                db.clone(),
                Arc::new(Transcoder::new()),
                config.clone(),
                HardwareState::new(None),
                Arc::new(EventChannels::default()),
                false,
            )
            .await,
        );
        let scanner = Arc::new(LibraryScanner::new(db.clone(), config));
        Ok(TestFixture {
            server: McpServer::new(db, agent, Some(scanner)),
            path: db_path,
            _guard: guard,
        })
    }

    fn temp_db_path() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "alchemist_mcp_test_{}_{}.db",
            std::process::id(),
            nanos
        ))
    }

    fn cleanup_db(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }

    fn request(method: &str, id: i64, params: Value) -> Value {
        json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": id,
            "method": method,
            "params": params
        })
    }

    async fn handle(server: &McpServer, value: Value) -> McpResponse {
        match server.handle_value(value).await {
            Some(response) => response,
            None => panic!("test expected a response"),
        }
    }

    #[tokio::test]
    async fn malformed_json_returns_parse_error() -> Result<()> {
        let fixture = test_server().await?;
        let response = match fixture.server.handle_json_line("{not json").await {
            Some(response) => response,
            None => panic!("missing response"),
        };
        assert_eq!(response.id, Value::Null);
        assert_eq!(
            response.error.as_ref().map(|error| error.code),
            Some(PARSE_ERROR)
        );
        fixture.cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn initialize_and_ping_return_protocol_results() -> Result<()> {
        let fixture = test_server().await?;
        let init = handle(&fixture.server, request("initialize", 1, json!({}))).await;
        assert_eq!(
            init.result
                .as_ref()
                .and_then(|value| value.get("protocolVersion")),
            Some(&json!(MCP_PROTOCOL_VERSION))
        );
        let ping = handle(&fixture.server, request("ping", 2, json!({}))).await;
        assert_eq!(ping.result, Some(json!({})));
        fixture.cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn initialized_notification_has_no_response() -> Result<()> {
        let fixture = test_server().await?;
        let response = fixture
            .server
            .handle_value(json!({
                "jsonrpc": JSONRPC_VERSION,
                "method": "notifications/initialized"
            }))
            .await;
        assert!(response.is_none());
        fixture.cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn tools_list_is_read_only_and_excludes_mutations() -> Result<()> {
        let fixture = test_server().await?;
        let response = handle(&fixture.server, request("tools/list", 3, json!({}))).await;
        let tools = response
            .result
            .as_ref()
            .and_then(|result| result.get("tools"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let names: Vec<String> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect();
        assert!(names.contains(&"alchemist_engine_status".to_string()));
        assert!(!names.iter().any(|name| name.contains("trigger")));
        assert!(!names.iter().any(|name| name.contains("enqueue")));
        fixture.cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn unknown_tool_returns_invalid_params_error() -> Result<()> {
        let fixture = test_server().await?;
        let response = handle(
            &fixture.server,
            request("tools/call", 4, json!({ "name": "alchemist_enqueue" })),
        )
        .await;
        assert_eq!(
            response.error.as_ref().map(|error| error.code),
            Some(INVALID_PARAMS)
        );
        fixture.cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn read_only_tools_return_structured_content() -> Result<()> {
        let fixture = test_server().await?;
        let response = handle(
            &fixture.server,
            request(
                "tools/call",
                5,
                json!({
                    "name": "alchemist_engine_status",
                    "arguments": {}
                }),
            ),
        )
        .await;
        assert_eq!(
            response
                .result
                .as_ref()
                .and_then(|result| result.get("isError")),
            Some(&json!(false))
        );
        assert!(
            response
                .result
                .as_ref()
                .and_then(|result| result.get("structuredContent"))
                .and_then(|content| content.get("engine_mode"))
                .is_some()
        );
        fixture.cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn recent_jobs_respects_limit() -> Result<()> {
        let fixture = test_server().await?;
        fixture
            .server
            .db
            .add_job(Job {
                id: 1,
                input_path: "/tmp/input.mkv".to_string(),
                output_path: "/tmp/output.mkv".to_string(),
                status: JobState::Queued,
                decision_reason: None,
                priority: 0,
                progress: 0.0,
                attempt_count: 0,
                vmaf_score: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                input_metadata_json: None,
            })
            .await?;
        let response = handle(
            &fixture.server,
            request(
                "tools/call",
                6,
                json!({
                    "name": "alchemist_recent_jobs",
                    "arguments": { "limit": 1 }
                }),
            ),
        )
        .await;
        let jobs_len = response
            .result
            .as_ref()
            .and_then(|result| result.get("structuredContent"))
            .and_then(|content| content.get("jobs"))
            .and_then(Value::as_array)
            .map(Vec::len);
        assert_eq!(jobs_len, Some(1));
        fixture.cleanup();
        Ok(())
    }
}
