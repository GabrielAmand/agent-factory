use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::error::{AppError, ErrorKind};
use crate::protocol::{DeveloperWorkspace, LeadResponse};

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_DEVELOPER_HTTP_REQUEST_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT_SECONDS: u64 = 2;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OllamaMetrics {
    pub prompt_eval_count: Option<u64>,
    pub eval_count: Option<u64>,
    pub total_duration: Option<u64>,
    pub load_duration: Option<u64>,
    pub prompt_eval_duration: Option<u64>,
    pub eval_duration: Option<u64>,
}

pub struct RoleSuccess<T> {
    pub output: T,
    pub metrics: OllamaMetrics,
}

pub struct OllamaFailure {
    pub error: Box<AppError>,
    pub metrics: Option<OllamaMetrics>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 2],
    stream: bool,
    format: &'a Value,
    keep_alive: u8,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_duration: Option<u64>,
    eval_duration: Option<u64>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

pub fn call_lead(
    config: &Config,
    user_request: &str,
    prompt: &str,
    schema: &Value,
) -> Result<RoleSuccess<LeadResponse>, OllamaFailure> {
    let envelope = send_chat(
        config,
        &config.lead_model,
        user_request,
        prompt,
        schema,
        None,
    )?;
    let metrics = metrics_from(&envelope);
    let output = LeadResponse::parse_and_validate(&envelope.message.content).map_err(|error| {
        OllamaFailure {
            error: Box::new(error),
            metrics: Some(metrics_from(&envelope)),
        }
    })?;
    Ok(RoleSuccess { output, metrics })
}

pub fn call_developer(
    config: &Config,
    request_json: &str,
    selected_task_id: &str,
    prompt: &str,
    schema: &Value,
) -> Result<RoleSuccess<DeveloperWorkspace>, OllamaFailure> {
    let envelope = send_chat(
        config,
        &config.developer_model,
        request_json,
        prompt,
        schema,
        Some(MAX_DEVELOPER_HTTP_REQUEST_BYTES),
    )?;
    let metrics = metrics_from(&envelope);
    let output =
        DeveloperWorkspace::parse_and_validate(&envelope.message.content, selected_task_id)
            .map_err(|error| OllamaFailure {
                error: Box::new(error),
                metrics: Some(metrics_from(&envelope)),
            })?;
    Ok(RoleSuccess { output, metrics })
}

fn send_chat(
    config: &Config,
    model: &str,
    user_content: &str,
    prompt: &str,
    schema: &Value,
    maximum_request_bytes: Option<usize>,
) -> Result<ChatResponse, OllamaFailure> {
    let request = build_request(model, user_content, prompt, schema);
    let request_bytes = serialize_request(&request, maximum_request_bytes)?;

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECONDS)))
        .timeout_global(Some(Duration::from_secs(config.response_timeout_seconds)))
        .max_redirects(0)
        .proxy(None)
        .build()
        .into();
    let mut response = agent
        .post(config.chat_url.as_str())
        .header("content-type", "application/json")
        .send(&request_bytes)
        .map_err(|error| {
            failure(
                ErrorKind::Network,
                format!("Ollama request failed: {error}"),
            )
        })?;

    if response.status().is_redirection() {
        return Err(failure(
            ErrorKind::Response,
            "Ollama redirects are not allowed",
        ));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_vec()
        .map_err(|error| {
            failure(
                ErrorKind::Response,
                format!("Ollama response exceeded limits or could not be read: {error}"),
            )
        })?;
    serde_json::from_slice(&body).map_err(|error| {
        failure(
            ErrorKind::Response,
            format!("Ollama returned an invalid response envelope: {error}"),
        )
    })
}

fn serialize_request(
    request: &ChatRequest<'_>,
    maximum_bytes: Option<usize>,
) -> Result<Vec<u8>, OllamaFailure> {
    let bytes = serde_json::to_vec(request).map_err(|error| {
        failure(
            ErrorKind::Validation,
            format!("could not serialize Ollama request: {error}"),
        )
    })?;
    if maximum_bytes.is_some_and(|maximum| bytes.len() > maximum) {
        return Err(failure(
            ErrorKind::Delegation,
            "Developer HTTP request body must not exceed 65536 bytes",
        ));
    }
    Ok(bytes)
}

fn failure(kind: ErrorKind, message: impl Into<String>) -> OllamaFailure {
    OllamaFailure {
        error: Box::new(AppError::new(kind, message)),
        metrics: None,
    }
}

fn build_request<'a>(
    model: &'a str,
    user_content: &'a str,
    prompt: &'a str,
    schema: &'a Value,
) -> ChatRequest<'a> {
    ChatRequest {
        model,
        messages: [
            ChatMessage {
                role: "system",
                content: prompt,
            },
            ChatMessage {
                role: "user",
                content: user_content,
            },
        ],
        stream: false,
        format: schema,
        keep_alive: 0,
    }
}

fn metrics_from(response: &ChatResponse) -> OllamaMetrics {
    OllamaMetrics {
        prompt_eval_count: response.prompt_eval_count,
        eval_count: response.eval_count,
        total_duration: response.total_duration,
        load_duration: response.load_duration,
        prompt_eval_duration: response.prompt_eval_duration,
        eval_duration: response.eval_duration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_role_requests_are_non_streaming_unloaded_and_tool_free() {
        let schema = serde_json::json!({"type": "object"});
        for model in ["lead", "developer"] {
            let value =
                serde_json::to_value(build_request(model, "request", "prompt", &schema)).unwrap();
            assert_eq!(value["stream"], false);
            assert_eq!(value["keep_alive"], 0);
            assert_eq!(value["messages"].as_array().unwrap().len(), 2);
            assert!(value.get("tools").is_none());
        }
    }

    #[test]
    fn developer_http_payload_limit_is_larger_than_request_contract_limit() {
        assert!(MAX_DEVELOPER_HTTP_REQUEST_BYTES > crate::protocol::MAX_DEVELOPER_REQUEST_BYTES);
        let schema = serde_json::json!({"type": "object"});
        let oversized = "x".repeat(MAX_DEVELOPER_HTTP_REQUEST_BYTES);
        let request = build_request("developer", &oversized, "prompt", &schema);
        assert!(serialize_request(&request, Some(MAX_DEVELOPER_HTTP_REQUEST_BYTES)).is_err());
    }
}
