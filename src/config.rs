use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use url::{Host, Url};

use crate::error::{AppError, ErrorKind};

const MAX_MODEL_CHARS: usize = 200;
const MAX_ENDPOINT_CHARS: usize = 2_048;
const MAX_REPORT_PATH_CHARS: usize = 4_096;
const MAX_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    lead_model: String,
    developer_model: String,
    ollama_endpoint: String,
    response_timeout_seconds: u64,
    report_directory: String,
    workspace_directory: String,
}

#[derive(Debug)]
pub struct Config {
    pub lead_model: String,
    pub developer_model: String,
    pub chat_url: Url,
    pub response_timeout_seconds: u64,
    pub report_directory: PathBuf,
    pub workspace_directory: PathBuf,
}

impl Config {
    pub fn load(repository_root: &Path) -> Result<Self, AppError> {
        let path = repository_root.join("agent-factory.toml");
        let contents = fs::read_to_string(&path).map_err(|error| {
            AppError::new(
                ErrorKind::Configuration,
                format!("could not read {}: {error}", path.display()),
            )
        })?;
        let file: FileConfig = toml::from_str(&contents).map_err(|error| {
            AppError::new(
                ErrorKind::Configuration,
                format!("invalid configuration: {error}"),
            )
        })?;
        Self::validate(file, repository_root)
    }

    fn validate(file: FileConfig, repository_root: &Path) -> Result<Self, AppError> {
        validate_non_empty("lead_model", &file.lead_model, MAX_MODEL_CHARS)?;
        validate_non_empty("developer_model", &file.developer_model, MAX_MODEL_CHARS)?;
        validate_non_empty("ollama_endpoint", &file.ollama_endpoint, MAX_ENDPOINT_CHARS)?;
        validate_non_empty(
            "report_directory",
            &file.report_directory,
            MAX_REPORT_PATH_CHARS,
        )?;
        validate_non_empty(
            "workspace_directory",
            &file.workspace_directory,
            MAX_REPORT_PATH_CHARS,
        )?;
        if file.workspace_directory != "workspaces" {
            return Err(AppError::new(
                ErrorKind::Configuration,
                "workspace_directory must be the repository-relative path workspaces",
            ));
        }

        if !(1..=MAX_TIMEOUT_SECONDS).contains(&file.response_timeout_seconds) {
            return Err(AppError::new(
                ErrorKind::Configuration,
                "response_timeout_seconds must be between 1 and 600",
            ));
        }

        let chat_url = validate_endpoint(&file.ollama_endpoint)?;
        let report_directory = validate_report_directory(repository_root, &file.report_directory)?;
        let workspace_directory =
            validate_workspace_directory(repository_root, &file.workspace_directory)?;

        Ok(Self {
            lead_model: file.lead_model,
            developer_model: file.developer_model,
            chat_url,
            response_timeout_seconds: file.response_timeout_seconds,
            report_directory,
            workspace_directory,
        })
    }
}

fn validate_workspace_directory(root: &Path, configured: &str) -> Result<PathBuf, AppError> {
    let directory = validate_report_directory(root, configured)?;
    let metadata = fs::symlink_metadata(root.join(configured)).map_err(|error| {
        AppError::new(
            ErrorKind::Configuration,
            format!("workspace directory is unavailable: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "workspace directory must not be a symbolic link",
        ));
    }
    Ok(directory)
}

fn validate_non_empty(name: &str, value: &str, maximum: usize) -> Result<(), AppError> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > maximum {
        return Err(AppError::new(
            ErrorKind::Configuration,
            format!("{name} must contain between 1 and {maximum} characters"),
        ));
    }
    Ok(())
}

fn validate_endpoint(value: &str) -> Result<Url, AppError> {
    if !has_approved_explicit_authority(value) {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "ollama_endpoint must use the explicit host localhost, 127.0.0.1, or [::1]",
        ));
    }

    let mut url = Url::parse(value).map_err(|_| {
        AppError::new(
            ErrorKind::Configuration,
            "ollama_endpoint must be a valid URL",
        )
    })?;

    if url.scheme() != "http" {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "ollama_endpoint must use plain HTTP",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "ollama_endpoint must not contain credentials",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "ollama_endpoint must not contain a query string or fragment",
        ));
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "ollama_endpoint must not contain a path",
        ));
    }

    match url.host() {
        Some(Host::Domain("localhost")) => {}
        Some(Host::Ipv4(address)) if address == std::net::Ipv4Addr::LOCALHOST => {}
        Some(Host::Ipv6(address)) if address == std::net::Ipv6Addr::LOCALHOST => {}
        _ => {
            return Err(AppError::new(
                ErrorKind::Configuration,
                "ollama_endpoint host must be localhost, 127.0.0.1, or ::1",
            ));
        }
    }

    if url.port().is_none() {
        url.set_port(Some(11_434)).map_err(|_| {
            AppError::new(
                ErrorKind::Configuration,
                "could not set the default Ollama port",
            )
        })?;
    }
    url.set_path("/api/chat");
    Ok(url)
}

