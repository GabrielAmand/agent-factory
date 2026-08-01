use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, ErrorKind};

pub const REGISTRY_VERSION: &str = "official-source-registry-v1";
const MAX_REGISTRY_URL_BYTES: usize = 2_048;
const MAX_PATH_PREFIX_BYTES: usize = 512;
const EXPECTED_FACTS: [(&str, &str); 8] = [
    ("docker", "Docker"),
    ("kubernetes", "Kubernetes"),
    ("terraform", "Terraform"),
    ("ansible", "Ansible"),
    ("jenkins", "Jenkins"),
    ("gitlab-ci", "GitLab CI"),
    ("prometheus", "Prometheus"),
    ("argo-cd", "Argo CD"),
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfficialSourceRegistry {
    pub registry_version: String,
    pub policy_id: String,
    pub topic: String,
    pub approval_status: ApprovalStatus,
    pub entries: Vec<SourceEntry>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEntry {
    pub source_id: String,
    pub fact_id: String,
    pub display_name: String,
    pub verification: VerificationStatus,
    pub allowed_https_hosts: Vec<AllowedHttpsHost>,
    pub canonical_official_url: Option<String>,
    pub canonical_source_url: Option<String>,
    pub redirect_policy: RedirectPolicy,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedHttpsHost {
    pub host: String,
    pub allowed_path_prefixes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    PendingAuthoritativeVerification,
    AuthoritativeVerified,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RedirectPolicy {
    SameApprovedHostOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryReadiness {
    pub structurally_valid: bool,
    pub complete: bool,
    pub authoritative_verification_pending: bool,
    pub retrieval_ready: bool,
}

impl OfficialSourceRegistry {
    pub fn parse_and_validate(json: &str) -> Result<Self, AppError> {
        let registry: Self = serde_json::from_str(json).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("source registry is not valid contract JSON: {error}"),
            )
        })?;
        registry.validate_structure()?;
        Ok(registry)
    }

    pub fn readiness(&self) -> RegistryReadiness {
        let complete = self.entries.len() == EXPECTED_FACTS.len()
            && self
                .entries
                .iter()
                .zip(EXPECTED_FACTS)
                .all(|(entry, expected)| {
                    entry.fact_id == expected.0 && entry.display_name == expected.1
                });
        let pending = self
            .entries
            .iter()
            .any(|entry| entry.verification != VerificationStatus::AuthoritativeVerified);
        let verified_fields_complete = self.entries.iter().all(SourceEntry::is_retrieval_complete);
        RegistryReadiness {
            structurally_valid: true,
            complete,
            authoritative_verification_pending: pending,
            retrieval_ready: complete
                && !pending
                && verified_fields_complete
                && self.approval_status == ApprovalStatus::Approved,
        }
    }

    pub fn require_retrieval_ready(&self) -> Result<(), AppError> {
        if !self.readiness().retrieval_ready {
            return Err(AppError::coded(
                ErrorKind::Validation,
                "registry_not_retrieval_ready",
                "source registry is incomplete, unverified, or unapproved",
            ));
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), AppError> {
        if self.registry_version != REGISTRY_VERSION
            || self.policy_id != "official-devops-tools-v1"
            || self.topic != "devops-tools"
        {
            return validation("unsupported registry version, policy, or topic");
        }
        if self.entries.is_empty() || self.entries.len() > 8 {
            return validation("registry entries must contain between 1 and 8 items");
        }
        let mut source_ids = HashSet::new();
        let mut fact_ids = HashSet::new();
        let mut names = HashSet::new();
        for entry in &self.entries {
            validate_identifier("source_id", &entry.source_id)?;
            validate_identifier("fact_id", &entry.fact_id)?;
            if entry.display_name.trim().is_empty() || entry.display_name.chars().count() > 100 {
                return validation("registry display_name must contain 1 to 100 characters");
            }
            if !source_ids.insert(entry.source_id.as_str()) {
                return validation("registry source IDs must be unique");
            }
            if !fact_ids.insert(entry.fact_id.as_str()) {
                return validation("registry fact IDs must be unique");
            }
            if !names.insert(entry.display_name.as_str()) {
                return validation("registry display names must be unique");
            }
            entry.validate_policy_fields()?;
        }
        Ok(())
    }
}

impl SourceEntry {
    fn is_retrieval_complete(&self) -> bool {
        !self.allowed_https_hosts.is_empty()
            && self
                .allowed_https_hosts
                .iter()
                .all(|host| !host.allowed_path_prefixes.is_empty())
            && self.canonical_official_url.is_some()
            && self.canonical_source_url.is_some()
    }

    fn validate_policy_fields(&self) -> Result<(), AppError> {
        if self.verification == VerificationStatus::PendingAuthoritativeVerification {
            if !self.allowed_https_hosts.is_empty()
                || self.canonical_official_url.is_some()
                || self.canonical_source_url.is_some()
            {
                return validation("pending entries must not contain unverified network policy");
            }
            return Ok(());
        }

        if self.allowed_https_hosts.is_empty() || self.allowed_https_hosts.len() > 4 {
            return validation("verified entry requires 1 to 4 exact HTTPS hosts");
        }
        let mut hosts = HashSet::new();
        for allowed_host in &self.allowed_https_hosts {
            validate_domain(&allowed_host.host)?;
            if !hosts.insert(allowed_host.host.as_str()) {
                return validation("approved HTTPS hosts must be unique within an entry");
            }
            if allowed_host.allowed_path_prefixes.is_empty()
                || allowed_host.allowed_path_prefixes.len() > 8
            {
                return validation("each approved host requires 1 to 8 path prefixes");
            }
            let mut prefixes = HashSet::new();
            for prefix in &allowed_host.allowed_path_prefixes {
                validate_path_prefix(prefix)?;
                if !prefixes.insert(prefix.as_str()) {
                    return validation("path prefixes must be unique within an approved host");
                }
            }
        }
        validate_canonical_url(
            self.canonical_official_url.as_deref(),
            &self.allowed_https_hosts,
        )?;
        validate_canonical_url(
            self.canonical_source_url.as_deref(),
            &self.allowed_https_hosts,
        )
    }
}

fn validate_canonical_url(
    value: Option<&str>,
    allowed_hosts: &[AllowedHttpsHost],
) -> Result<(), AppError> {
    let raw = value.ok_or_else(|| {
        AppError::new(ErrorKind::Validation, "verified entry lacks canonical URL")
    })?;
    if raw.len() > MAX_REGISTRY_URL_BYTES {
        return validation("canonical URL exceeds 2048 bytes");
    }
    let url = Url::parse(raw)
        .map_err(|error| AppError::new(ErrorKind::Validation, format!("invalid URL: {error}")))?;
    let matching_host = url
        .host_str()
        .and_then(|host| allowed_hosts.iter().find(|allowed| allowed.host == host));
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.as_str() != raw
    {
        return validation("canonical URL violates registry policy");
    }
    let Some(allowed_host) = matching_host else {
        return validation("canonical URL host is not exactly approved");
    };
    if !allowed_host
        .allowed_path_prefixes
        .iter()
        .any(|prefix| path_matches_prefix(url.path(), prefix))
    {
        return validation("canonical URL path violates its exact host policy");
    }
    Ok(())
}

fn validate_path_prefix(prefix: &str) -> Result<(), AppError> {
    if !prefix.starts_with('/')
        || prefix.len() > MAX_PATH_PREFIX_BYTES
        || (prefix.len() > 1 && prefix.ends_with('/'))
        || prefix.contains(['?', '#', '\\'])
        || prefix.split('/').any(|part| part == "..")
        || !prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-' | b'_'))
    {
        return validation("invalid allowed path prefix");
    }
    Ok(())
}

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn validate_domain(domain: &str) -> Result<(), AppError> {
    if domain.is_empty()
        || domain.len() > 253
        || !domain.contains('.')
        || domain.parse::<std::net::IpAddr>().is_ok()
        || domain != domain.to_ascii_lowercase()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return validation("invalid allowed HTTPS domain");
    }
    Ok(())
}

fn validate_identifier(name: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return validation(format!("invalid {name}"));
    }
    Ok(())
}

fn validation(message: impl Into<String>) -> Result<(), AppError> {
    Err(AppError::new(ErrorKind::Validation, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_registry_is_complete_verified_approved_and_retrieval_ready() {
        let registry = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../official-sources/official-devops-tools-v1.json"
        ))
        .unwrap();
        assert_eq!(
            registry.readiness(),
            RegistryReadiness {
                structurally_valid: true,
                complete: true,
                authoritative_verification_pending: false,
                retrieval_ready: true,
            }
        );
        assert!(registry.require_retrieval_ready().is_ok());
        assert_eq!(registry.entries[0].allowed_https_hosts.len(), 2);
        assert_eq!(registry.entries[5].allowed_https_hosts.len(), 2);
    }

    #[test]
    fn incomplete_pending_fixture_is_structural_but_not_complete() {
        let registry = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../tests/fixtures/e0/registry-pending-verification.json"
        ))
        .unwrap();
        let readiness = registry.readiness();
        assert!(readiness.structurally_valid);
        assert!(!readiness.complete);
        assert!(readiness.authoritative_verification_pending);
        assert!(!readiness.retrieval_ready);
    }

    #[test]
    fn retrieval_ready_fixture_is_complete_verified_and_approved() {
        let mut registry = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../tests/fixtures/e0/registry-valid.json"
        ))
        .unwrap();
        assert!(registry.readiness().retrieval_ready);

        registry.approval_status = ApprovalStatus::Pending;
        assert!(!registry.readiness().retrieval_ready);
        registry.approval_status = ApprovalStatus::Approved;
        registry.entries[0].verification = VerificationStatus::PendingAuthoritativeVerification;
        assert!(registry.readiness().authoritative_verification_pending);
        assert!(!registry.readiness().retrieval_ready);
    }

    #[test]
    fn rejects_invalid_registry_fixtures() {
        for fixture in [
            include_str!("../tests/fixtures/e0/registry-duplicate-source-id.json"),
            include_str!("../tests/fixtures/e0/registry-duplicate-fact-id.json"),
            include_str!("../tests/fixtures/e0/registry-duplicate-display-name.json"),
            include_str!("../tests/fixtures/e0/registry-invalid-host.json"),
            include_str!("../tests/fixtures/e0/registry-invalid-path-prefix.json"),
            include_str!("../tests/fixtures/e0/registry-invalid-canonical-url.json"),
            include_str!("../tests/fixtures/e0/registry-duplicate-host.json"),
            include_str!("../tests/fixtures/e0/registry-empty-hosts.json"),
            include_str!("../tests/fixtures/e0/registry-host-suffix-attempt.json"),
            include_str!("../tests/fixtures/e0/registry-invalid-redirect-policy.json"),
        ] {
            assert!(OfficialSourceRegistry::parse_and_validate(fixture).is_err());
        }
    }

    #[test]
    fn host_matching_is_exact_and_never_uses_suffixes() {
        let allowed = vec![AllowedHttpsHost {
            host: "docs.example.test".to_owned(),
            allowed_path_prefixes: vec!["/docs".to_owned()],
        }];
        assert!(
            validate_canonical_url(Some("https://docs.example.test/docs/start"), &allowed).is_ok()
        );
        for rejected in [
            "https://evil.docs.example.test/docs/start",
            "https://docs.example.test.evil.test/docs/start",
            "https://example.test/docs/start",
            "https://DOCS.example.test/docs/start",
        ] {
            assert!(validate_canonical_url(Some(rejected), &allowed).is_err());
        }
    }

    #[test]
    fn path_prefix_matching_respects_component_boundaries_and_root_is_exact() {
        assert!(path_matches_prefix("/ci", "/ci"));
        assert!(path_matches_prefix("/ci/pipelines", "/ci"));
        assert!(!path_matches_prefix("/ci-evil", "/ci"));
        assert!(path_matches_prefix("/", "/"));
        assert!(!path_matches_prefix("/anything", "/"));
    }
}
