use std::collections::HashSet;
use std::fmt;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::DefaultConnector;
use url::Url;

use crate::document_normalizer::{normalize_html, normalize_plain_text};
use crate::network_policy::{address_is_public, validate_registry_url};
use crate::source_registry::{OfficialSourceRegistry, RedirectPolicy, SourceEntry};

pub const MAX_NORMALIZED_BYTES_HARD: usize = 16 * 1024;
const MAX_BODY_BYTES_HARD: usize = 1024 * 1024;
const MAX_HEADER_BYTES_HARD: usize = 64 * 1024;
const MAX_REDIRECTS_HARD: u8 = 4;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RetrieverConfig {
    pub enabled: bool,
    pub dns_timeout_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
    pub total_timeout_seconds: u64,
    pub maximum_redirects: u8,
    pub maximum_compressed_bytes: usize,
    pub maximum_decompressed_bytes: usize,
    pub maximum_normalized_bytes: usize,
    pub maximum_response_header_bytes: usize,
    pub user_agent: String,
}

impl Default for RetrieverConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dns_timeout_seconds: 2,
            connect_timeout_seconds: 3,
            request_timeout_seconds: 15,
            total_timeout_seconds: 60,
            maximum_redirects: 2,
            maximum_compressed_bytes: 512 * 1024,
            maximum_decompressed_bytes: 512 * 1024,
            maximum_normalized_bytes: MAX_NORMALIZED_BYTES_HARD,
            maximum_response_header_bytes: 32 * 1024,
            user_agent: "agent-factory/0.1 official-source-retriever".to_owned(),
        }
    }
}

