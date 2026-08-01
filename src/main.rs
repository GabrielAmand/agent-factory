mod config;
mod error;
mod ollama;
mod protocol;
mod report;

use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use chrono::Utc;
use config::Config;
use error::{AppError, ErrorKind};
use ollama::{OllamaMetrics, call_lead};
use protocol::{LeadResponse, validate_user_request};
use report::{ReportData, write_report};

const CONFIG_ROOT: &str = ".";
const MAX_STDIN_BYTES: u64 = 64 * 1024;
const LEAD_PROMPT: &str = include_str!("../prompts/lead-v1.txt");
const LEAD_SCHEMA: &str = include_str!("../schemas/lead-response-v1.json");

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), AppError> {
    eprintln!("agent-factory: validating configuration");
    let repository_root = Path::new(CONFIG_ROOT);
    let config = Config::load(repository_root)?;
    let schema_result: Result<serde_json::Value, AppError> = serde_json::from_str(LEAD_SCHEMA)
        .map_err(|error| {
            AppError::new(
                ErrorKind::Configuration,
                format!("embedded Lead schema is invalid: {error}"),
            )
        });

    let started_at = Utc::now();
    let run_id = format!(
        "{}-{}",
        started_at.timestamp_nanos_opt().unwrap_or_default(),
        std::process::id()
    );

    let mut input = String::new();
    let read_result = read_bounded_input(io::stdin(), &mut input);
    let report_input = input.trim().to_owned();
    let input_result = read_result.and_then(|_| validate_user_request(&input));

    let mut metrics: Option<OllamaMetrics> = None;
    let mut lead_response: Option<LeadResponse> = None;
    let mut terminal_error: Option<AppError> = None;
    match (schema_result, input_result) {
        (Ok(schema), Ok(request)) => {
            eprintln!("agent-factory: calling local Lead model");
            match call_lead(&config, &request, LEAD_PROMPT, &schema) {
                Ok(success) => {
                    metrics = Some(success.metrics);
                    lead_response = Some(success.lead_response);
                    eprintln!("agent-factory: Lead response validated");
                }
                Err(failure) => {
                    metrics = failure.metrics;
                    terminal_error = Some(*failure.error);
                }
            }
        }
        (Err(error), _) | (_, Err(error)) => {
            terminal_error = Some(error);
        }
    }

    let finished_at = Utc::now();
    eprintln!("agent-factory: writing execution report");
    let report_path = write_report(
        &config.report_directory,
        ReportData {
            run_id: &run_id,
            started_at,
            finished_at,
            model: &config.model,
            input: &report_input,
            metrics: metrics.as_ref(),
            lead_response: lead_response.as_ref(),
            error: terminal_error.as_ref(),
        },
    )?;
    eprintln!("agent-factory: report written to {}", report_path.display());

    if let Some(error) = terminal_error {
        return Err(error);
    }
    Ok(())
}

fn read_bounded_input(reader: impl Read, output: &mut String) -> Result<(), AppError> {
    reader
        .take(MAX_STDIN_BYTES + 1)
        .read_to_string(output)
        .map_err(|error| {
            AppError::new(ErrorKind::Input, format!("could not read stdin: {error}"))
        })?;
    if output.len() as u64 > MAX_STDIN_BYTES {
        return Err(AppError::new(
            ErrorKind::Input,
            "stdin must not exceed 65536 bytes",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn rejects_stdin_above_the_byte_limit_without_reading_it_all() {
        let input = vec![b'x'; MAX_STDIN_BYTES as usize + 100];
        let mut output = String::new();
        assert!(read_bounded_input(Cursor::new(input), &mut output).is_err());
        assert_eq!(output.len(), MAX_STDIN_BYTES as usize + 1);
    }
}