fn has_approved_explicit_authority(value: &str) -> bool {
    let Some(authority) = value.strip_prefix("http://") else {
        return false;
    };
    let authority = authority.strip_suffix('/').unwrap_or(authority);

    ["localhost", "127.0.0.1", "[::1]"].iter().any(|host| {
        authority == *host
            || authority
                .strip_prefix(&format!("{host}:"))
                .is_some_and(|port| !port.is_empty() && port.parse::<u16>().is_ok())
    })
}

fn validate_report_directory(root: &Path, configured: &str) -> Result<PathBuf, AppError> {
    let canonical_root = root.canonicalize().map_err(|error| {
        AppError::new(
            ErrorKind::Configuration,
            format!("could not resolve repository root: {error}"),
        )
    })?;
    let configured_path = Path::new(configured);
    let candidate = if configured_path.is_absolute() {
        let relative = configured_path.strip_prefix(&canonical_root).map_err(|_| {
            AppError::new(
                ErrorKind::Configuration,
                "report directory must stay inside the repository",
            )
        })?;
        reject_parent_components(relative)?;
        configured_path.to_path_buf()
    } else {
        reject_parent_components(configured_path)?;
        canonical_root.join(configured_path)
    };
    let canonical_directory = candidate.canonicalize().map_err(|error| {
        AppError::new(
            ErrorKind::Configuration,
            format!("report directory is unavailable: {error}"),
        )
    })?;

    if !canonical_directory.starts_with(&canonical_root) || !canonical_directory.is_dir() {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "report directory must be an existing directory inside the repository",
        ));
    }
    Ok(canonical_directory)
}

fn reject_parent_components(path: &Path) -> Result<(), AppError> {
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "report directory must stay inside the repository",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn valid_file() -> FileConfig {
        FileConfig {
            lead_model: "gemma3:latest".to_owned(),
            developer_model: "qwen2.5-coder:7b".to_owned(),
            ollama_endpoint: "http://localhost:11434".to_owned(),
            response_timeout_seconds: 300,
            report_directory: "reports".to_owned(),
            workspace_directory: "workspaces".to_owned(),
        }
    }

    fn repository() -> TempDir {
        let directory = TempDir::new().expect("temporary directory");
        fs::create_dir(directory.path().join("reports")).expect("reports directory");
        fs::create_dir(directory.path().join("workspaces")).expect("workspaces directory");
        directory
    }

    #[test]
    fn accepts_each_explicit_loopback_host() {
        for endpoint in [
            "http://localhost:11434",
            "http://127.0.0.1:11434",
            "http://[::1]:11434",
        ] {
            let url = validate_endpoint(endpoint).expect("approved endpoint");
            assert_eq!(url.path(), "/api/chat");
        }
    }

    #[test]
    fn rejects_remote_and_ambiguous_endpoints() {
        for endpoint in [
            "https://localhost:11434",
            "http://example.com:11434",
            "http://127.0.0.2:11434",
            "http://127.1:11434",
            "http://LOCALHOST:11434",
            "http://user@localhost:11434",
            "http://localhost:11434/path",
            "http://localhost:11434?query=yes",
            "http://localhost:11434#fragment",
        ] {
            assert!(validate_endpoint(endpoint).is_err(), "accepted {endpoint}");
        }
    }

    #[test]
    fn rejects_timeout_above_the_approved_maximum() {
        let repository = repository();
        let mut file = valid_file();
        file.response_timeout_seconds = 601;
        assert!(Config::validate(file, repository.path()).is_err());
    }

    #[test]
    fn rejects_any_workspace_directory_other_than_the_approved_root() {
        let repository = repository();
        let mut file = valid_file();
        file.workspace_directory = "other-workspaces".to_owned();
        let error = Config::validate(file, repository.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "workspace_directory must be the repository-relative path workspaces"
        );
    }

    #[test]
    fn rejects_report_directory_outside_repository() {
        let repository = repository();
        let outside = TempDir::new().expect("outside directory");
        let mut file = valid_file();
        file.report_directory = outside.path().display().to_string();
        let error = Config::validate(file, repository.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "report directory must stay inside the repository"
        );
    }

    #[test]
    fn rejects_nonexistent_outside_path_before_canonicalization() {
        let repository = repository();
        let mut file = valid_file();
        file.report_directory = repository
            .path()
            .parent()
            .unwrap()
            .join("definitely-does-not-exist")
            .display()
            .to_string();
        let error = Config::validate(file, repository.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "report directory must stay inside the repository"
        );
    }

    #[test]
    fn rejects_relative_parent_traversal_before_canonicalization() {
        let repository = repository();
        let mut file = valid_file();
        file.report_directory = "../outside".to_owned();
        let error = Config::validate(file, repository.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "report directory must stay inside the repository"
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonical_check_rejects_symlink_to_outside_directory() {
        use std::os::unix::fs::symlink;

        let repository = repository();
        let outside = TempDir::new().expect("outside directory");
        symlink(outside.path(), repository.path().join("linked-reports")).unwrap();
        let mut file = valid_file();
        file.report_directory = "linked-reports".to_owned();
        let error = Config::validate(file, repository.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "report directory must be an existing directory inside the repository"
        );
    }
}
