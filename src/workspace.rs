use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::error::{AppError, ErrorKind};
use crate::protocol::{DeveloperWorkspace, GENERATED_FILE_NAMES};

#[derive(Debug)]
pub struct PublishedFile {
    pub path: String,
    pub bytes: usize,
}

#[derive(Debug)]
pub struct PublishedWorkspace {
    pub relative_path: String,
    pub files: Vec<PublishedFile>,
    pub creation_duration: Duration,
}

pub fn publish_workspace(
    root: &Path,
    run_id: &str,
    workspace: &DeveloperWorkspace,
) -> Result<PublishedWorkspace, AppError> {
    publish_workspace_inner(root, run_id, workspace, None)
}

fn publish_workspace_inner(
    root: &Path,
    run_id: &str,
    workspace: &DeveloperWorkspace,
    fail_after_files: Option<usize>,
) -> Result<PublishedWorkspace, AppError> {
    validate_run_id(run_id)?;
    validate_workspace_root(root)?;
    let started = Instant::now();
    let final_name = format!("run-{run_id}");
    let staging_name = format!(".run-{run_id}.staging");
    let final_path = root.join(&final_name);
    let staging_path = root.join(&staging_name);

    if final_path.exists() || staging_path.exists() {
        return Err(AppError::new(
            ErrorKind::Workspace,
            "run workspace or staging directory already exists",
        ));
    }
    create_private_directory(&staging_path)?;

    let write_result =
        write_staging_files(&staging_path, workspace, fail_after_files).and_then(|files| {
            sync_directory(&staging_path)?;
            if final_path.exists() {
                return Err(AppError::new(
                    ErrorKind::Workspace,
                    "final run workspace already exists",
                ));
            }
            fs::rename(&staging_path, &final_path).map_err(|error| {
                AppError::new(
                    ErrorKind::Workspace,
                    format!("could not publish run workspace: {error}"),
                )
            })?;
            Ok(files)
        });

    match write_result {
        Ok(files) => Ok(PublishedWorkspace {
            relative_path: format!("workspaces/{final_name}"),
            files,
            creation_duration: started.elapsed(),
        }),
        Err(error) => {
            if staging_path.exists() {
                fs::remove_dir_all(&staging_path).map_err(|cleanup_error| {
                    AppError::new(
                        ErrorKind::Workspace,
                        format!(
                            "{error}; could not remove failed staging directory: {cleanup_error}"
                        ),
                    )
                })?;
            }
            Err(error)
        }
    }
}

fn write_staging_files(
    staging_path: &Path,
    workspace: &DeveloperWorkspace,
    fail_after_files: Option<usize>,
) -> Result<Vec<PublishedFile>, AppError> {
    let mut published = Vec::with_capacity(GENERATED_FILE_NAMES.len());
    for (index, name) in GENERATED_FILE_NAMES.iter().enumerate() {
        if fail_after_files == Some(index) {
            return Err(AppError::new(
                ErrorKind::Workspace,
                "injected staging write failure",
            ));
        }
        let generated = workspace.file(name).ok_or_else(|| {
            AppError::new(
                ErrorKind::Workspace,
                format!("validated workspace unexpectedly lacks {name}"),
            )
        })?;
        let path = staging_path.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|error| {
            AppError::new(
                ErrorKind::Workspace,
                format!("could not create staged {name}: {error}"),
            )
        })?;
        file.write_all(generated.content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| {
                AppError::new(
                    ErrorKind::Workspace,
                    format!("could not write staged {name}: {error}"),
                )
            })?;
        published.push(PublishedFile {
            path: (*name).to_owned(),
            bytes: generated.content.len(),
        });
    }
    Ok(published)
}

fn validate_workspace_root(root: &Path) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        AppError::new(
            ErrorKind::Workspace,
            format!("workspace root is unavailable: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::new(
            ErrorKind::Workspace,
            "workspace root must be a real directory",
        ));
    }
    Ok(())
}

pub fn validate_run_id(run_id: &str) -> Result<(), AppError> {
    if run_id.is_empty()
        || run_id.len() > 100
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(AppError::new(
            ErrorKind::Workspace,
            "run ID must use 1 to 100 ASCII letters, digits, or hyphens",
        ));
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), AppError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|error| {
        AppError::new(
            ErrorKind::Workspace,
            format!("could not create staging directory: {error}"),
        )
    })
}

fn sync_directory(path: &Path) -> Result<(), AppError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            AppError::new(
                ErrorKind::Workspace,
                format!("could not synchronize directory: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::protocol::DeveloperWorkspace;

    fn valid_workspace() -> DeveloperWorkspace {
        let json = serde_json::json!({
            "response_version": "developer-workspace-v1",
            "task_id": "task-1",
            "files": [
                {"path":"index.html","content":"<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"></head><body><main id=\"tool-list\"></main><script src=\"app.js\" defer></script></body></html>"},
                {"path":"app.js","content":"fetch(\"resources.json\").then(response => response.json());"},
                {"path":"styles.css","content":"body { color: black; }"},
                {"path":"resources.json","content":"{\"resources_version\":\"resources-v1\",\"tools\":[{\"name\":\"Tool\",\"description\":\"Description\",\"tags\":[\"automation\"]}]}"}
            ]
        });
        DeveloperWorkspace::parse_and_validate(&json.to_string(), "task-1").unwrap()
    }

    #[test]
    fn publishes_exact_files_atomically() {
        let root = TempDir::new().unwrap();
        let published = publish_workspace(root.path(), "run-1", &valid_workspace()).unwrap();
        let final_path = root.path().join("run-run-1");
        assert!(final_path.is_dir());
        assert_eq!(published.files.len(), 4);
        for name in GENERATED_FILE_NAMES {
            assert!(final_path.join(name).is_file());
        }
        assert!(!root.path().join(".run-run-1.staging").exists());
    }

    #[test]
    fn failed_staging_write_is_cleaned_without_final_workspace() {
        let root = TempDir::new().unwrap();
        let result = publish_workspace_inner(root.path(), "run-2", &valid_workspace(), Some(2));
        assert!(result.is_err());
        assert!(!root.path().join("run-run-2").exists());
        assert!(!root.path().join(".run-run-2.staging").exists());
    }

    #[test]
    fn failed_validation_does_not_create_a_workspace() {
        let root = TempDir::new().unwrap();
        let invalid = serde_json::json!({
            "response_version": "developer-workspace-v1",
            "task_id": "task-1",
            "files": []
        });
        assert!(DeveloperWorkspace::parse_and_validate(&invalid.to_string(), "task-1").is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn never_overwrites_existing_workspace() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("run-existing")).unwrap();
        assert!(publish_workspace(root.path(), "existing", &valid_workspace()).is_err());
        assert!(root.path().join("run-existing").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_workspace_root() {
        use std::os::unix::fs::symlink;
        let parent = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let link = parent.path().join("workspaces");
        symlink(target.path(), &link).unwrap();
        assert!(publish_workspace(&link, "run-3", &valid_workspace()).is_err());
    }
}