impl RetrieverConfig {
    pub fn validate(&self) -> Result<(), RetrievalError> {
        if self.dns_timeout_seconds == 0
            || self.dns_timeout_seconds > 10
            || self.connect_timeout_seconds == 0
            || self.connect_timeout_seconds > 10
            || self.request_timeout_seconds == 0
            || self.request_timeout_seconds > 30
            || self.total_timeout_seconds == 0
            || self.total_timeout_seconds > 120
            || self.maximum_redirects > MAX_REDIRECTS_HARD
            || self.maximum_compressed_bytes == 0
            || self.maximum_compressed_bytes > MAX_BODY_BYTES_HARD
            || self.maximum_decompressed_bytes == 0
            || self.maximum_decompressed_bytes > MAX_BODY_BYTES_HARD
            || self.maximum_normalized_bytes == 0
            || self.maximum_normalized_bytes > MAX_NORMALIZED_BYTES_HARD
            || self.maximum_response_header_bytes == 0
            || self.maximum_response_header_bytes > MAX_HEADER_BYTES_HARD
            || self.user_agent.trim().is_empty()
            || self.user_agent.len() > 128
            || !self.user_agent.is_ascii()
        {
            return Err(RetrievalError::new(
                RetrievalErrorCode::ConfigurationInvalid,
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RetrievalRequest<'a> {
    pub policy_id: &'a str,
    pub fact_id: &'a str,
    pub source_id: &'a str,
}

#[derive(Debug, Serialize)]
pub struct RetrievalResult {
    pub policy_id: String,
    pub fact_id: String,
    pub source_id: String,
    pub requested_canonical_url: String,
    pub final_url: String,
    pub redirect_chain: Vec<String>,
    pub selected_address_family: AddressFamily,
    pub http_status: u16,
    pub content_type: String,
    pub charset: Option<String>,
    pub content_encoding: Option<String>,
    pub normalized_text: String,
    pub original_byte_count: usize,
    pub decoded_byte_count: usize,
    pub normalized_byte_count: usize,
    pub elapsed_ms: u64,
    pub retrieved_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RetrievalDiagnostic {
    pub requested_url: Option<String>,
    pub final_attempted_url: Option<String>,
    pub redirect_chain: Vec<String>,
    pub http_status: Option<u16>,
    pub content_type: Option<String>,
    pub charset: Option<String>,
    pub content_encoding: Option<String>,
    pub retry_after: Option<String>,
    pub transferred_bytes: usize,
    pub selected_ip_family: Option<AddressFamily>,
    pub stable_error_code: Option<&'static str>,
    pub elapsed_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Stable vocabulary includes errors used by later injected transports.
pub enum RetrievalErrorCode {
    RegistryNotRetrievalReady,
    UnknownPolicy,
    UnknownFactId,
    UnknownSourceId,
    CanonicalUrlMismatch,
    SchemeForbidden,
    HostForbidden,
    PortForbidden,
    PathForbidden,
    QueryForbidden,
    CredentialsForbidden,
    IpLiteralForbidden,
    DnsResolutionFailed,
    DnsNoPublicAddress,
    DnsForbiddenAddress,
    ConnectionFailed,
    TlsFailed,
    Timeout,
    ProxyPolicyViolation,
    RedirectLimitExceeded,
    RedirectLoop,
    RedirectLocationInvalid,
    RedirectTargetForbidden,
    HttpStatusForbidden,
    RateLimited,
    ContentTypeMissing,
    ContentTypeForbidden,
    CharsetForbidden,
    CompressedSizeExceeded,
    DecompressedSizeExceeded,
    NormalizedSizeExceeded,
    HtmlParseFailed,
    ConfigurationInvalid,
    RetrieverDisabled,
}

impl RetrievalErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RegistryNotRetrievalReady => "registry_not_retrieval_ready",
            Self::UnknownPolicy => "unknown_policy",
            Self::UnknownFactId => "unknown_fact_id",
            Self::UnknownSourceId => "unknown_source_id",
            Self::CanonicalUrlMismatch => "canonical_url_mismatch",
            Self::SchemeForbidden => "scheme_forbidden",
            Self::HostForbidden => "host_forbidden",
            Self::PortForbidden => "port_forbidden",
            Self::PathForbidden => "path_forbidden",
            Self::QueryForbidden => "query_forbidden",
            Self::CredentialsForbidden => "credentials_forbidden",
            Self::IpLiteralForbidden => "ip_literal_forbidden",
            Self::DnsResolutionFailed => "dns_resolution_failed",
            Self::DnsNoPublicAddress => "dns_no_public_address",
            Self::DnsForbiddenAddress => "dns_forbidden_address",
            Self::ConnectionFailed => "connection_failed",
            Self::TlsFailed => "tls_failed",
            Self::Timeout => "timeout",
            Self::ProxyPolicyViolation => "proxy_policy_violation",
            Self::RedirectLimitExceeded => "redirect_limit_exceeded",
            Self::RedirectLoop => "redirect_loop",
            Self::RedirectLocationInvalid => "redirect_location_invalid",
            Self::RedirectTargetForbidden => "redirect_target_forbidden",
            Self::HttpStatusForbidden => "http_status_forbidden",
            Self::RateLimited => "rate_limited",
            Self::ContentTypeMissing => "content_type_missing",
            Self::ContentTypeForbidden => "content_type_forbidden",
            Self::CharsetForbidden => "charset_forbidden",
            Self::CompressedSizeExceeded => "compressed_size_exceeded",
            Self::DecompressedSizeExceeded => "decompressed_size_exceeded",
            Self::NormalizedSizeExceeded => "normalized_size_exceeded",
            Self::HtmlParseFailed => "html_parse_failed",
            Self::ConfigurationInvalid => "retriever_configuration_invalid",
            Self::RetrieverDisabled => "retriever_disabled",
        }
    }
}

#[derive(Debug)]
pub struct RetrievalError {
    code: RetrievalErrorCode,
    diagnostic: Box<RetrievalDiagnostic>,
}
impl RetrievalError {
    pub fn new(code: RetrievalErrorCode) -> Self {
        Self {
            code,
            diagnostic: Box::new(RetrievalDiagnostic::default()),
        }
    }
    pub fn code(&self) -> &'static str {
        self.code.as_str()
    }
    pub fn diagnostic(&self) -> &RetrievalDiagnostic {
        &self.diagnostic
    }
    fn with_diagnostic(mut self, mut diagnostic: RetrievalDiagnostic) -> Self {
        if diagnostic.selected_ip_family.is_none() {
            diagnostic.selected_ip_family = self.diagnostic.selected_ip_family;
        }
        diagnostic.stable_error_code = Some(self.code());
        self.diagnostic = Box::new(diagnostic);
        self
    }
    fn with_selected_family(mut self, family: AddressFamily) -> Self {
        self.diagnostic.selected_ip_family = Some(family);
        self
    }
}
impl fmt::Display for RetrievalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "official retrieval failed ({})", self.code())
    }
}
impl std::error::Error for RetrievalError {}

