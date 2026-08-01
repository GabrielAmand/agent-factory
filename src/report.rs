use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::error::{AppError, ErrorKind};
use crate::ollama::OllamaMetrics;
use crate::protocol::{
    DEVELOPER_REQUEST_VERSION, DEVELOPER_SCHEMA_VERSION, DeveloperProposal, LEAD_SCHEMA_VERSION,
    LeadResponse,
};

pub const REPORT_VERSION: &str = "execution-report-v2";
pub const TRANSMITTED_FIELDS: [&str; 4] = [
    "selected_task_id",
    "selected_task_title",
    "selected_task_objective",
    "selected_task_acceptance_criteria",
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
pub struct FailureSummary {
    stage: &'static str,
    kind: ErrorKind,
    cause: String,
}

#[derive(Serialize)]
pub struct InputMeasurements {
    bytes: usize,
    characters: usize,
}

#[derive(Serialize)]
pub struct LeadRoleReport<'a> {
    status: StageStatus,
    validation: ValidationStatus,
    model: &'a str,
    schema_version: &'static str,
    metrics: Option<&'a OllamaMetrics>,
    response: Option<&'a LeadResponse>,
}

#[derive(Serialize)]
pub struct DelegationReport<'a> {
    status: StageStatus,
    selected_task_id: Option<&'a str>,
    request_version: &'static str,
    request_json_bytes: Option<usize>,
    transmitted_fields: [&'static str; 4],
}

#[derive(Serialize)]
pub struct DeveloperRoleReport<'a> {
    status: StageStatus,
    validation: ValidationStatus,
    model: &'a str,
    schema_version: &'static str,
    metrics: Option<&'a OllamaMetrics>,
    proposal: Option<&'a DeveloperProposal>,
}

#[derive(Serialize)]
pub struct ExecutionReport<'a> {
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
    pub developer_proposal: Option<&'a DeveloperProposal>,
    pub failure_stage: Option<&'static str>,
    pub error: Option<&'a AppError>,
}

pub fn write_report(directory: &Path, data: ReportData<'_>) -> Result<PathBuf, AppError> {
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
            proposal: data.developer_proposal,
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
    use crate::protocol::{LeadResponse, LeadTask};
    use chrono::TimeZone;
    use tempfile::TempDir;

    #[test]
    fn v2_report_preserves_lead_success_when_developer_fails_without_raw_data() {
        let directory = TempDir::new().unwrap();
        let time = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let metrics = OllamaMetrics {
            prompt_eval_count: Some(3),
            ..Default::default()
        };
        let lead_response = LeadResponse {
            summary: "validated lead summary".to_owned(),
            assumptions: vec![],
            acceptance_criteria: vec!["validated lead criterion".to_owned()],
            tasks: vec![LeadTask {
                id: "task-1".to_owned(),
                title: "Validated lead task".to_owned(),
                objective: "Describe the proposed change.".to_owned(),
                acceptance_criteria: vec!["The proposal is testable.".to_owned()],
                depends_on: vec![],
            }],
        };
        let path = write_report(
            directory.path(),
            ReportData {
                run_id: "test-run",
                started_at: time,
                finished_at: time,
                input: "private raw user request",
                lead_model: "lead",
                lead_status: StageStatus::Success,
                lead_validation: ValidationStatus::Passed,
                lead_metrics: Some(&metrics),
                lead_response: Some(&lead_response),
                delegation_status: StageStatus::Success,
                selected_task_id: Some("task-1"),
                developer_request_bytes: Some(100),
                developer_model: "developer",
                developer_status: StageStatus::Failure,
                developer_validation: ValidationStatus::Failed,
                developer_metrics: None,
                developer_proposal: None,
                failure_stage: Some("developer"),
                error: Some(&AppError::new(ErrorKind::Validation, "compact cause")),
            },
        )
        .unwrap();
        let contents = fs::read_to_string(path).unwrap();
        let report: serde_json::Value = serde_json::from_str(&contents).unwrap();

        assert_eq!(report["lead"]["status"], "success");
        assert_eq!(report["lead"]["validation"], "passed");
        assert_eq!(report["lead"]["model"], "lead");
        assert_eq!(report["lead"]["metrics"]["prompt_eval_count"], 3);
        assert_eq!(
            report["lead"]["response"]["summary"],
            "validated lead summary"
        );
        assert_eq!(report["developer"]["status"], "failure");
        assert_eq!(report["developer"]["validation"], "failed");
        assert!(!contents.contains("private raw user request"));
        assert!(!contents.contains("private raw prompt"));
        assert!(!contents.contains("private raw model response"));
        assert!(contents.contains("execution-report-v2"));
        assert!(!directory.path().join(".test-run.tmp").exists());
    }
}
