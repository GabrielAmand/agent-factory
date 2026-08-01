use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::error::{AppError, ErrorKind};
use crate::ollama::OllamaMetrics;
use crate::protocol::{
    DEVELOPER_REQUEST_VERSION, DEVELOPER_SCHEMA_VERSION, LEAD_SCHEMA_VERSION, LeadResponse,
};
use crate::workspace::PublishedWorkspace;

pub const REPORT_VERSION: &str = "execution-report-v3";
pub const TRANSMITTED_FIELDS: [&str; 5] = [
    "selected_task_id",
    "selected_task_title",
    "selected_task_objective",
    "selected_task_acceptance_criteria",
    "lead_acceptance_criteria",
];

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Success,
    Failure,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    NotAttempted,
    Success,
    Failure,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    NotAttempted,
    Passed,
    Failed,
}

#[derive(Serialize)]
struct FailureSummary {
    stage: &'static str,
    kind: ErrorKind,
    cause: String,
}

#[derive(Serialize)]
struct InputMeasurements {
    bytes: usize,
    characters: usize,
}

#[derive(Serialize)]
struct LeadRoleReport<'a> {
    status: StageStatus,
    validation: ValidationStatus,
    model: &'a str,
    schema_version: &'static str,
    metrics: Option<&'a OllamaMetrics>,
    response: Option<&'a LeadResponse>,
}

#[derive(Serialize)]
struct DelegationReport<'a> {
    status: StageStatus,
    selected_task_id: Option<&'a str>,
    request_version: &'static str,
    request_json_bytes: Option<usize>,
    transmitted_fields: [&'static str; 5],
}

#[derive(Serialize)]
struct DeveloperRoleReport<'a> {
    status: StageStatus,
    validation: ValidationStatus,
    model: &'a str,
    schema_version: &'static str,
    metrics: Option<&'a OllamaMetrics>,
}

#[derive(Serialize)]
struct ArtifactValidationReport {
    status: ValidationStatus,
    failure_count: usize,
    failure_codes: Vec<&'static str>,
    file_count: usize,
    generated_bytes: usize,
}

#[derive(Serialize)]
struct WorkspaceFileReport<'a> {
    path: &'a str,
    bytes: usize,
}

#[derive(Serialize)]
struct WorkspaceReport<'a> {
    status: StageStatus,
    relative_path: Option<&'a str>,
    creation_duration_ms: Option<u64>,
    files: Vec<WorkspaceFileReport<'a>>,
}

#[derive(Serialize)]
struct PreviewReport {
    status: &'static str,
    human_approval: &'static str,
}

#[derive(Serialize)]
struct ExecutionReport<'a> {
    report_version: &'static str,
    run_id: &'a str,
    status: ReportStatus,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    total_duration_ms: u64,
    input: InputMeasurements,
    lead: LeadRoleReport<'a>,
    delegation: DelegationReport<'a>,
    developer: DeveloperRoleReport<'a>,
    artifact_validation: ArtifactValidationReport,
    workspace: WorkspaceReport<'a>,
    preview: PreviewReport,
    failure: Option<FailureSummary>,
}

pub struct ReportData<'a> {
    pub run_id: &'a str,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub input: &'a str,
    pub lead_model: &'a str,
    pub lead_status: StageStatus,
    pub lead_validation: ValidationStatus,
    pub lead_metrics: Option<&'a OllamaMetrics>,
    pub lead_response: Option<&'a LeadResponse>,
    pub delegation_status: StageStatus,
    pub selected_task_id: Option<&'a str>,
    pub developer_request_bytes: Option<usize>,
    pub developer_model: &'a str,
    pub developer_status: StageStatus,
    pub developer_validation: ValidationStatus,
    pub developer_metrics: Option<&'a OllamaMetrics>,
    pub generated_file_count: usize,
    pub generated_bytes: usize,
    pub workspace_status: StageStatus,
    pub published_workspace: Option<&'a PublishedWorkspace>,
    pub failure_stage: Option<&'static str>,
    pub error: Option<&'a AppError>,
}