pub fn retrieve(
    registry: &OfficialSourceRegistry,
    request: RetrievalRequest<'_>,
    config: &RetrieverConfig,
) -> Result<RetrievalResult, RetrievalError> {
    retrieve_with(registry, request, config, |url, remaining| {
        execute_pinned_get(url, config, remaining)
    })
}

fn retrieve_with<F>(
    registry: &OfficialSourceRegistry,
    request: RetrievalRequest<'_>,
    config: &RetrieverConfig,
    mut execute: F,
) -> Result<RetrievalResult, RetrievalError>
where
    F: FnMut(&Url, Duration) -> Result<(HttpResponse, IpAddr), RetrievalError>,
{
    let started = Instant::now();
    let mut diagnostic = RetrievalDiagnostic::default();
    let outcome = (|| {
        config.validate()?;
        if !config.enabled {
            return Err(RetrievalError::new(RetrievalErrorCode::RetrieverDisabled));
        }
        registry
            .require_retrieval_ready()
            .map_err(|_| RetrievalError::new(RetrievalErrorCode::RegistryNotRetrievalReady))?;
        if registry.policy_id != request.policy_id {
            return Err(RetrievalError::new(RetrievalErrorCode::UnknownPolicy));
        }
        let entry =
            registry
                .entry(request.fact_id, request.source_id)
                .map_err(|error| match error.code() {
                    Some("unknown_fact_id") => {
                        RetrievalError::new(RetrievalErrorCode::UnknownFactId)
                    }
                    _ => RetrievalError::new(RetrievalErrorCode::UnknownSourceId),
                })?;
        let canonical = entry
            .source_url()
            .map_err(|_| RetrievalError::new(RetrievalErrorCode::CanonicalUrlMismatch))?;
        let mut current = validate_registry_url(entry, canonical)?;
        diagnostic.requested_url = Some(canonical.to_owned());
        diagnostic.final_attempted_url = Some(current.to_string());
        let mut seen = HashSet::new();
        let mut redirects = Vec::new();
        loop {
            if started.elapsed() >= Duration::from_secs(config.total_timeout_seconds) {
                return Err(RetrievalError::new(RetrievalErrorCode::Timeout));
            }
            if !seen.insert(current.as_str().to_owned()) {
                return Err(RetrievalError::new(RetrievalErrorCode::RedirectLoop));
            }
            let remaining =
                Duration::from_secs(config.total_timeout_seconds).saturating_sub(started.elapsed());
            diagnostic.final_attempted_url = Some(current.to_string());
            let (response, selected) = execute(&current, remaining)?;
            diagnostic.selected_ip_family = Some(address_family(selected));
            diagnostic.http_status = Some(response.status);
            diagnostic.content_type = response.content_type.as_deref().map(normalize_header_value);
            diagnostic.charset = response.content_type.as_deref().and_then(extract_charset);
            diagnostic.content_encoding = response
                .content_encoding
                .as_deref()
                .map(normalize_header_value);
            diagnostic.retry_after = response.retry_after.clone();
            diagnostic.transferred_bytes = response.body.len();
            if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
                if redirects.len() >= usize::from(config.maximum_redirects) {
                    return Err(RetrievalError::new(
                        RetrievalErrorCode::RedirectLimitExceeded,
                    ));
                }
                let location = response.location.ok_or_else(|| {
                    RetrievalError::new(RetrievalErrorCode::RedirectLocationInvalid)
                })?;
                let destination = current.join(&location).map_err(|_| {
                    RetrievalError::new(RetrievalErrorCode::RedirectLocationInvalid)
                })?;
                validate_redirect(entry, &current, &destination)?;
                redirects.push(destination.as_str().to_owned());
                diagnostic.redirect_chain = redirects.clone();
                current = destination;
                continue;
            }
            if response.status == 429 {
                return Err(RetrievalError::new(RetrievalErrorCode::RateLimited));
            }
            if response.status != 200 {
                return Err(RetrievalError::new(RetrievalErrorCode::HttpStatusForbidden));
            }
            let kind = validate_content_type(response.content_type.as_deref())?;
            if response
                .content_encoding
                .as_deref()
                .is_some_and(|v| !v.eq_ignore_ascii_case("identity"))
            {
                return Err(RetrievalError::new(
                    RetrievalErrorCode::CompressedSizeExceeded,
                ));
            }
            if response.body.len() > config.maximum_compressed_bytes {
                return Err(RetrievalError::new(
                    RetrievalErrorCode::CompressedSizeExceeded,
                ));
            }
            if response.body.len() > config.maximum_decompressed_bytes {
                return Err(RetrievalError::new(
                    RetrievalErrorCode::DecompressedSizeExceeded,
                ));
            }
            let normalized = match kind {
                ContentKind::Html => {
                    normalize_html(&response.body, config.maximum_normalized_bytes)?
                }
                ContentKind::Plain => {
                    normalize_plain_text(&response.body, config.maximum_normalized_bytes)?
                }
            };
            return Ok(RetrievalResult {
                policy_id: request.policy_id.to_owned(),
                fact_id: request.fact_id.to_owned(),
                source_id: request.source_id.to_owned(),
                requested_canonical_url: canonical.to_owned(),
                final_url: current.to_string(),
                redirect_chain: redirects,
                selected_address_family: if selected.is_ipv4() {
                    AddressFamily::Ipv4
                } else {
                    AddressFamily::Ipv6
                },
                http_status: response.status,
                content_type: kind.label().to_owned(),
                charset: diagnostic.charset.clone(),
                content_encoding: diagnostic.content_encoding.clone(),
                original_byte_count: response.body.len(),
                decoded_byte_count: response.body.len(),
                normalized_byte_count: normalized.len(),
                elapsed_ms: elapsed_milliseconds(started.elapsed()),
                normalized_text: normalized,
                retrieved_at: Utc::now(),
            });
        }
    })();
    diagnostic.elapsed_ms = elapsed_milliseconds(started.elapsed());
    outcome.map_err(|error| error.with_diagnostic(diagnostic))
}

