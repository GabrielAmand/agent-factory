use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const LEAD_V2_VERSION: &str = "lead-response-v2";
pub const DEVOPS_TOPIC: &str = "devops-tools";
pub const DEVOPS_POLICY: &str = "official-devops-tools-v1";
pub const DEVOPS_REQUESTED_COUNT: usize = 8;
pub const REQUIRED_FACT_FIELDS: [&str; 5] = [
    "display_name",
    "description",
    "tags",
    "official_url",
    "source_url",
];

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchMode {
    #[default]
    Off,
    Auto,
    Required,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeadResponseV2 {
    pub response_version: String,
    pub summary: String,
    pub assumptions: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub tasks: Vec<LeadTaskV2>,
    pub research: ResearchDirective,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeadTaskV2 {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub acceptance_criteria: Vec<String>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResearchDirective {
    NotRequired,
    Required(ResearchRequest),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchRequest {
    pub reason_code: ResearchReasonCode,
    pub topic: String,
    pub requested_count: usize,
    pub required_fields: Vec<String>,
    pub source_policy: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchReasonCode {
    OfficialExternalFacts,
    CurrentOfficialInformation,
    OfficialDocumentationLinks,
    UserExplicitlyRequestedSources,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationDecision {
    ContinueWithoutResearch,
    ResearchAccepted,
}

impl LeadResponseV2 {
    pub fn parse_and_validate(json: &str) -> Result<Self, AppError> {
        let response: Self = serde_json::from_str(json).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("Lead V2 response is not valid contract JSON: {error}"),
            )
        })?;
        if response.response_version != LEAD_V2_VERSION {
            return validation("unsupported Lead V2 response_version");
        }
        validate_lead_shape(&response)?;
        if let ResearchDirective::Required(request) = &response.research {
            validate_research_request(request)?;
        }
        Ok(response)
    }
}

pub fn evaluate_activation(
    mode: ResearchMode,
    directive: &ResearchDirective,
) -> Result<ActivationDecision, AppError> {
    match (mode, directive) {
        (ResearchMode::Off, ResearchDirective::NotRequired)
        | (ResearchMode::Auto, ResearchDirective::NotRequired) => {
            Ok(ActivationDecision::ContinueWithoutResearch)
        }
        (ResearchMode::Off, ResearchDirective::Required(_)) => Err(AppError::coded(
            ErrorKind::Validation,
            "research_forbidden",
            "the Lead requested research while research mode is off",
        )),
        (ResearchMode::Required, ResearchDirective::NotRequired) => Err(AppError::coded(
            ErrorKind::Validation,
            "required_research_missing",
            "research mode requires a validated research request",
        )),
        (ResearchMode::Auto | ResearchMode::Required, ResearchDirective::Required(request)) => {
            validate_research_request(request)?;
            Ok(ActivationDecision::ResearchAccepted)
        }
    }
}

fn validate_research_request(request: &ResearchRequest) -> Result<(), AppError> {
    if request.topic != DEVOPS_TOPIC {
        return validation("unsupported research topic");
    }
    if request.source_policy != DEVOPS_POLICY {
        return validation("unsupported research source_policy");
    }
    if request.requested_count != DEVOPS_REQUESTED_COUNT {
        return validation("official-devops-tools-v1 requires exactly 8 items");
    }
    if request.required_fields.as_slice() != REQUIRED_FACT_FIELDS {
        return validation("required_fields must exactly match the supported fact fields");
    }
    Ok(())
}

fn validate_lead_shape(response: &LeadResponseV2) -> Result<(), AppError> {
    validate_text("summary", &response.summary, 1, 2_000)?;
    validate_strings("assumptions", &response.assumptions, 0, 10, 1_000)?;
    validate_strings(
        "acceptance_criteria",
        &response.acceptance_criteria,
        1,
        20,
        1_000,
    )?;
    if !(1..=20).contains(&response.tasks.len()) {
        return validation("tasks must contain between 1 and 20 items");
    }
    let mut ids = std::collections::HashSet::new();
    for task in &response.tasks {
        if task.id.is_empty()
            || task.id.len() > 32
            || !task
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return validation("task id must use 1 to 32 lowercase ASCII ID characters");
        }
        validate_text("task title", &task.title, 1, 200)?;
        validate_text("task objective", &task.objective, 1, 2_000)?;
        validate_strings(
            "task acceptance_criteria",
            &task.acceptance_criteria,
            1,
            20,
            1_000,
        )?;
        if task.depends_on.len() > 20 {
            return validation("task depends_on must contain at most 20 items");
        }
        if !ids.insert(task.id.as_str()) {
            return validation("task IDs must be unique");
        }
    }
    let known: std::collections::HashSet<&str> =
        response.tasks.iter().map(|task| task.id.as_str()).collect();
    for task in &response.tasks {
        let mut dependencies = std::collections::HashSet::new();
        for dependency in &task.depends_on {
            if dependency == &task.id || !known.contains(dependency.as_str()) {
                return validation("task dependency is self-referential or unknown");
            }
            if !dependencies.insert(dependency.as_str()) {
                return validation("task dependencies must be unique");
            }
        }
    }
    Ok(())
}

fn validate_strings(
    name: &str,
    values: &[String],
    minimum: usize,
    maximum: usize,
    max_chars: usize,
) -> Result<(), AppError> {
    if !(minimum..=maximum).contains(&values.len()) {
        return validation(format!(
            "{name} must contain between {minimum} and {maximum} items"
        ));
    }
    for value in values {
        validate_text(name, value, 1, max_chars)?;
    }
    Ok(())
}

fn validate_text(name: &str, value: &str, min: usize, max: usize) -> Result<(), AppError> {
    let count = value.trim().chars().count();
    if !(min..=max).contains(&count) {
        return validation(format!(
            "{name} must contain between {min} and {max} characters"
        ));
    }
    Ok(())
}

fn validation<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::new(ErrorKind::Validation, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOT_REQUIRED: &str = include_str!("../tests/fixtures/e0/lead-v2-not-required.json");
    const REQUIRED: &str = include_str!("../tests/fixtures/e0/lead-v2-required.json");

    #[test]
    fn activation_matrix_is_fail_closed() {
        let not_required = &LeadResponseV2::parse_and_validate(NOT_REQUIRED)
            .unwrap()
            .research;
        let required = &LeadResponseV2::parse_and_validate(REQUIRED)
            .unwrap()
            .research;
        assert_eq!(
            evaluate_activation(ResearchMode::Off, not_required).unwrap(),
            ActivationDecision::ContinueWithoutResearch
        );
        assert_eq!(
            evaluate_activation(ResearchMode::Off, required)
                .unwrap_err()
                .code(),
            Some("research_forbidden")
        );
        assert_eq!(
            evaluate_activation(ResearchMode::Auto, not_required).unwrap(),
            ActivationDecision::ContinueWithoutResearch
        );
        assert_eq!(
            evaluate_activation(ResearchMode::Auto, required).unwrap(),
            ActivationDecision::ResearchAccepted
        );
        assert_eq!(
            evaluate_activation(ResearchMode::Required, required).unwrap(),
            ActivationDecision::ResearchAccepted
        );
        assert_eq!(
            evaluate_activation(ResearchMode::Required, not_required)
                .unwrap_err()
                .code(),
            Some("required_research_missing")
        );
    }

    #[test]
    fn rejects_unsupported_policy_values_and_model_urls() {
        for fixture in [
            include_str!("../tests/fixtures/e0/lead-v2-unsupported-topic.json"),
            include_str!("../tests/fixtures/e0/lead-v2-unsupported-policy.json"),
            include_str!("../tests/fixtures/e0/lead-v2-incorrect-count.json"),
            include_str!("../tests/fixtures/e0/lead-v2-unsupported-field.json"),
            include_str!("../tests/fixtures/e0/lead-v2-model-url-attempt.json"),
        ] {
            assert!(LeadResponseV2::parse_and_validate(fixture).is_err());
        }
    }

    #[test]
    fn research_mode_defaults_to_off() {
        assert_eq!(ResearchMode::default(), ResearchMode::Off);
    }

    #[test]
    fn every_e0_schema_is_valid_json() {
        for schema in [
            include_str!("../schemas/official-source-registry-v1.json"),
            include_str!("../schemas/lead-response-v2.json"),
            include_str!("../schemas/explorer-request-v1.json"),
            include_str!("../schemas/explorer-response-v1.json"),
            include_str!("../schemas/fact-bundle-v1.json"),
            include_str!("../schemas/developer-workspace-v2.json"),
        ] {
            let parsed: serde_json::Value = serde_json::from_str(schema).unwrap();
            assert_eq!(
                parsed["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
        }
    }
}
