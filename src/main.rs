mod config;
mod document_normalizer;
mod e2;
mod error;
mod explorer;
mod fact_bundle;
mod network_policy;
mod ollama;
mod preview;
mod protocol;
mod report;
#[allow(dead_code)] // Phase E0 research contracts are not activated by the V1 runtime.
mod research;
mod retriever;
mod source_registry;
mod workspace;

use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use chrono::Utc;
use config::Config;
use error::{AppError, ErrorKind};
use ollama::{OllamaMetrics, call_developer, call_lead};
use protocol::{DeveloperRequestV2, LeadResponse, select_first_ready_task, validate_user_request};
use report::{ReportData, StageStatus, ValidationStatus, write_report};
use workspace::{PublishedWorkspace, publish_workspace};

const CONFIG_ROOT: &str = ".";
const MAX_STDIN_BYTES: u64 = 64 * 1024;
const LEAD_PROMPT: &str = include_str!("../prompts/lead-v1.txt");
const LEAD_SCHEMA: &str = include_str!("../schemas/lead-response-v1.json");
const DEVELOPER_PROMPT: &str = include_str!("../prompts/developer-workspace-v1.txt");
const DEVELOPER_SCHEMA: &str = include_str!("../schemas/developer-workspace-v1.json");
const EXPLORER_PROMPT: &str = include_str!("../prompts/explorer-v1.txt");
const EXPLORER_SCHEMA: &str = include_str!("../schemas/explorer-response-v1.json");

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
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty() {
        run_generation()
    } else if arguments[0] == "retrieve-official" {
        run_retrieve_official(&arguments)
    } else if arguments[0] == "explore-official" {
        run_explore_official(&arguments)
    } else {
        run_preview(&arguments)
    }
}

fn run_explore_official(arguments: &[String]) -> Result<(), AppError> {
    if arguments.len() != 3 || arguments[1] != "--policy" {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "usage: agent-factory explore-official --policy <id>",
        ));
    }
    let config = Config::load(Path::new(CONFIG_ROOT))?;
    let registry = load_official_registry(&arguments[2])?;
    let schema = parse_embedded_schema("Explorer", EXPLORER_SCHEMA)?;
    let research = research::ResearchRequest {
        reason_code: research::ResearchReasonCode::OfficialExternalFacts,
        topic: research::DEVOPS_TOPIC.to_owned(),
        requested_count: registry.entries.len(),
        required_fields: research::REQUIRED_FACT_FIELDS
            .iter()
            .map(|v| (*v).to_owned())
            .collect(),
        source_policy: arguments[2].clone(),
    };
    let outcome = e2::run_with(
        &registry,
        &research,
        &config.retriever,
        &config.explorer,
        |request, retriever_config| retriever::retrieve(&registry, request, retriever_config),
        |request_json| {
            ollama::call_explorer(
                &config,
                &config.explorer,
                request_json,
                EXPLORER_PROMPT,
                &schema,
            )
            .map(|success| e2::ExplorerTransportOutput {
                json: success.output,
                metrics: success.metrics,
            })
            .map_err(|failure| failure.stable_code.unwrap_or("explorer_call_failed"))
        },
    );
    match outcome {
        Ok(result) => {
            let metadata = serde_json::json!({
                "policy_id": result.policy_id,
                "registry_version": result.registry_version,
                "expected_fact_count": result.expected_fact_count,
                "retrieved_source_count": result.retrieved_source_count,
                "retrievals": result.retrievals,
                "explorer_model": result.explorer_model,
                "expected_explorer_call_count": result.expected_explorer_call_count,
                "completed_explorer_call_count": result.completed_explorer_call_count,
                "explorer_calls": result.explorer_calls,
                "validated_fact_count": result.validated_fact_count,
                "fact_bundle_version": result.bundle.bundle_version,
                "digest_algorithm": result.digest.algorithm,
                "bundle_digest": result.digest.hex,
                "canonical_byte_count": result.digest.canonical_byte_count,
                "retrieval_duration_ms": result.retrieval_duration_ms,
                "explorer_duration_ms": result.explorer_duration_ms,
                "total_duration_ms": result.total_duration_ms,
            });
            println!("{metadata}");
            Ok(())
        }
        Err(failure) => {
            let diagnostic = serde_json::to_string(&failure).map_err(|_| {
                AppError::new(ErrorKind::Validation, "could not serialize E2 diagnostics")
            })?;
            eprintln!("agent-factory: exploration diagnostic {diagnostic}");
            Err(failure.as_app_error())
        }
    }
}

fn load_official_registry(
    policy_id: &str,
) -> Result<source_registry::OfficialSourceRegistry, AppError> {
    let path = match policy_id {
        "official-devops-tools-v1" => "official-sources/official-devops-tools-v1.json",
        "official-devops-tools-v2" => "official-sources/official-devops-tools-v2.json",
        _ => {
            return Err(AppError::coded(
                ErrorKind::Validation,
                "unknown_policy",
                "official source policy is not supported",
            ));
        }
    };
    let registry_json = std::fs::read_to_string(path).map_err(|_| {
        AppError::new(
            ErrorKind::Configuration,
            "official source registry is unavailable",
        )
    })?;
    source_registry::OfficialSourceRegistry::parse_and_validate(&registry_json)
}

