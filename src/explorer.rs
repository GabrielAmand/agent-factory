use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};
use crate::research::REQUIRED_FACT_FIELDS;

pub const EXPLORER_REQUEST_VERSION: &str = "explorer-request-v1";
pub const EXPLORER_RESPONSE_VERSION: &str = "explorer-response-v1";
pub const MAX_EXPLORER_REQUEST_BYTES: usize = 160 * 1024;
pub const MAX_EXPLORER_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_DOCUMENTS: usize = 8;
pub const MAX_DOCUMENT_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_COMBINED_DOCUMENT_TEXT_BYTES: usize = 96 * 1024;

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerRequestV1 {
    pub request_version: &'static str,
    pub topic: String,
    pub requested_count: usize,
    pub required_fields: Vec<String>,
    pub source_policy: String,
    pub retrieved_documents: Vec<RetrievedDocument>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievedDocument {
    pub source_id: String,
    pub canonical_url: String,
    pub normalized_text: String,
}

impl ExplorerRequestV1 {
    pub fn to_bounded_json(&self) -> Result<String, AppError> {
        if self.request_version != EXPLORER_REQUEST_VERSION
            || self.topic != "devops-tools"
            || self.source_policy != "official-devops-tools-v1"
            || self.requested_count != 8
            || !(1..=MAX_DOCUMENTS).contains(&self.retrieved_documents.len())
            || self.required_fields.as_slice() != REQUIRED_FACT_FIELDS
        {
            return validation("invalid Explorer request policy fields");
        }
        let mut source_ids = HashSet::new();
        let mut combined = 0;
        for document in &self.retrieved_documents {
            if document.source_id.is_empty()
                || document.source_id.len() > 64
                || !document
                    .source_id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !source_ids.insert(document.source_id.as_str())
            {
                return validation("Explorer request source IDs must be unique");
            }
            let url = url::Url::parse(&document.canonical_url).map_err(|error| {
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
                || document.canonical_url.len() > 2_048
            {
                return validation("Explorer document URL violates the trusted contract");
            }
            if document.normalized_text.is_empty()
                || document.normalized_text.len() > MAX_DOCUMENT_TEXT_BYTES
            {
                return validation("Explorer document text is empty or too large");
            }
            combined += document.normalized_text.len();
        }
        if combined > MAX_COMBINED_DOCUMENT_TEXT_BYTES {
            return validation("combined Explorer document text is too large");
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerResponseV1 {
    pub response_version: String,
    pub items: Vec<ExplorerFact>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerFact {
    pub fact_id: String,
    pub display_name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub official_url: String,
    pub source_url: String,
    pub source_ids: Vec<String>,
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
        if response.response_version != EXPLORER_RESPONSE_VERSION {
            return validation("unsupported Explorer response_version");
        }
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
            requested_count: 8,
            required_fields: REQUIRED_FACT_FIELDS
                .iter()
                .map(|field| (*field).to_owned())
                .collect(),
            source_policy: "official-devops-tools-v1".to_owned(),
            retrieved_documents: vec![RetrievedDocument {
                source_id: "docker-docs".to_owned(),
                canonical_url: "https://docs.example.test/docker".to_owned(),
                normalized_text: "Bounded official fixture text.".to_owned(),
            }],
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
}
