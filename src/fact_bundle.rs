use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::error::{AppError, ErrorKind};
use crate::explorer::{ExplorerFact, ExplorerResponseV1};
use crate::source_registry::OfficialSourceRegistry;

pub const FACT_BUNDLE_VERSION: &str = "fact-bundle-v1";
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_TAGS_PER_FACT: usize = 10;
pub const MAX_TAG_CHARS: usize = 32;

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
        response: ExplorerResponseV1,
    ) -> Result<Self, AppError> {
        registry.require_retrieval_ready()?;
        if response.items.len() != registry.entries.len() {
            return validation("Explorer response has missing or unsupported items");
        }
        let items: HashMap<String, ExplorerFact> = response
            .items
            .into_iter()
            .map(|item| (item.fact_id.clone(), item))
            .collect();
        if items.len() != registry.entries.len() {
            return validation("Explorer fact IDs must be unique");
        }

        let known_sources: HashSet<&str> = registry
            .entries
            .iter()
            .map(|entry| entry.source_id.as_str())
            .collect();
        let mut names = HashSet::new();
        let mut facts = Vec::with_capacity(registry.entries.len());
        for entry in &registry.entries {
            let item = items
                .get(&entry.fact_id)
                .ok_or_else(|| AppError::new(ErrorKind::Validation, "Explorer fact is missing"))?;
            if item.display_name != entry.display_name {
                return validation("Explorer changed a registry display name");
            }
            if !names.insert(item.display_name.as_str()) {
                return validation("Explorer display names must be unique");
            }
            if Some(item.official_url.as_str()) != entry.canonical_official_url.as_deref()
                || Some(item.source_url.as_str()) != entry.canonical_source_url.as_deref()
            {
                return validation("Explorer changed or invented a URL");
            }
            validate_description(&item.description)?;
            validate_tags(&item.tags)?;
            if item.source_ids.is_empty()
                || item.source_ids.len() > 8
                || !item
                    .source_ids
                    .iter()
                    .all(|source| known_sources.contains(source.as_str()))
                || !item
                    .source_ids
                    .iter()
                    .any(|source| source == &entry.source_id)
            {
                return validation("Explorer supplied unknown or incomplete source provenance");
            }
            let unique_sources: HashSet<&str> =
                item.source_ids.iter().map(String::as_str).collect();
            if unique_sources.len() != item.source_ids.len() {
                return validation("Explorer source IDs must be unique per fact");
            }
            let mut tags = item.tags.clone();
            tags.sort();
            let mut source_ids = item.source_ids.clone();
            source_ids.sort();
            facts.push(ValidatedFact {
                fact_id: item.fact_id.clone(),
                display_name: item.display_name.clone(),
                description: item.description.clone(),
                tags,
                official_url: item.official_url.clone(),
                source_url: item.source_url.clone(),
                source_ids,
            });
        }
        Ok(Self {
            bundle_version: FACT_BUNDLE_VERSION,
            source_policy: registry.policy_id.clone(),
            facts,
        })
    }

    /// Canonical digest input: compact UTF-8 JSON, fixed struct key order, registry fact order.
    /// E0 deliberately does not hash these bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AppError> {
        serde_json::to_vec(self).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("could not serialize canonical fact bundle: {error}"),
            )
        })
    }
}

