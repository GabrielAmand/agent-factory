use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::{Host, Url};

use crate::retriever::{RetrievalError, RetrievalErrorCode};
use crate::source_registry::SourceEntry;

pub fn validate_registry_url(entry: &SourceEntry, raw: &str) -> Result<Url, RetrievalError> {
    if raw.bytes().any(|byte| byte.is_ascii_control()) || raw.contains('\\') || has_bad_percent(raw)
    {
        return Err(RetrievalError::new(
            RetrievalErrorCode::CanonicalUrlMismatch,
        ));
    }
    let url = Url::parse(raw)
        .map_err(|_| RetrievalError::new(RetrievalErrorCode::CanonicalUrlMismatch))?;
    if raw.contains('%') {
        return Err(RetrievalError::new(RetrievalErrorCode::PathForbidden));
    }
    if url.scheme() != "https" {
        return Err(RetrievalError::new(RetrievalErrorCode::SchemeForbidden));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(RetrievalError::new(
            RetrievalErrorCode::CredentialsForbidden,
        ));
    }
    if url.port().is_some() {
        return Err(RetrievalError::new(RetrievalErrorCode::PortForbidden));
    }
    if url.fragment().is_some() {
        return Err(RetrievalError::new(
            RetrievalErrorCode::CanonicalUrlMismatch,
        ));
    }
    if url.query().is_some() {
        return Err(RetrievalError::new(RetrievalErrorCode::QueryForbidden));
    }
    match url.host() {
        Some(Host::Domain(host))
            if host.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
            }) => {}
        Some(Host::Ipv4(_)) | Some(Host::Ipv6(_)) => {
            return Err(RetrievalError::new(RetrievalErrorCode::IpLiteralForbidden));
        }
        _ => return Err(RetrievalError::new(RetrievalErrorCode::HostForbidden)),
    }
    if !entry.allows_host(url.host_str().unwrap_or_default()) {
        return Err(RetrievalError::new(RetrievalErrorCode::HostForbidden));
    }
    if url.path().contains('%') {
        return Err(RetrievalError::new(RetrievalErrorCode::PathForbidden));
    }
    if !entry.allows_url(&url) {
        return Err(RetrievalError::new(RetrievalErrorCode::PathForbidden));
    }
    Ok(url)
}

pub fn address_is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => ipv4_is_public(v4),
        IpAddr::V6(v6) => ipv6_is_public(v6),
    }
}

fn ipv4_is_public(address: Ipv4Addr) -> bool {
    let value = u32::from(address);
    ![
        (0x0000_0000, 8),
        (0x0a00_0000, 8),
        (0x6440_0000, 10),
        (0x7f00_0000, 8),
        (0xa9fe_0000, 16),
        (0xac10_0000, 12),
        (0xc000_0000, 24),
        (0xc000_0200, 24),
        (0xc01f_c400, 24),
        (0xc034_c100, 24),
        (0xc058_6300, 24),
        (0xc0a8_0000, 16),
        (0xc0af_3000, 24),
        (0xc612_0000, 15),
        (0xc633_6400, 24),
        (0xcb00_7100, 24),
        (0xe000_0000, 4),
        (0xf000_0000, 4),
    ]
    .iter()
    .any(|(network, prefix)| value >> (32 - prefix) == network >> (32 - prefix))
}

fn ipv6_is_public(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return ipv4_is_public(mapped);
    }
    let value = u128::from(address);
    if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
        return false;
    }
    ![
        (0u128, 8),
        (
            u128::from_be_bytes([0x00, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            96,
        ),
        (
            u128::from_be_bytes([0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            64,
        ),
        (
            u128::from_be_bytes([0x20, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            23,
        ),
        (
            u128::from_be_bytes([0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            32,
        ),
        (
            u128::from_be_bytes([0x20, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            16,
        ),
        (
            u128::from_be_bytes([0x3f, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            20,
        ),
        (
            u128::from_be_bytes([0x5f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            16,
        ),
        (
            u128::from_be_bytes([0xfc, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            7,
        ),
        (
            u128::from_be_bytes([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            10,
        ),
    ]
    .iter()
    .any(|(network, prefix)| value >> (128 - prefix) == network >> (128 - prefix))
}

fn has_bad_percent(value: &str) -> bool {
    let bytes = value.as_bytes();
    (0..bytes.len()).any(|index| {
        bytes[index] == b'%'
            && (index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_registry::OfficialSourceRegistry;
    #[test]
    fn rejects_special_addresses_and_accepts_public() {
        for value in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "203.0.113.1",
            "224.0.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!address_is_public(value.parse().unwrap()), "{value}");
        }
        assert!(address_is_public("8.8.8.8".parse().unwrap()));
        assert!(address_is_public("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn url_policy_is_exact_and_conservative() {
        let registry = OfficialSourceRegistry::parse_and_validate(include_str!(
            "../official-sources/official-devops-tools-v1.json"
        ))
        .unwrap();
        let entry = registry.entry("docker", "docker-official").unwrap();
        assert!(validate_registry_url(entry, "https://docs.docker.com/").is_ok());
        for (url, code) in [
            ("http://docs.docker.com/", "scheme_forbidden"),
            ("https://user@docs.docker.com/", "credentials_forbidden"),
            ("https://127.0.0.1/", "ip_literal_forbidden"),
            ("https://docs.docker.com:444/", "port_forbidden"),
            ("https://docs.docker.com/?x=1", "query_forbidden"),
            ("https://docs.docker.com.evil.example/", "host_forbidden"),
            ("https://docs.docker.com/%2e%2e/", "path_forbidden"),
        ] {
            assert_eq!(validate_registry_url(entry, url).unwrap_err().code(), code);
        }
    }
}
