use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};
use crate::retriever::RetrievalResult;
use crate::source_registry::{OfficialSourceRegistry, SourceEntry};

pub const EXPLORER_REQUEST_VERSION: &str = "explorer-request-v1";
pub const MAX_EXPLORER_REQUEST_BYTES: usize = 160 * 1024;
pub const MAX_EXPLORER_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_DOCUMENT_TEXT_BYTES: usize = 16 * 1024;
const MAX_MODEL_CHARS: usize = 200;
const MAX_EXPLORER_TIMEOUT_SECONDS: u64 = 600;
const MAX_EXPLORER_CONTEXT_TOKENS: u32 = 65_536;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExplorerConfig {
    pub enabled: bool,
    pub model: String,
    pub timeout_seconds: u64,
    pub context_tokens: u32,
}

impl Default for ExplorerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: "gemma3:latest".to_owned(),
            timeout_seconds: 300,
            context_tokens: 32_768,
        }
    }
}

impl ExplorerConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.model.trim().is_empty()
            || self.model.chars().count() > MAX_MODEL_CHARS
            || !(1..=MAX_EXPLORER_TIMEOUT_SECONDS).contains(&self.timeout_seconds)
            || !(1..=MAX_EXPLORER_CONTEXT_TOKENS).contains(&self.context_tokens)
        {
            return Err(AppError::coded(
                ErrorKind::Configuration,
                "explorer_configuration_invalid",
                "Explorer configuration exceeds its security limits",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerRequestV1 {
    pub request_version: &'static str,
    pub topic: String,
    pub source_policy: String,
    pub retrieved_document: RetrievedDocument,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievedDocument {
    pub fact_id: String,
    pub display_name: String,
    pub source_id: String,
    pub official_url: String,
    pub source_url: String,
    pub normalized_text: String,
}

impl ExplorerRequestV1 {
    pub fn from_retrieval(
        registry: &OfficialSourceRegistry,
        entry: &SourceEntry,
        retrieval: &RetrievalResult,
    ) -> Result<Self, AppError> {
        registry.require_retrieval_ready()?;
        let official_url = entry.canonical_official_url.as_deref().ok_or_else(|| {
            AppError::coded(
                ErrorKind::Validation,
                "explorer_request_invalid",
                "registry official URL is unavailable",
            )
        })?;
        let source_url = entry.source_url()?;
        if retrieval.policy_id != registry.policy_id
            || retrieval.fact_id != entry.fact_id
            || retrieval.source_id != entry.source_id
            || retrieval.requested_canonical_url != source_url
            || retrieval.normalized_text.is_empty()
        {
            return validation("retrieval does not match deterministic registry metadata");
        }
        let request = Self {
            request_version: EXPLORER_REQUEST_VERSION,
            topic: registry.topic.clone(),
            source_policy: registry.policy_id.clone(),
            retrieved_document: RetrievedDocument {
                fact_id: entry.fact_id.clone(),
                display_name: entry.display_name.clone(),
                source_id: entry.source_id.clone(),
                official_url: official_url.to_owned(),
                source_url: source_url.to_owned(),
                normalized_text: retrieval.normalized_text.clone(),
            },
        };
        request.to_bounded_json()?;
        Ok(request)
    }

    pub fn to_bounded_json(&self) -> Result<String, AppError> {
        if self.request_version != EXPLORER_REQUEST_VERSION
            || self.topic != "devops-tools"
            || !matches!(
                self.source_policy.as_str(),
                "official-devops-tools-v1" | "official-devops-tools-v2"
            )
        {
            return validation("invalid Explorer request policy fields");
        }
        let document = &self.retrieved_document;
        if document.fact_id.is_empty()
            || document.fact_id.len() > 64
            || !document
                .fact_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || document.display_name.trim().is_empty()
            || document.display_name.chars().count() > 100
            || document.source_id.is_empty()
            || document.source_id.len() > 64
            || !document
                .source_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return validation("Explorer request contains an invalid trusted identifier");
        }
        for value in [&document.official_url, &document.source_url] {
            let url = url::Url::parse(value).map_err(|error| {
                AppError::new(
                    ErrorKind::Validation,
                    format!("invalid Explorer document URL: {error}"),
                )
            })?;
            if url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || value.len() > 2_048
            {
                return validation("Explorer document URL violates the trusted contract");
            }
        }
        if document.normalized_text.is_empty()
            || document.normalized_text.len() > MAX_DOCUMENT_TEXT_BYTES
        {
            return validation("Explorer document text is empty or too large");
        }
        let json = serde_json::to_string(self).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("could not serialize Explorer request: {error}"),
            )
        })?;
        if json.len() > MAX_EXPLORER_REQUEST_BYTES {
            return validation("Explorer request exceeds 163840 bytes");
        }
        Ok(json)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerResponseV1 {
    pub description: String,
    pub tags: Vec<String>,
}

impl ExplorerResponseV1 {
    pub fn parse(json: &str) -> Result<Self, AppError> {
        if json.len() > MAX_EXPLORER_RESPONSE_BYTES {
            return validation("Explorer response exceeds 65536 bytes");
        }
        let response: Self = serde_json::from_str(json).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("Explorer response is not valid contract JSON: {error}"),
            )
        })?;
        Ok(response)
    }
}

fn validation<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::new(ErrorKind::Validation, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_request_serialization_is_bounded_and_strictly_trusted() {
        let request = ExplorerRequestV1 {
            request_version: EXPLORER_REQUEST_VERSION,
            topic: "devops-tools".to_owned(),
            source_policy: "official-devops-tools-v1".to_owned(),
            retrieved_document: RetrievedDocument {
                fact_id: "docker".to_owned(),
                display_name: "Docker".to_owned(),
                source_id: "docker-docs".to_owned(),
                official_url: "https://www.example.test/".to_owned(),
                source_url: "https://docs.example.test/docker".to_owned(),
                normalized_text: "Bounded official fixture text.".to_owned(),
            },
        };
        let serialized = request.to_bounded_json().unwrap();
        assert!(serialized.len() < MAX_EXPLORER_REQUEST_BYTES);
        assert!(!serialized.contains("headers"));
    }

    #[test]
    fn explorer_response_denies_unknown_fields() {
        let attempted = include_str!("../tests/fixtures/e0/explorer-invented-field.json");
        assert!(ExplorerResponseV1::parse(attempted).is_err());
    }

    #[test]
    fn explorer_request_enforces_single_document_utf8_byte_limit() {
        let request = ExplorerRequestV1 {
            request_version: EXPLORER_REQUEST_VERSION,
            topic: "devops-tools".to_owned(),
            source_policy: "official-devops-tools-v1".to_owned(),
            retrieved_document: RetrievedDocument {
                fact_id: "docker".to_owned(),
                display_name: "Docker".to_owned(),
                source_id: "docker-official".to_owned(),
                official_url: "https://www.example.test/".to_owned(),
                source_url: "https://docs.example.test/docker".to_owned(),
                normalized_text: "x".repeat(MAX_DOCUMENT_TEXT_BYTES + 1),
            },
        };
        assert!(request.to_bounded_json().is_err());
    }
}