fn validate_description(value: &str) -> Result<(), AppError> {
    let count = value.trim().chars().count();
    if !(1..=MAX_DESCRIPTION_CHARS).contains(&count)
        || contains_non_plain_text(value)
        || value.contains(['\0', '\r', '\n', '\t'])
    {
        return validation("fact description must be bounded plain text");
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> Result<(), AppError> {
    if !(1..=MAX_TAGS_PER_FACT).contains(&tags.len()) {
        return validation("each fact requires 1 to 10 tags");
    }
    let mut unique = HashSet::new();
    for tag in tags {
        if tag.is_empty()
            || tag.len() > MAX_TAG_CHARS
            || !tag
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || !unique.insert(tag.as_str())
        {
            return validation("fact tags must be unique conservative ASCII labels");
        }
    }
    Ok(())
}

fn contains_non_plain_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.contains(['<', '>', '`', '*', '#', '[', ']', '{', '}', '|', '$', '\\'])
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("](")
        || lower.contains("<script")
        || lower.contains("#!/")
        || lower.contains("curl ")
        || lower.contains("wget ")
        || lower.starts_with("rm ")
        || lower.starts_with("sudo ")
        || lower.starts_with("sh ")
        || lower.starts_with("bash ")
}

fn validation<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::new(ErrorKind::Validation, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(fixture: &str) -> Result<FactBundleV1, AppError> {
        let registry = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../tests/fixtures/e0/registry-valid.json"
        ))?;
        let response = ExplorerResponseV1::parse(fixture)?;
        FactBundleV1::from_explorer(&registry, response)
    }

    fn build_mutation(fixture: &str) -> Result<FactBundleV1, AppError> {
        let mutation: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let mutation = mutation["mutation"].as_str().unwrap();
        let mut response: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/e0/explorer-valid-reordered.json"
        ))
        .unwrap();
        let items = response["items"].as_array_mut().unwrap();
        match mutation {
            "invented_fact" => items[0]["fact_id"] = "invented".into(),
            "changed_url" => items[0]["official_url"] = "https://docs.example.test/invented".into(),
            "changed_name" => items[0]["display_name"] = "Changed".into(),
            "unknown_source" => items[0]["source_ids"][0] = "unknown".into(),
            "duplicate_fact" => items[0]["fact_id"] = "prometheus".into(),
            "missing_fact" => {
                items.pop();
            }
            "invalid_description" => items[0]["description"] = "x".repeat(501).into(),
            "invalid_tag" => items[0]["tags"][0] = "Not Conservative".into(),
            _ => panic!("unknown test mutation"),
        }
        build(&serde_json::to_string(&response).unwrap())
    }

    #[test]
    fn bundle_uses_registry_order_and_has_deterministic_canonical_bytes() {
        let bundle = build(include_str!(
            "../tests/fixtures/e0/explorer-valid-reordered.json"
        ))
        .unwrap();
        assert_eq!(bundle.facts[0].fact_id, "docker");
        assert_eq!(bundle.facts[7].fact_id, "argo-cd");
        assert_eq!(
            bundle.canonical_bytes().unwrap(),
            bundle.canonical_bytes().unwrap()
        );
        assert_eq!(
            String::from_utf8(bundle.canonical_bytes().unwrap()).unwrap(),
            include_str!("../tests/fixtures/e0/fact-bundle-canonical.json").trim()
        );
    }

    #[test]
    fn rejects_provenance_and_fact_mutations() {
        for fixture in [
            include_str!("../tests/fixtures/e0/explorer-invented-fact.json"),
            include_str!("../tests/fixtures/e0/explorer-changed-url.json"),
            include_str!("../tests/fixtures/e0/explorer-changed-name.json"),
            include_str!("../tests/fixtures/e0/explorer-unknown-source.json"),
            include_str!("../tests/fixtures/e0/explorer-duplicate-fact.json"),
            include_str!("../tests/fixtures/e0/explorer-missing-fact.json"),
            include_str!("../tests/fixtures/e0/explorer-invalid-description.json"),
            include_str!("../tests/fixtures/e0/explorer-invalid-tag.json"),
        ] {
            assert!(build_mutation(fixture).is_err());
        }
    }

    #[test]
    fn rejects_markup_markdown_and_command_like_descriptions() {
        for value in [
            "<script>bad</script>",
            "**bold claim**",
            "[official](https://example.test)",
            "curl https://example.test",
            "rm generated-file",
        ] {
            assert!(validate_description(value).is_err());
        }
    }
}