fn run_retrieve_official(arguments: &[String]) -> Result<(), AppError> {
    if arguments.len() != 7
        || arguments[1] != "--policy"
        || arguments[3] != "--fact-id"
        || arguments[5] != "--source-id"
    {
        return Err(AppError::new(
            ErrorKind::Configuration,
            "usage: agent-factory retrieve-official --policy <id> --fact-id <id> --source-id <id>",
        ));
    }
    let config = Config::load(Path::new(CONFIG_ROOT))?;
    let registry = load_official_registry(&arguments[2])?;
    let result = match retriever::retrieve(
        &registry,
        retriever::RetrievalRequest {
            policy_id: &arguments[2],
            fact_id: &arguments[4],
            source_id: &arguments[6],
        },
        &config.retriever,
    ) {
        Ok(result) => result,
        Err(error) => {
            let metadata = serde_json::to_string(error.diagnostic()).map_err(|_| {
                AppError::new(
                    ErrorKind::Validation,
                    "could not serialize sanitized retrieval diagnostics",
                )
            })?;
            eprintln!("agent-factory: retrieval diagnostic {metadata}");
            return Err(AppError::coded(
                ErrorKind::Validation,
                error.code(),
                error.to_string(),
            ));
        }
    };
    let metadata = serde_json::json!({
        "requested_url": result.requested_canonical_url,
        "final_attempted_url": result.final_url,
        "redirect_chain": result.redirect_chain,
        "http_status": result.http_status,
        "content_type": result.content_type,
        "charset": result.charset,
        "content_encoding": result.content_encoding,
        "transferred_bytes": result.original_byte_count,
        "decoded_bytes": result.decoded_byte_count,
        "normalized_bytes": result.normalized_byte_count,
        "selected_ip_family": result.selected_address_family,
        "elapsed_ms": result.elapsed_ms,
    });
    eprintln!("agent-factory: retrieval diagnostic {metadata}");
    println!("{}", result.normalized_text);
    Ok(())
}

fn run_preview(arguments: &[String]) -> Result<(), AppError> {
    if arguments.len() != 5
        || arguments[0] != "preview"
        || arguments[1] != "--run-id"
        || arguments[3] != "--port"
    {
        return Err(AppError::new(
            ErrorKind::Preview,
            "usage: agent-factory preview --run-id <id> --port <1024-65535>",
        ));
    }
    let port = arguments[4].parse::<u16>().map_err(|_| {
        AppError::new(
            ErrorKind::Preview,
            "preview port must be between 1024 and 65535",
        )
    })?;
    let config = Config::load(Path::new(CONFIG_ROOT))?;
    preview::serve(&config.workspace_directory, &arguments[2], port)
}