fn address_family(address: IpAddr) -> AddressFamily {
    if address.is_ipv4() {
        AddressFamily::Ipv4
    } else {
        AddressFamily::Ipv6
    }
}

fn elapsed_milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn normalize_header_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(256)
        .collect()
}

fn extract_charset(value: &str) -> Option<String> {
    value.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        name.trim().eq_ignore_ascii_case("charset").then(|| {
            value
                .trim()
                .trim_matches('"')
                .to_ascii_lowercase()
                .chars()
                .take(40)
                .collect()
        })
    })
}

fn validate_redirect(entry: &SourceEntry, from: &Url, to: &Url) -> Result<(), RetrievalError> {
    validate_registry_url(entry, to.as_str())
        .map_err(|_| RetrievalError::new(RetrievalErrorCode::RedirectTargetForbidden))?;
    match entry.redirect_policy {
        RedirectPolicy::SameApprovedHostOnly if from.host_str() != to.host_str() => Err(
            RetrievalError::new(RetrievalErrorCode::RedirectTargetForbidden),
        ),
        _ => Ok(()),
    }
}

enum ContentKind {
    Html,
    Plain,
}
impl ContentKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",
            Self::Plain => "text/plain; charset=utf-8",
        }
    }
}
fn validate_content_type(value: Option<&str>) -> Result<ContentKind, RetrievalError> {
    let value = value.ok_or_else(|| RetrievalError::new(RetrievalErrorCode::ContentTypeMissing))?;
    let mut parts = value.split(';');
    let mime = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    for parameter in parts {
        let parameter = parameter.trim();
        if !parameter.eq_ignore_ascii_case("charset=utf-8")
            && !parameter.eq_ignore_ascii_case("charset=\"utf-8\"")
        {
            return Err(RetrievalError::new(RetrievalErrorCode::CharsetForbidden));
        }
    }
    match mime.as_str() {
        "text/html" => Ok(ContentKind::Html),
        "text/plain" => Ok(ContentKind::Plain),
        _ => Err(RetrievalError::new(
            RetrievalErrorCode::ContentTypeForbidden,
        )),
    }
}

#[derive(Clone)]
struct HttpResponse {
    status: u16,
    content_type: Option<String>,
    content_encoding: Option<String>,
    retry_after: Option<String>,
    location: Option<String>,
    body: Vec<u8>,
}

fn select_public_address(addresses: &[SocketAddr]) -> Result<SocketAddr, RetrievalErrorCode> {
    if addresses.is_empty() {
        return Err(RetrievalErrorCode::DnsNoPublicAddress);
    }
    if addresses
        .iter()
        .any(|address| !address_is_public(address.ip()))
    {
        return Err(RetrievalErrorCode::DnsForbiddenAddress);
    }
    let mut sorted = addresses.to_vec();
    sorted.sort_by_key(|address| address.to_string());
    sorted
        .first()
        .copied()
        .ok_or(RetrievalErrorCode::DnsNoPublicAddress)
}