pub fn write_report(directory: &Path, data: ReportData<'_>) -> Result<PathBuf, AppError> {
    let artifact_status = if data.generated_file_count == 4 {
        ValidationStatus::Passed
    } else if matches!(data.developer_validation, ValidationStatus::Failed) {
        ValidationStatus::Failed
    } else {
        ValidationStatus::NotAttempted
    };
    let artifact_failed = matches!(artifact_status, ValidationStatus::Failed);
    let artifact_failure_codes = if artifact_failed {
        vec![
            data.error
                .and_then(AppError::code)
                .unwrap_or("developer_workspace_invalid"),
        ]
    } else {
        vec![]
    };
    let workspace_files = data.published_workspace.map_or_else(Vec::new, |workspace| {
        workspace
            .files
            .iter()
            .map(|file| WorkspaceFileReport {
                path: &file.path,
                bytes: file.bytes,
            })
            .collect()
    });
    let report = ExecutionReport {
        report_version: REPORT_VERSION,
        run_id: data.run_id,
        status: if data.error.is_some() {
            ReportStatus::Failure
        } else {
            ReportStatus::Success
        },
        started_at: data.started_at,
        finished_at: data.finished_at,
        total_duration_ms: (data.finished_at - data.started_at)
            .num_milliseconds()
            .max(0) as u64,
        input: InputMeasurements {
            bytes: data.input.len(),
            characters: data.input.chars().count(),
        },
        lead: LeadRoleReport {
            status: data.lead_status,
            validation: data.lead_validation,
            model: data.lead_model,
            schema_version: LEAD_SCHEMA_VERSION,
            metrics: data.lead_metrics,
            response: data.lead_response,
        },
        delegation: DelegationReport {
            status: data.delegation_status,
            selected_task_id: data.selected_task_id,
            request_version: DEVELOPER_REQUEST_VERSION,
            request_json_bytes: data.developer_request_bytes,
            transmitted_fields: TRANSMITTED_FIELDS,
        },
        developer: DeveloperRoleReport {
            status: data.developer_status,
            validation: data.developer_validation,
            model: data.developer_model,
            schema_version: DEVELOPER_SCHEMA_VERSION,
            metrics: data.developer_metrics,
        },
        artifact_validation: ArtifactValidationReport {
            status: artifact_status,
            failure_count: usize::from(artifact_failed),
            failure_codes: artifact_failure_codes,
            file_count: data.generated_file_count,
            generated_bytes: data.generated_bytes,
        },
        workspace: WorkspaceReport {
            status: data.workspace_status,
            relative_path: data
                .published_workspace
                .map(|workspace| workspace.relative_path.as_str()),
            creation_duration_ms: data
                .published_workspace
                .map(|workspace| workspace.creation_duration.as_millis() as u64),
            files: workspace_files,
        },
        preview: PreviewReport {
            status: "not_started",
            human_approval: "pending",
        },
        failure: data.error.map(|error| FailureSummary {
            stage: data.failure_stage.unwrap_or("unknown"),
            kind: error.kind(),
            cause: error.to_string(),
        }),
    };
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| {
        AppError::new(
            ErrorKind::Report,
            format!("could not serialize execution report: {error}"),
        )
    })?;
    write_atomically(directory, &data, &bytes)
}

