use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::error::{AppError, ErrorKind};
use crate::protocol::LeadResponse;

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
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

pub struct OllamaSuccess {
    pub lead_response: LeadResponse,
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
) -> Result<OllamaSuccess, OllamaFailure> {
    let request = build_request(&config.model, user_request, prompt, schema);
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
        .send_json(&request)
        .map_err(|error| OllamaFailure {
            error: Box::new(AppError::new(
                ErrorKind::Network,
                format!("Ollama request failed: {error}"),
            )),
            metrics: None,
        })?;

    if response.status().is_redirection() {
        return Err(OllamaFailure {
            error: Box::new(AppError::new(
                ErrorKind::Response,
                "Ollama redirects are not allowed",
            )),
            metrics: None,
        });
    }

    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_vec()
        .map_err(|error| OllamaFailure {
            error: Box::new(AppError::new(
                ErrorKind::Response,
                format!("Ollama response exceeded limits or could not be read: {error}"),
            )),
            metrics: None,
        })?;

    let envelope: ChatResponse = serde_json::from_slice(&body).map_err(|error| OllamaFailure {
        error: Box::new(AppError::new(
            ErrorKind::Response,
            format!("Ollama returned an invalid response envelope: {error}"),
        )),
        metrics: None,
    })?;
    let metrics = metrics_from(&envelope);
    let lead_response =
        LeadResponse::parse_and_validate(&envelope.message.content).map_err(|error| {
            OllamaFailure {
                error: Box::new(error),
                metrics: Some(metrics_from(&envelope)),
            }
        })?;

    Ok(OllamaSuccess {
        lead_response,
        metrics,
    })
}

fn build_request<'a>(
    model: &'a str,
    user_request: &'a str,
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
                content: user_request,
            },
        ],
        stream: false,
        format: schema,
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
    fn request_is_non_streaming_and_contains_no_tools() {
        let schema = serde_json::json!({"type": "object"});
        let request = build_request("model", "request", "prompt", &schema);
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["stream"], false);
        assert_eq!(value["messages"].as_array().unwrap().len(), 2);
        assert!(value.get("tools").is_none());
    }
}