fn execute_pinned_get(
    url: &Url,
    config: &RetrieverConfig,
    remaining: Duration,
) -> Result<(HttpResponse, IpAddr), RetrievalError> {
    let selected = resolve_and_select(
        url,
        Duration::from_secs(config.dns_timeout_seconds).min(remaining),
    )?;
    let resolver = PinnedResolver { address: selected };
    let client_config = client_configuration(config, remaining);
    let agent = ureq::Agent::with_parts(client_config, DefaultConnector::default(), resolver);
    let mut response = agent
        .get(url.as_str())
        .header("User-Agent", &config.user_agent)
        .header("Accept", "text/html, text/plain;q=0.9")
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|error| {
            map_ureq_error(error).with_selected_family(address_family(selected.ip()))
        })?;
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned)
    };
    let content_type = header("Content-Type");
    let content_encoding = header("Content-Encoding");
    let location = header("Location");
    let retry_after = header("Retry-After")
        .as_deref()
        .and_then(validate_retry_after);
    let limit = config.maximum_compressed_bytes;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take((limit + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| RetrievalError::new(RetrievalErrorCode::ConnectionFailed))?;
    Ok((
        HttpResponse {
            status,
            content_type,
            content_encoding,
            retry_after,
            location,
            body,
        },
        selected.ip(),
    ))
}

fn client_configuration(config: &RetrieverConfig, remaining: Duration) -> ureq::config::Config {
    let request_timeout = Duration::from_secs(config.request_timeout_seconds).min(remaining);
    ureq::config::Config::builder()
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .max_response_header_size(config.maximum_response_header_bytes)
        .timeout_global(Some(request_timeout))
        .timeout_resolve(Some(
            Duration::from_secs(config.dns_timeout_seconds).min(request_timeout),
        ))
        .timeout_connect(Some(
            Duration::from_secs(config.connect_timeout_seconds).min(request_timeout),
        ))
        .timeout_recv_response(Some(request_timeout))
        .timeout_recv_body(Some(request_timeout))
        .build()
}

#[derive(Debug)]
struct PinnedResolver {
    address: SocketAddr,
}
impl Resolver for PinnedResolver {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: ureq::unversioned::transport::NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let mut result = self.empty();
        result.push(self.address);
        Ok(result)
    }
}

fn resolve_and_select(url: &Url, timeout: Duration) -> Result<SocketAddr, RetrievalError> {
    let host = url
        .host_str()
        .ok_or_else(|| RetrievalError::new(RetrievalErrorCode::HostForbidden))?
        .to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = (host.as_str(), 443)
            .to_socket_addrs()
            .map(|addresses| addresses.collect::<Vec<_>>());
        let _ = sender.send(result);
    });
    let addresses = receiver
        .recv_timeout(timeout)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => RetrievalError::new(RetrievalErrorCode::Timeout),
            mpsc::RecvTimeoutError::Disconnected => {
                RetrievalError::new(RetrievalErrorCode::DnsResolutionFailed)
            }
        })?
        .map_err(|_| RetrievalError::new(RetrievalErrorCode::DnsResolutionFailed))?;
    select_public_address(&addresses).map_err(RetrievalError::new)
}

fn map_ureq_error(error: ureq::Error) -> RetrievalError {
    match error {
        ureq::Error::Timeout(_) => RetrievalError::new(RetrievalErrorCode::Timeout),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => {
            RetrievalError::new(RetrievalErrorCode::TlsFailed)
        }
        _ => RetrievalError::new(RetrievalErrorCode::ConnectionFailed),
    }
}

