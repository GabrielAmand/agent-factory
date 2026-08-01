use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::error::{AppError, ErrorKind};
use crate::ollama::OllamaMetrics;
use crate::protocol::{LEAD_SCHEMA_VERSION, LeadResponse};

pub const REPORT_VERSION: &str = "execution-report-v1";

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Success,
    Failure,
}

#[derive(Serialize)]
pub struct FailureSummary {
    kind: ErrorKind,
    cause: String,
}

#[derive(Serialize)]
pub struct InputMeasurements {
    bytes: usize,
    characters: usize,
}

#[derive(Serialize)]
pub struct ExecutionReport<'a> {
    report_version: &'static str,
    schema_version: &'static str,
    run_id: &'a str,
    status: ReportStatus,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    total_duration_ms: u64,
    model: &'a str,
    input: InputMeasurements,
    validation_succeeded: bool,
    metrics: Option<&'a OllamaMetrics>,
    lead_response: Option<&'a LeadResponse>,
    failure: Option<FailureSummary>,
}

pub struct ReportData<'a> {
    pub run_id: &'a str,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub model: &'a str,
    pub input: &'a str,
    pub metrics: Option<&'a OllamaMetrics>,
    pub lead_response: Option<&'a LeadResponse>,
    pub error: Option<&'a AppError>,
}

pub fn write_report(directory: &Path, data: ReportData<'_>) -> Result<PathBuf, AppError> {
    let status = if data.error.is_some() {
        ReportStatus::Failure
    } else {
        ReportStatus::Success
    };
    let report = ExecutionReport {
        report_version: REPORT_VERSION,
        schema_version: LEAD_SCHEMA_VERSION,
        run_id: data.run_id,
        status,
        started_at: data.started_at,
        finished_at: data.finished_at,
        total_duration_ms: (data.finished_at - data.started_at)
            .num_milliseconds()
            .max(0) as u64,
        model: data.model,
        input: InputMeasurements {
            bytes: data.input.len(),
            characters: data.input.chars().count(),
        },
        validation_succeeded: data.lead_response.is_some(),
        metrics: data.metrics,
        lead_response: data.lead_response,
        failure: data.error.map(|error| FailureSummary {
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
    let timestamp = data
        .started_at
        .to_rfc3339_opts(SecondsFormat::Millis, true)
        .replace([':', '.'], "-");
    let filename = format!("{timestamp}-{}.json", data.run_id);
    let final_path = directory.join(filename);
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
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
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
    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn report_omits_raw_input_and_writes_atomically() {
        let directory = TempDir::new().unwrap();
        let time = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let path = write_report(
            directory.path(),
            ReportData {
                run_id: "test-run",
                started_at: time,
                finished_at: time,
                model: "test-model",
                input: "do not persist this request",
                metrics: None,
                lead_response: None,
                error: Some(&AppError::new(ErrorKind::Input, "invalid input")),
            },
        )
        .unwrap();

        let contents = fs::read_to_string(path).unwrap();
        assert!(!contents.contains("do not persist this request"));
        assert!(contents.contains("\"report_version\": \"execution-report-v1\""));
        assert!(!directory.path().join(".test-run.tmp").exists());
    }
}