fn run_generation() -> Result<(), AppError> {
    eprintln!("agent-factory: validating configuration");
    let config = Config::load(Path::new(CONFIG_ROOT))?;
    let started_at = Utc::now();
    let run_id = format!(
        "{}-{}",
        started_at.timestamp_nanos_opt().unwrap_or_default(),
        std::process::id()
    );

    let mut input = String::new();
    let input_result =
        read_bounded_input(io::stdin(), &mut input).and_then(|_| validate_user_request(&input));
    let report_input = input.trim().to_owned();
    let lead_schema = parse_embedded_schema("Lead", LEAD_SCHEMA);
    let developer_schema = parse_embedded_schema("Developer", DEVELOPER_SCHEMA);

    let mut lead_status = StageStatus::NotAttempted;
    let mut lead_validation = ValidationStatus::NotAttempted;
    let mut lead_metrics: Option<OllamaMetrics> = None;
    let mut lead_response: Option<LeadResponse> = None;
    let mut delegation_status = StageStatus::NotAttempted;
    let mut selected_task_id: Option<String> = None;
    let mut developer_request_bytes: Option<usize> = None;
    let mut developer_status = StageStatus::NotAttempted;
    let mut developer_validation = ValidationStatus::NotAttempted;
    let mut developer_metrics: Option<OllamaMetrics> = None;
    let mut generated_file_count = 0;
    let mut generated_bytes = 0;
    let mut workspace_status = StageStatus::NotAttempted;
    let mut published_workspace: Option<PublishedWorkspace> = None;
    let mut terminal_error: Option<AppError> = None;
    let mut failure_stage: Option<&'static str> = None;

    // The V2 workflow remains explicit and sequential: Lead, delegation, Developer, publication.
    match (input_result, lead_schema, developer_schema) {
        (Ok(request), Ok(lead_schema), Ok(developer_schema)) => {
            eprintln!("agent-factory: calling local Lead model");
            lead_status = StageStatus::Failure;
            match call_lead(&config, &request, LEAD_PROMPT, &lead_schema) {
                Ok(success) => {
                    lead_status = StageStatus::Success;
                    lead_validation = ValidationStatus::Passed;
                    eprintln!("agent-factory: Lead response validated");
                    let delegation_result =
                        select_first_ready_task(&success.output).and_then(|task| {
                            let task_id = task.id.clone();
                            let request_json = DeveloperRequestV2::from_task(
                                task,
                                &success.output.acceptance_criteria,
                            )
                            .to_bounded_json()?;
                            Ok((task_id, request_json))
                        });
                    lead_metrics = Some(success.metrics);
                    lead_response = Some(success.output);

                    match delegation_result {
                        Ok((task_id, developer_request)) => {
                            selected_task_id = Some(task_id.clone());
                            delegation_status = StageStatus::Success;
                            developer_request_bytes = Some(developer_request.len());
                            eprintln!("agent-factory: calling local Developer model");
                            developer_status = StageStatus::Failure;
                            match call_developer(
                                &config,
                                &developer_request,
                                &task_id,
                                DEVELOPER_PROMPT,
                                &developer_schema,
                            ) {
                                Ok(success) => {
                                    developer_status = StageStatus::Success;
                                    developer_validation = ValidationStatus::Passed;
                                    developer_metrics = Some(success.metrics);
                                    generated_file_count = success.output.files.len();
                                    generated_bytes = success.output.total_bytes();
                                    eprintln!("agent-factory: Developer workspace validated");
                                    workspace_status = StageStatus::Failure;
                                    eprintln!("agent-factory: publishing isolated workspace");
                                    match publish_workspace(
                                        &config.workspace_directory,
                                        &run_id,
                                        &success.output,
                                    ) {
                                        Ok(published) => {
                                            workspace_status = StageStatus::Success;
                                            eprintln!(
                                                "agent-factory: workspace published to {}",
                                                published.relative_path
                                            );
                                            published_workspace = Some(published);
                                        }
                                        Err(error) => {
                                            terminal_error = Some(error);
                                            failure_stage = Some("workspace");
                                        }
                                    }
                                }
                                Err(failure) => {
                                    developer_validation = validation_status(&failure.error);
                                    developer_metrics = failure.metrics;
                                    terminal_error = Some(*failure.error);
                                    failure_stage = Some("developer");
                                }
                            }
                        }
                        Err(error) => {
                            delegation_status = StageStatus::Failure;
                            terminal_error = Some(error);
                            failure_stage = Some("delegation");
                        }
                    }
                }
                Err(failure) => {
                    lead_validation = validation_status(&failure.error);
                    lead_metrics = failure.metrics;
                    terminal_error = Some(*failure.error);
                    failure_stage = Some("lead");
                }
            }
        }
        (Err(error), _, _) => {
            terminal_error = Some(error);
            failure_stage = Some("input");
        }
        (_, Err(error), _) | (_, _, Err(error)) => {
            terminal_error = Some(error);
            failure_stage = Some("configuration");
        }
    }

    let finished_at = Utc::now();
    eprintln!("agent-factory: writing execution report");
    let report_result = write_report(
        &config.report_directory,
        ReportData {
            run_id: &run_id,
            started_at,
            finished_at,
            input: &report_input,
            lead_model: &config.lead_model,
            lead_status,
            lead_validation,
            lead_metrics: lead_metrics.as_ref(),
            lead_response: lead_response.as_ref(),
            delegation_status,
            selected_task_id: selected_task_id.as_deref(),
            developer_request_bytes,
            developer_model: &config.developer_model,
            developer_status,
            developer_validation,
            developer_metrics: developer_metrics.as_ref(),
            generated_file_count,
            generated_bytes,
            workspace_status,
            published_workspace: published_workspace.as_ref(),
            failure_stage,
            error: terminal_error.as_ref(),
        },
    );
    let report_path = match report_result {
        Ok(path) => path,
        Err(error) => {
            if let Some(workspace) = &published_workspace {
                eprintln!(
                    "agent-factory: report failed after workspace publication; workspace retained at {}",
                    workspace.relative_path
                );
            }
            return Err(error);
        }
    };
    eprintln!("agent-factory: report written to {}", report_path.display());
    if let Some(error) = terminal_error {
        return Err(error);
    }
    Ok(())
}

fn parse_embedded_schema(role: &str, schema: &str) -> Result<serde_json::Value, AppError> {
    serde_json::from_str(schema).map_err(|error| {
        AppError::new(
            ErrorKind::Configuration,
            format!("embedded {role} schema is invalid: {error}"),
        )
    })
}

fn validation_status(error: &AppError) -> ValidationStatus {
    if matches!(error.kind(), ErrorKind::Validation) {
        ValidationStatus::Failed
    } else {
        ValidationStatus::NotAttempted
    }
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
    use super::*;
    use std::io::Cursor;

    #[test]
    fn rejects_stdin_above_the_byte_limit_without_reading_it_all() {
        let input = vec![b'x'; MAX_STDIN_BYTES as usize + 100];
        let mut output = String::new();
        assert!(read_bounded_input(Cursor::new(input), &mut output).is_err());
        assert_eq!(output.len(), MAX_STDIN_BYTES as usize + 1);
    }

    #[test]
    fn generation_schemas_omit_ollama_incompatible_max_length() {
        assert!(!LEAD_SCHEMA.contains("maxLength"));
        assert!(!DEVELOPER_SCHEMA.contains("maxLength"));
    }
}