fn validate_retry_after(value: &str) -> Option<String> {
    let normalized = normalize_header_value(value);
    if normalized.is_empty() {
        return None;
    }
    if normalized.bytes().all(|byte| byte.is_ascii_digit()) {
        return normalized
            .parse::<u64>()
            .ok()
            .map(|seconds| seconds.to_string());
    }
    chrono::DateTime::parse_from_rfc2822(&normalized)
        .ok()
        .filter(|date| normalized.ends_with(" GMT") && date.offset().local_minus_utc() == 0)
        .map(|_| normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> OfficialSourceRegistry {
        OfficialSourceRegistry::parse_and_validate(include_str!(
            "../official-sources/official-devops-tools-v1.json"
        ))
        .unwrap()
    }
    fn request<'a>() -> RetrievalRequest<'a> {
        RetrievalRequest {
            policy_id: "official-devops-tools-v1",
            fact_id: "docker",
            source_id: "docker-official",
        }
    }
    fn terraform_request<'a>() -> RetrievalRequest<'a> {
        RetrievalRequest {
            policy_id: "official-devops-tools-v1",
            fact_id: "terraform",
            source_id: "terraform-official",
        }
    }
    fn config() -> RetrieverConfig {
        RetrieverConfig {
            enabled: true,
            ..RetrieverConfig::default()
        }
    }
    fn response(status: u16, content_type: Option<&str>, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            content_type: content_type.map(str::to_owned),
            content_encoding: None,
            retry_after: None,
            location: None,
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn registry_binding_fails_closed() {
        let registry = registry();
        let config = config();
        for (request, code) in [
            (
                RetrievalRequest {
                    policy_id: "wrong",
                    ..request()
                },
                "unknown_policy",
            ),
            (
                RetrievalRequest {
                    fact_id: "wrong",
                    ..request()
                },
                "unknown_fact_id",
            ),
            (
                RetrievalRequest {
                    source_id: "wrong",
                    ..request()
                },
                "unknown_source_id",
            ),
        ] {
            assert_eq!(
                retrieve_with(&registry, request, &config, |_, _| unreachable!())
                    .unwrap_err()
                    .code(),
                code
            );
        }
    }

    #[test]
    fn non_ready_registry_and_disabled_configuration_fail_before_transport() {
        let pending = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../tests/fixtures/e0/registry-pending-verification.json"
        ))
        .unwrap();
        assert_eq!(
            retrieve_with(&pending, request(), &config(), |_, _| unreachable!())
                .unwrap_err()
                .code(),
            "registry_not_retrieval_ready"
        );
        assert_eq!(
            retrieve_with(
                &registry(),
                request(),
                &RetrieverConfig::default(),
                |_, _| unreachable!()
            )
            .unwrap_err()
            .code(),
            "retriever_disabled"
        );
    }
    #[test]
    fn successful_html_and_plain_text_are_bounded() {
        let registry = registry();
        let config = config();
        let html = retrieve_with(&registry, request(), &config, |_, _| {
            Ok((
                response(
                    200,
                    Some("text/html; charset=utf-8"),
                    "<h1>Docker &amp; Docs</h1><script>bad()</script>",
                ),
                "8.8.8.8".parse().unwrap(),
            ))
        })
        .unwrap();
        assert_eq!(html.normalized_text, "Docker & Docs");
        assert!(!html.normalized_text.contains('<'));
        let plain = retrieve_with(&registry, request(), &config, |_, _| {
            Ok((
                response(200, Some("text/plain"), " Docker   Docs "),
                "8.8.8.8".parse().unwrap(),
            ))
        })
        .unwrap();
        assert_eq!(plain.normalized_text, "Docker Docs");
    }
    #[test]
    fn response_policy_rejects_status_type_charset_and_sizes() {
        let registry = registry();
        for (candidate, code) in [
            (
                response(206, Some("text/plain"), "x"),
                "http_status_forbidden",
            ),
            (
                response(404, Some("text/plain"), "x"),
                "http_status_forbidden",
            ),
            (
                response(500, Some("text/plain"), "x"),
                "http_status_forbidden",
            ),
            (response(200, None, "x"), "content_type_missing"),
            (
                response(200, Some("application/json"), "{}"),
                "content_type_forbidden",
            ),
            (
                response(200, Some("text/html; charset=iso-8859-1"), "x"),
                "charset_forbidden",
            ),
        ] {
            assert_eq!(
                retrieve_with(&registry, request(), &config(), |_, _| Ok((
                    candidate.clone(),
                    "8.8.8.8".parse().unwrap()
                )))
                .unwrap_err()
                .code(),
                code
            );
        }
        let mut tiny = config();
        tiny.maximum_compressed_bytes = 4;
        tiny.maximum_decompressed_bytes = 4;
        assert_eq!(
            retrieve_with(&registry, request(), &tiny, |_, _| Ok((
                response(200, Some("text/plain"), "12345"),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "compressed_size_exceeded"
        );
        let mut normalized = config();
        normalized.maximum_normalized_bytes = 4;
        assert_eq!(
            retrieve_with(&registry, request(), &normalized, |_, _| Ok((
                response(200, Some("text/plain"), "12345"),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "normalized_size_exceeded"
        );
        let mut decoded = config();
        decoded.maximum_compressed_bytes = 8;
        decoded.maximum_decompressed_bytes = 4;
        assert_eq!(
            retrieve_with(&registry, request(), &decoded, |_, _| {
                Ok((
                    response(200, Some("text/plain"), "12345"),
                    "8.8.8.8".parse().unwrap(),
                ))
            })
            .unwrap_err()
            .code(),
            "decompressed_size_exceeded"
        );
        let mut encoded = response(200, Some("text/html"), "content");
        encoded.content_encoding = Some("gzip".to_owned());
        assert_eq!(
            retrieve_with(&registry, request(), &config(), |_, _| Ok((
                encoded.clone(),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "compressed_size_exceeded"
        );
    }
    #[test]
    fn redirects_are_manual_bounded_and_policy_checked() {
        let registry = registry();
        let mut calls = 0;
        let result = retrieve_with(&registry, terraform_request(), &config(), |_, _| {
            calls += 1;
            if calls == 1 {
                let mut r = response(302, None, "");
                r.location = Some("/terraform/language".to_owned());
                Ok((r, "8.8.8.8".parse().unwrap()))
            } else {
                Ok((
                    response(200, Some("text/plain"), "docs"),
                    "8.8.8.8".parse().unwrap(),
                ))
            }
        })
        .unwrap();
        assert_eq!(
            result.redirect_chain,
            vec!["https://developer.hashicorp.com/terraform/language"]
        );
        for location in [
            "http://docs.docker.com/",
            "https://evil.example/",
            "https://docs.docker.com.evil.example/",
            "https://docs.docker.com/",
        ] {
            let mut r = response(302, None, "");
            r.location = Some(location.to_owned());
            assert_eq!(
                retrieve_with(&registry, request(), &config(), |_, _| Ok((
                    r.clone(),
                    "8.8.8.8".parse().unwrap()
                )))
                .unwrap_err()
                .code(),
                "redirect_target_forbidden"
            );
        }
    }
    #[test]
    fn redirect_loop_and_limit_are_rejected() {
        let registry = registry();
        let mut r = response(302, None, "");
        r.location = Some("/".to_owned());
        assert_eq!(
            retrieve_with(&registry, request(), &config(), |_, _| Ok((
                r.clone(),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "redirect_loop"
        );
        let mut zero = config();
        zero.maximum_redirects = 0;
        let mut r = response(302, None, "");
        r.location = Some("/manuals/".to_owned());
        assert_eq!(
            retrieve_with(&registry, request(), &zero, |_, _| Ok((
                r.clone(),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "redirect_limit_exceeded"
        );
    }

    #[test]
    fn missing_malformed_and_non_redirect_locations_fail_closed() {
        let registry = registry();
        let redirect = response(302, None, "");
        assert_eq!(
            retrieve_with(&registry, request(), &config(), |_, _| Ok((
                redirect.clone(),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "redirect_location_invalid"
        );
        let not_modified = response(304, None, "");
        assert_eq!(
            retrieve_with(&registry, request(), &config(), |_, _| Ok((
                not_modified.clone(),
                "8.8.8.8".parse().unwrap()
            )))
            .unwrap_err()
            .code(),
            "http_status_forbidden"
        );
    }
    #[test]
    fn strict_dns_policy_rejects_mixed_answers_and_pins_one() {
        let public: SocketAddr = "8.8.8.8:443".parse().unwrap();
        let private: SocketAddr = "127.0.0.1:443".parse().unwrap();
        assert_eq!(
            select_public_address(&[public, private]).unwrap_err(),
            RetrievalErrorCode::DnsForbiddenAddress
        );
        assert_eq!(select_public_address(&[public]).unwrap(), public);
    }
    #[test]
    fn client_configuration_disables_proxies_and_redirects() {
        let config = config();
        let built = client_configuration(&config, Duration::from_secs(60));
        assert!(built.proxy().is_none());
        assert_eq!(built.max_redirects(), 0);
    }

    #[test]
    fn default_body_limits_are_512_kib_with_one_mib_hard_ceiling() {
        let defaults = RetrieverConfig::default();
        assert_eq!(defaults.maximum_compressed_bytes, 512 * 1024);
        assert_eq!(defaults.maximum_decompressed_bytes, 512 * 1024);
        let mut ceiling = defaults;
        ceiling.maximum_compressed_bytes = MAX_BODY_BYTES_HARD;
        ceiling.maximum_decompressed_bytes = MAX_BODY_BYTES_HARD;
        assert!(ceiling.validate().is_ok());
        ceiling.maximum_compressed_bytes += 1;
        assert_eq!(
            ceiling.validate().unwrap_err().code(),
            "retriever_configuration_invalid"
        );
    }

    #[test]
    fn failure_diagnostic_is_sanitized_and_preserves_completed_metadata() {
        let registry = registry();
        let mut failure_response = response(404, Some(" Text/HTML ; Charset=UTF-8 "), "not found");
        failure_response.content_encoding = Some(" identity ".to_owned());
        let failure = retrieve_with(&registry, request(), &config(), |_, _| {
            Ok((failure_response.clone(), "8.8.8.8".parse().unwrap()))
        })
        .unwrap_err();
        let diagnostic = failure.diagnostic();
        assert_eq!(diagnostic.stable_error_code, Some("http_status_forbidden"));
        assert_eq!(diagnostic.http_status, Some(404));
        assert_eq!(
            diagnostic.content_type.as_deref(),
            Some("Text/HTML ; Charset=UTF-8")
        );
        assert_eq!(diagnostic.charset.as_deref(), Some("utf-8"));
        assert_eq!(diagnostic.content_encoding.as_deref(), Some("identity"));
        assert_eq!(diagnostic.transferred_bytes, 9);
        assert_eq!(diagnostic.selected_ip_family, Some(AddressFamily::Ipv4));
        let json = serde_json::to_string(diagnostic).unwrap();
        for forbidden in ["headers", "cookie", "authorization", "proxy", "8.8.8.8"] {
            assert!(!json.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn size_failure_diagnostic_records_bounded_bytes_without_body_content() {
        let registry = registry();
        let mut small = config();
        small.maximum_compressed_bytes = 4;
        let failure = retrieve_with(&registry, request(), &small, |_, _| {
            Ok((
                response(200, Some("text/html"), "secret-body"),
                "8.8.8.8".parse().unwrap(),
            ))
        })
        .unwrap_err();
        assert_eq!(failure.code(), "compressed_size_exceeded");
        assert_eq!(failure.diagnostic().transferred_bytes, 11);
        assert!(
            !serde_json::to_string(failure.diagnostic())
                .unwrap()
                .contains("secret-body")
        );
    }

    #[test]
    fn rate_limit_diagnostics_allow_only_valid_retry_after_values() {
        let registry = registry();
        for (header, expected) in [
            (None, None),
            (Some("120"), Some("120")),
            (
                Some("Wed, 21 Oct 2015 07:28:00 GMT"),
                Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            ),
            (Some("tomorrow; authorization=secret"), None),
        ] {
            let mut limited = response(429, Some("text/html; charset=UTF-8"), "limited");
            limited.retry_after = header.and_then(validate_retry_after);
            let failure = retrieve_with(&registry, request(), &config(), |_, _| {
                Ok((limited.clone(), "8.8.8.8".parse().unwrap()))
            })
            .unwrap_err();
            assert_eq!(failure.code(), "rate_limited");
            assert_eq!(failure.diagnostic().http_status, Some(429));
            assert_eq!(failure.diagnostic().retry_after.as_deref(), expected);
            let serialized = serde_json::to_string(failure.diagnostic()).unwrap();
            assert!(!serialized.contains("authorization"));
            assert!(!serialized.contains("secret"));
        }
    }

    #[test]
    fn pinned_resolver_always_returns_only_the_validated_address() {
        let expected: SocketAddr = "8.8.8.8:443".parse().unwrap();
        let resolver = PinnedResolver { address: expected };
        let uri = "https://docs.docker.com/".parse().unwrap();
        let config = client_configuration(&config(), Duration::from_secs(60));
        let resolved = resolver
            .resolve(
                &uri,
                &config,
                ureq::unversioned::transport::NextTimeout {
                    after: ureq::unversioned::transport::time::Duration::from_secs(1),
                    reason: ureq::Timeout::Global,
                },
            )
            .unwrap();
        assert_eq!(&resolved[..], &[expected]);
    }
}
