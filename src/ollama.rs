use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::error::{AppError, ErrorKind};
use crate::explorer::{ExplorerConfig, MAX_EXPLORER_RESPONSE_BYTES};
use crate::protocol::{DeveloperWorkspace, LeadResponse};

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_DEVELOPER_HTTP_REQUEST_BYTES: usize = 64 * 1024;
const MAX_EXPLORER_HTTP_REQUEST_BYTES: usize = 192 * 1024;
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
    pub stable_code: Option<&'static str>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 2],
    stream: bool,
    format: &'a Value,
    keep_alive: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
}

#[derive(Serialize)]
struct ChatOptions {
    temperature: f32,
    num_ctx: u32,
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
            stable_code: None,
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
                stable_code: None,
            })?;
    Ok(RoleSuccess { output, metrics })
}

pub fn call_explorer(
    config: &Config,
    explorer: &ExplorerConfig,
    request_json: &str,
    prompt: &str,
    schema: &Value,
) -> Result<RoleSuccess<String>, OllamaFailure> {
    let request = build_explorer_request(
        &explorer.model,
        request_json,
        prompt,
        schema,
        explorer.context_tokens,
    );
    let request_bytes = serialize_request(&request, Some(MAX_EXPLORER_HTTP_REQUEST_BYTES))?;
    let envelope = send_serialized_chat(
        config,
        &request_bytes,
        explorer.timeout_seconds,
        MAX_EXPLORER_RESPONSE_BYTES as u64,
        Some("explorer_timeout"),
    )?;
    if envelope.message.content.is_empty() {
        return Err(failure(
            ErrorKind::Response,
            "Explorer returned empty output",
        ));
    }
    let metrics = metrics_from(&envelope);
    Ok(RoleSuccess {
        output: envelope.message.content,
        metrics,
    })
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

    send_serialized_chat(
        config,
        &request_bytes,
        config.response_timeout_seconds,
        MAX_RESPONSE_BYTES,
        None,
    )
}

fn send_serialized_chat(
    config: &Config,
    request_bytes: &[u8],
    timeout_seconds: u64,
    maximum_response_bytes: u64,
    timeout_code: Option<&'static str>,
) -> Result<ChatResponse, OllamaFailure> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECONDS)))
        .timeout_global(Some(Duration::from_secs(timeout_seconds)))
        .max_redirects(0)
        .proxy(None)
        .build()
        .into();
    let mut response = agent
        .post(config.chat_url.as_str())
        .header("content-type", "application/json")
        .send(request_bytes)
        .map_err(|error| {
            if matches!(error, ureq::Error::Timeout(_)) {
                timeout_failure(timeout_code)
            } else {
                failure(
                    ErrorKind::Network,
                    format!("Ollama request failed: {error}"),
                )
            }
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
        .limit(maximum_response_bytes)
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

fn timeout_failure(code: Option<&'static str>) -> OllamaFailure {
    match code {
        Some(code) => coded_failure(ErrorKind::Network, code, "Ollama request timed out"),
        None => failure(ErrorKind::Network, "Ollama request timed out"),
    }
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
        let message = if maximum_bytes == Some(MAX_DEVELOPER_HTTP_REQUEST_BYTES) {
            "Developer HTTP request body must not exceed 65536 bytes"
        } else {
            "Explorer HTTP request body must not exceed 196608 bytes"
        };
        return Err(failure(ErrorKind::Delegation, message));
    }
    Ok(bytes)
}

fn failure(kind: ErrorKind, message: impl Into<String>) -> OllamaFailure {
    OllamaFailure {
        error: Box::new(AppError::new(kind, message)),
        metrics: None,
        stable_code: None,
    }
}

fn coded_failure(kind: ErrorKind, code: &'static str, message: impl Into<String>) -> OllamaFailure {
    OllamaFailure {
        error: Box::new(AppError::coded(kind, code, message)),
        metrics: None,
        stable_code: Some(code),
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
        options: None,
    }
}

fn build_explorer_request<'a>(
    model: &'a str,
    user_content: &'a str,
    prompt: &'a str,
    schema: &'a Value,
    context_tokens: u32,
) -> ChatRequest<'a> {
    let mut request = build_request(model, user_content, prompt, schema);
    request.options = Some(ChatOptions {
        temperature: 0.0,
        num_ctx: context_tokens,
    });
    request
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
    fn explorer_request_is_deterministic_unloaded_and_tool_free() {
        let schema = serde_json::json!({"type": "object"});
        let value = serde_json::to_value(build_explorer_request(
            "explorer",
            "trusted-json",
            "fixed-prompt",
            &schema,
            32_768,
        ))
        .unwrap();
        assert_eq!(value["model"], "explorer");
        assert_eq!(value["stream"], false);
        assert_eq!(value["keep_alive"], 0);
        assert_eq!(value["options"]["temperature"], 0.0);
        assert_eq!(value["options"]["num_ctx"], 32_768);
        assert!(value.get("tools").is_none());
    }

    #[test]
    fn explorer_timeout_code_does_not_leak_into_existing_roles() {
        assert_eq!(timeout_failure(None).stable_code, None);
        assert_eq!(
            timeout_failure(Some("explorer_timeout")).stable_code,
            Some("explorer_timeout")
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)] // Documents the relationship between two contracts.
    fn developer_http_payload_limit_is_larger_than_request_contract_limit() {
        assert!(MAX_DEVELOPER_HTTP_REQUEST_BYTES > crate::protocol::MAX_DEVELOPER_REQUEST_BYTES);
        let schema = serde_json::json!({"type": "object"});
        let oversized = "x".repeat(MAX_DEVELOPER_HTTP_REQUEST_BYTES);
        let request = build_request("developer", &oversized, "prompt", &schema);
        assert!(serialize_request(&request, Some(MAX_DEVELOPER_HTTP_REQUEST_BYTES)).is_err());
    }
}
