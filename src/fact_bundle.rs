use std::collections::HashSet;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, ErrorKind};
use crate::explorer::ExplorerResponseV1;
use crate::retriever::RetrievalResult;
use crate::source_registry::OfficialSourceRegistry;

pub const FACT_BUNDLE_VERSION: &str = "fact-bundle-v1";
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_TAGS_PER_FACT: usize = 10;
pub const MAX_TAG_CHARS: usize = 32;
pub const FACT_BUNDLE_DIGEST_ALGORITHM: &str = "sha256";

#[derive(Debug, PartialEq, Eq)]
pub struct FactBundleDigest {
    pub algorithm: &'static str,
    pub hex: String,
    pub canonical_byte_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FactBundleV1 {
    pub bundle_version: &'static str,
    pub source_policy: String,
    pub facts: Vec<ValidatedFact>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatedFact {
    pub fact_id: String,
    pub display_name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub official_url: String,
    pub source_url: String,
    pub source_ids: Vec<String>,
}

impl FactBundleV1 {
    pub fn from_explorer(
        registry: &OfficialSourceRegistry,
        retrievals: &[RetrievalResult],
        semantic_results: Vec<ExplorerResponseV1>,
    ) -> Result<Self, AppError> {
        registry.require_retrieval_ready()?;
        if semantic_results.len() != registry.entries.len()
            || retrievals.len() != registry.entries.len()
        {
            return validation("Explorer response has missing or unsupported items");
        }
        let mut facts = Vec::with_capacity(registry.entries.len());
        for ((entry, retrieval), item) in registry
            .entries
            .iter()
            .zip(retrievals)
            .zip(semantic_results)
        {
            let official_url = entry
                .canonical_official_url
                .as_deref()
                .ok_or_else(|| AppError::new(ErrorKind::Validation, "official URL is missing"))?;
            let source_url = entry.source_url()?;
            if retrieval.policy_id != registry.policy_id
                || retrieval.fact_id != entry.fact_id
                || retrieval.source_id != entry.source_id
                || retrieval.requested_canonical_url != source_url
                || retrieval.normalized_text.is_empty()
            {
                return validation("completed retrieval set does not match registry order");
            }
            let mut tags = validate_and_normalize_explorer_output(&item)
                .map_err(|_| AppError::new(ErrorKind::Validation, "semantic fields are invalid"))?;
            tags.sort();
            facts.push(ValidatedFact {
                fact_id: entry.fact_id.clone(),
                display_name: entry.display_name.clone(),
                description: item.description,
                tags,
                official_url: official_url.to_owned(),
                source_url: source_url.to_owned(),
                source_ids: vec![retrieval.source_id.clone()],
            });
        }
        Ok(Self {
            bundle_version: FACT_BUNDLE_VERSION,
            source_policy: registry.policy_id.clone(),
            facts,
        })
    }

    /// Canonical digest input: compact UTF-8 JSON, fixed struct key order, registry fact order,
    /// lexically sorted tags, and lexically sorted source IDs.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AppError> {
        serde_json::to_vec(self).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("could not serialize canonical fact bundle: {error}"),
            )
        })
    }

    pub fn digest(&self) -> Result<FactBundleDigest, AppError> {
        let bytes = self.canonical_bytes().map_err(|_| {
            AppError::coded(
                ErrorKind::Validation,
                "fact_bundle_digest_failed",
                "could not prepare canonical FactBundle bytes",
            )
        })?;
        let hex = format!("{:x}", Sha256::digest(&bytes));
        Ok(FactBundleDigest {
            algorithm: FACT_BUNDLE_DIGEST_ALGORITHM,
            hex,
            canonical_byte_count: bytes.len(),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SemanticValidationFailure {
    pub field: &'static str,
    pub reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_utf8_bytes: Option<usize>,
    pub tag_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_tag_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_tag_utf8_bytes: Option<usize>,
}

fn description_failure(value: &str, tag_count: usize) -> Option<SemanticValidationFailure> {
    let trimmed_count = value.trim().chars().count();
    let lower = value.to_ascii_lowercase();
    let reason = if trimmed_count == 0 {
        "description_empty"
    } else if trimmed_count > MAX_DESCRIPTION_CHARS {
        "description_too_long"
    } else if value.contains(['\r', '\n']) {
        "description_not_single_line"
    } else if value.chars().any(char::is_control) {
        "description_contains_control_character"
    } else if lower.contains("<script") || lower.contains("javascript:") {
        "description_contains_script"
    } else if value.contains(['<', '>']) {
        "description_contains_html"
    } else if lower.contains("http://") || lower.contains("https://") {
        "description_contains_url"
    } else if value.contains(['`', '*', '#', '[', ']', '{', '}', '|', '$', '\\'])
        || lower.contains("](")
    {
        "description_contains_markdown"
    } else if is_command_like(&lower) {
        "description_command_like"
    } else {
        return None;
    };
    Some(SemanticValidationFailure {
        field: "description",
        reason,
        description_utf8_bytes: Some(value.len()),
        tag_count,
        rejected_tag_index: None,
        rejected_tag_utf8_bytes: None,
    })
}

fn normalize_tags(tags: &[String]) -> Result<Vec<String>, SemanticValidationFailure> {
    if tags.is_empty() {
        return Err(tag_failure("tags_empty", tags, None));
    }
    if tags.len() > MAX_TAGS_PER_FACT {
        return Err(tag_failure("too_many_tags", tags, None));
    }
    let mut unique = HashSet::new();
    let mut normalized_tags = Vec::with_capacity(tags.len());
    for (index, tag) in tags.iter().enumerate() {
        if !tag.is_ascii() {
            return Err(tag_failure("tag_non_ascii", tags, Some(index)));
        }
        if tag
            .chars()
            .any(|character| character.is_control() && !character.is_ascii_whitespace())
        {
            return Err(tag_failure(
                "tag_contains_control_character",
                tags,
                Some(index),
            ));
        }
        let mut normalized = String::with_capacity(tag.len());
        let mut pending_hyphen = false;
        for byte in tag
            .trim_matches(|character: char| character.is_ascii_whitespace())
            .bytes()
        {
            if byte.is_ascii_whitespace() || byte == b'-' {
                pending_hyphen = !normalized.is_empty();
            } else if byte.is_ascii_alphanumeric() {
                if pending_hyphen {
                    normalized.push('-');
                    pending_hyphen = false;
                }
                normalized.push(byte.to_ascii_lowercase() as char);
            }
        }
        if normalized.is_empty() {
            return Err(tag_failure(
                "tag_empty_after_normalization",
                tags,
                Some(index),
            ));
        }
        if normalized.len() > MAX_TAG_CHARS {
            return Err(tag_failure(
                "tag_too_long_after_normalization",
                tags,
                Some(index),
            ));
        }
        if !unique.insert(normalized.clone()) {
            return Err(tag_failure(
                "duplicate_tag_after_normalization",
                tags,
                Some(index),
            ));
        }
        normalized_tags.push(normalized);
    }
    Ok(normalized_tags)
}

fn tag_failure(
    reason: &'static str,
    tags: &[String],
    rejected_index: Option<usize>,
) -> SemanticValidationFailure {
    SemanticValidationFailure {
        field: "tags",
        reason,
        description_utf8_bytes: None,
        tag_count: tags.len(),
        rejected_tag_index: rejected_index,
        rejected_tag_utf8_bytes: rejected_index.map(|index| tags[index].len()),
    }
}

pub(crate) fn validate_and_normalize_explorer_output(
    value: &ExplorerResponseV1,
) -> Result<Vec<String>, SemanticValidationFailure> {
    if let Some(failure) = description_failure(&value.description, value.tags.len()) {
        return Err(failure);
    }
    normalize_tags(&value.tags)
}

fn is_command_like(lower: &str) -> bool {
    lower.contains("#!/")
        || lower.contains("curl ")
        || lower.contains("wget ")
        || lower.starts_with("rm ")
        || lower.starts_with("sudo ")
        || lower.starts_with("sh ")
        || lower.starts_with("bash ")
        || [
            "-----begin ",
            "authorization:",
            "bearer ",
            "api_key=",
            "api-key=",
            "apikey=",
            "client_secret=",
            "client-secret=",
            "password=",
            "access_token=",
            "access-token=",
            "akia",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn validation<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::new(ErrorKind::Validation, message))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::retriever::AddressFamily;

    fn registry() -> OfficialSourceRegistry {
        OfficialSourceRegistry::parse_and_validate(include_str!(
            "../official-sources/official-devops-tools-v2.json"
        ))
        .unwrap()
    }

    fn retrievals(registry: &OfficialSourceRegistry) -> Vec<RetrievalResult> {
        registry
            .entries
            .iter()
            .map(|entry| RetrievalResult {
                policy_id: registry.policy_id.clone(),
                fact_id: entry.fact_id.clone(),
                source_id: entry.source_id.clone(),
                requested_canonical_url: entry.source_url().unwrap().to_owned(),
                final_url: entry.source_url().unwrap().to_owned(),
                redirect_chain: vec![],
                selected_address_family: AddressFamily::Ipv4,
                http_status: 200,
                content_type: "text/html".to_owned(),
                charset: Some("utf-8".to_owned()),
                content_encoding: None,
                normalized_text: "Synthetic evidence".to_owned(),
                original_byte_count: 18,
                decoded_byte_count: 18,
                normalized_byte_count: 18,
                elapsed_ms: 1,
                retrieved_at: Utc::now(),
            })
            .collect()
    }

    fn semantic_results() -> Vec<ExplorerResponseV1> {
        [
            (
                "Docker provides tools for building and running containerized applications.",
                &["containers", "development"][..],
            ),
            (
                "Kubernetes manages containerized workloads using declarative configuration.",
                &["containers", "orchestration"][..],
            ),
            (
                "Terraform manages infrastructure through declarative configuration files.",
                &["infrastructure", "provisioning"][..],
            ),
            (
                "Jenkins supports automation for building testing and delivering software.",
                &["automation", "ci-cd"][..],
            ),
            (
                "GitLab CI defines automated software pipelines within GitLab projects.",
                &["automation", "ci-cd"][..],
            ),
            (
                "Prometheus collects time series metrics and evaluates monitoring rules.",
                &["metrics", "monitoring"][..],
            ),
        ]
        .into_iter()
        .map(|(description, tags)| ExplorerResponseV1 {
            description: description.to_owned(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        })
        .collect()
    }

    fn build(semantic_results: Vec<ExplorerResponseV1>) -> Result<FactBundleV1, AppError> {
        let registry = registry();
        let retrievals = retrievals(&registry);
        FactBundleV1::from_explorer(&registry, &retrievals, semantic_results)
    }

    #[test]
    fn ordered_outputs_receive_only_rust_owned_metadata() {
        let registry = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../official-sources/official-devops-tools-v2.json"
        ))
        .unwrap();
        let retrievals = retrievals(&registry);
        let bundle =
            FactBundleV1::from_explorer(&registry, &retrievals, semantic_results()).unwrap();
        assert_eq!(bundle.facts[0].fact_id, "docker");
        assert_eq!(bundle.facts[0].source_ids, ["docker-official"]);
        assert_eq!(bundle.facts[1].fact_id, "kubernetes");
        assert_eq!(bundle.facts[1].source_ids, ["kubernetes-official"]);
        assert_eq!(
            bundle.canonical_bytes().unwrap(),
            bundle.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn missing_extra_and_invalid_semantic_outputs_are_rejected() {
        let mut missing = semantic_results();
        missing.pop();
        assert!(build(missing).is_err());

        let mut extra = semantic_results();
        extra.push(extra[0].clone());
        assert!(build(extra).is_err());

        let mut invalid_description = semantic_results();
        invalid_description[0].description = "x".repeat(501);
        assert!(build(invalid_description).is_err());

        let mut invalid_tag = semantic_results();
        invalid_tag[0].tags[0] = "café".into();
        assert!(build(invalid_tag).is_err());
    }

    #[test]
    fn retrieval_order_cannot_be_overridden_by_model_content() {
        let registry = registry();
        let mut retrievals = retrievals(&registry);
        retrievals.swap(0, 1);
        assert!(FactBundleV1::from_explorer(&registry, &retrievals, semantic_results()).is_err());
    }

    #[test]
    fn classifies_description_failures_without_exposing_text() {
        for (value, reason) in [
            ("", "description_empty"),
            (&"x".repeat(501), "description_too_long"),
            ("first\nsecond", "description_not_single_line"),
            ("bad\0text", "description_contains_control_character"),
            ("<script>bad</script>", "description_contains_script"),
            ("<b>bad</b>", "description_contains_html"),
            ("visit https://example.test", "description_contains_url"),
            ("**bold claim**", "description_contains_markdown"),
            ("curl service", "description_command_like"),
        ] {
            let output = ExplorerResponseV1 {
                description: value.to_owned(),
                tags: vec!["valid".to_owned()],
            };
            let failure = validate_and_normalize_explorer_output(&output).unwrap_err();
            assert_eq!(failure.field, "description");
            assert_eq!(failure.reason, reason);
            assert_eq!(failure.description_utf8_bytes, Some(value.len()));
            let diagnostic = serde_json::to_string(&failure).unwrap();
            if !value.is_empty() {
                assert!(!diagnostic.contains(value));
            }
        }
    }

    #[test]
    fn classifies_tag_failures_without_exposing_values() {
        let cases = [
            (vec![], "tags_empty", None),
            (vec!["leakvalue"; 11], "too_many_tags", None),
            (
                vec!["Cloud Native", "cloud-native"],
                "duplicate_tag_after_normalization",
                Some(1),
            ),
            (vec!["---"], "tag_empty_after_normalization", Some(0)),
            (
                vec!["abcdefghijklmnopqrstuvwxyzabcdefg"],
                "tag_too_long_after_normalization",
                Some(0),
            ),
            (vec!["café"], "tag_non_ascii", Some(0)),
            (
                vec!["bad\u{1}tag"],
                "tag_contains_control_character",
                Some(0),
            ),
        ];
        for (values, reason, index) in cases {
            let tags = values.iter().map(|value| (*value).to_owned()).collect();
            let output = ExplorerResponseV1 {
                description: "Valid description".to_owned(),
                tags,
            };
            let failure = validate_and_normalize_explorer_output(&output).unwrap_err();
            assert_eq!(failure.field, "tags");
            assert_eq!(failure.reason, reason);
            assert_eq!(failure.rejected_tag_index, index);
            let diagnostic = serde_json::to_string(&failure).unwrap();
            for value in values.into_iter().filter(|value| !value.is_empty()) {
                assert!(!diagnostic.contains(value));
            }
        }
    }

    #[test]
    fn normalizes_tags_deterministically_and_stores_only_normalized_forms() {
        let output = ExplorerResponseV1 {
            description: "Valid description".to_owned(),
            tags: [
                "Container Orchestration",
                "cloud native",
                "CI/CD",
                "  observability  ",
                "cloud---platform",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        };
        let expected = [
            "container-orchestration",
            "cloud-native",
            "cicd",
            "observability",
            "cloud-platform",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert_eq!(
            validate_and_normalize_explorer_output(&output).unwrap(),
            expected
        );
        assert_eq!(
            validate_and_normalize_explorer_output(&output).unwrap(),
            expected
        );

        let registry = registry();
        let retrievals = retrievals(&registry);
        let mut results = semantic_results();
        results[0] = output;
        let bundle = FactBundleV1::from_explorer(&registry, &retrievals, results).unwrap();
        let mut sorted = expected;
        sorted.sort();
        assert_eq!(bundle.facts[0].tags, sorted);
    }
}