fn write_atomically(
    directory: &Path,
    data: &ReportData<'_>,
    bytes: &[u8],
) -> Result<PathBuf, AppError> {
    let timestamp = data
        .started_at
        .to_rfc3339_opts(SecondsFormat::Millis, true)
        .replace([':', '.'], "-");
    let final_path = directory.join(format!("{timestamp}-{}.json", data.run_id));
    let temporary_path = directory.join(format!(".{}.tmp", data.run_id));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary_path).map_err(|error| {
        AppError::new(
            ErrorKind::Report,
            format!("could not create temporary execution report: {error}"),
        )
    })?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temporary_path);
        return Err(AppError::new(
            ErrorKind::Report,
            format!("could not write execution report: {error}"),
        ));
    }
    fs::rename(&temporary_path, &final_path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        AppError::new(
            ErrorKind::Report,
            format!("could not finalize execution report: {error}"),
        )
    })?;
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{PublishedFile, PublishedWorkspace};
    use chrono::TimeZone;
    use tempfile::TempDir;

    #[test]
    fn v3_report_keeps_metadata_but_omits_generated_contents_and_raw_data() {
        let directory = TempDir::new().unwrap();
        let time = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let metrics = OllamaMetrics {
            prompt_eval_count: Some(3),
            ..Default::default()
        };
        let published = PublishedWorkspace {
            relative_path: "workspaces/run-test".to_owned(),
            files: vec![PublishedFile {
                path: "index.html".to_owned(),
                bytes: 42,
            }],
            creation_duration: std::time::Duration::from_millis(7),
        };
        let path = write_report(
            directory.path(),
            ReportData {
                run_id: "test-run",
                started_at: time,
                finished_at: time,
                input: "private raw request",
                lead_model: "lead",
                lead_status: StageStatus::Success,
                lead_validation: ValidationStatus::Passed,
                lead_metrics: Some(&metrics),
                lead_response: None,
                delegation_status: StageStatus::Success,
                selected_task_id: Some("task-1"),
                developer_request_bytes: Some(100),
                developer_model: "developer",
                developer_status: StageStatus::Success,
                developer_validation: ValidationStatus::Passed,
                developer_metrics: Some(&metrics),
                generated_file_count: 4,
                generated_bytes: 42,
                workspace_status: StageStatus::Success,
                published_workspace: Some(&published),
                failure_stage: None,
                error: None,
            },
        )
        .unwrap();
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains("execution-report-v3"));
        assert!(contents.contains("workspaces/run-test"));
        assert!(contents.contains("\"human_approval\": \"pending\""));
        assert!(!contents.contains("private raw request"));
        assert!(!contents.contains("generated secret content"));
        assert!(!contents.contains("content\""));
    }

    #[test]
    fn workspace_failure_preserves_completed_role_results() {
        let directory = TempDir::new().unwrap();
        let time = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let metrics = OllamaMetrics {
            eval_count: Some(7),
            ..Default::default()
        };
        let error = AppError::new(ErrorKind::Workspace, "staging failed");
        let path = write_report(
            directory.path(),
            ReportData {
                run_id: "failed-run",
                started_at: time,
                finished_at: time,
                input: "private raw request",
                lead_model: "lead-model",
                lead_status: StageStatus::Success,
                lead_validation: ValidationStatus::Passed,
                lead_metrics: Some(&metrics),
                lead_response: None,
                delegation_status: StageStatus::Success,
                selected_task_id: Some("task-1"),
                developer_request_bytes: Some(100),
                developer_model: "developer-model",
                developer_status: StageStatus::Success,
                developer_validation: ValidationStatus::Passed,
                developer_metrics: Some(&metrics),
                generated_file_count: 4,
                generated_bytes: 1234,
                workspace_status: StageStatus::Failure,
                published_workspace: None,
                failure_stage: Some("workspace"),
                error: Some(&error),
            },
        )
        .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(report["status"], "failure");
        assert_eq!(report["lead"]["status"], "success");
        assert_eq!(report["lead"]["validation"], "passed");
        assert_eq!(report["lead"]["metrics"]["eval_count"], 7);
        assert_eq!(report["developer"]["status"], "success");
        assert_eq!(report["developer"]["validation"], "passed");
        assert_eq!(report["developer"]["metrics"]["eval_count"], 7);
        assert_eq!(report["artifact_validation"]["status"], "passed");
        assert_eq!(report["artifact_validation"]["generated_bytes"], 1234);
        assert_eq!(report["workspace"]["status"], "failure");
        assert_eq!(report["failure"]["stage"], "workspace");
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("private raw request")
        );
    }

    #[test]
    fn developer_validation_report_uses_the_stable_error_code() {
        let directory = TempDir::new().unwrap();
        let time = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let error = AppError::coded(
            ErrorKind::Validation,
            "missing_dom_target",
            "required target is absent",
        );
        let path = write_report(
            directory.path(),
            ReportData {
                run_id: "invalid-run",
                started_at: time,
                finished_at: time,
                input: "private raw request",
                lead_model: "lead",
                lead_status: StageStatus::Success,
                lead_validation: ValidationStatus::Passed,
                lead_metrics: None,
                lead_response: None,
                delegation_status: StageStatus::Success,
                selected_task_id: Some("task-1"),
                developer_request_bytes: Some(100),
                developer_model: "developer",
                developer_status: StageStatus::Failure,
                developer_validation: ValidationStatus::Failed,
                developer_metrics: None,
                generated_file_count: 0,
                generated_bytes: 0,
                workspace_status: StageStatus::NotAttempted,
                published_workspace: None,
                failure_stage: Some("developer"),
                error: Some(&error),
            },
        )
        .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(
            report["artifact_validation"]["failure_codes"],
            serde_json::json!(["missing_dom_target"])
        );
        assert_eq!(
            report["failure"]["cause"],
            "missing_dom_target: required target is absent"
        );
    }
}
