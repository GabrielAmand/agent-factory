use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const LEAD_SCHEMA_VERSION: &str = "lead-response-v1";
pub const DEVELOPER_SCHEMA_VERSION: &str = "developer-proposal-v1";
pub const DEVELOPER_REQUEST_VERSION: &str = "developer-request-v1";
pub const MAX_DEVELOPER_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_USER_REQUEST_CHARS: usize = 16_000;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeadResponse {
    pub summary: String,
    pub assumptions: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub tasks: Vec<LeadTask>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeadTask {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub acceptance_criteria: Vec<String>,
    pub depends_on: Vec<String>,
}

impl LeadResponse {
    pub fn parse_and_validate(json: &str) -> Result<Self, AppError> {
        let response: Self = serde_json::from_str(json).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("Lead response is not valid contract JSON: {error}"),
            )
        })?;
        response.validate()?;
        Ok(response)
    }

    fn validate(&self) -> Result<(), AppError> {
        validate_text("summary", &self.summary, 2_000)?;
        validate_collection("assumptions", &self.assumptions, 0, 10, 1_000)?;
        validate_collection(
            "acceptance_criteria",
            &self.acceptance_criteria,
            1,
            20,
            1_000,
        )?;
        if !(1..=20).contains(&self.tasks.len()) {
            return validation_error("tasks must contain between 1 and 20 items");
        }

        let task_ids: HashSet<&str> = self.tasks.iter().map(|task| task.id.as_str()).collect();
        if task_ids.len() != self.tasks.len() {
            return validation_error("task ids must be unique");
        }

        for task in &self.tasks {
            validate_task(task)?;
            validate_dependencies(task, &task_ids)?;
        }
        Ok(())
    }
}

pub fn select_first_ready_task(response: &LeadResponse) -> Result<&LeadTask, AppError> {
    response
        .tasks
        .iter()
        .find(|task| task.depends_on.is_empty())
        .ok_or_else(|| {
            AppError::new(
                ErrorKind::Delegation,
                "no Lead task is ready for delegation",
            )
        })
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperRequest<'a> {
    pub request_version: &'static str,
    pub selected_task_id: &'a str,
    pub selected_task_title: &'a str,
    pub selected_task_objective: &'a str,
    pub selected_task_acceptance_criteria: &'a [String],
}

impl<'a> DeveloperRequest<'a> {
    pub fn from_task(task: &'a LeadTask) -> Self {
        Self {
            request_version: DEVELOPER_REQUEST_VERSION,
            selected_task_id: &task.id,
            selected_task_title: &task.title,
            selected_task_objective: &task.objective,
            selected_task_acceptance_criteria: &task.acceptance_criteria,
        }
    }

    pub fn to_bounded_json(&self) -> Result<String, AppError> {
        let json = serde_json::to_string(self).map_err(|error| {
            AppError::new(
                ErrorKind::Delegation,
                format!("could not serialize Developer request: {error}"),
            )
        })?;
        if json.len() > MAX_DEVELOPER_REQUEST_BYTES {
            return Err(AppError::new(
                ErrorKind::Delegation,
                "Developer request JSON must not exceed 32768 bytes",
            ));
        }
        Ok(json)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeveloperDecision {
    ProposalReady,
    ClarificationRequired,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperProposal {
    pub decision: DeveloperDecision,
    pub task_id: String,
    pub summary: String,
    pub assumptions: Vec<String>,
    pub file_changes: Vec<FileChangeProposal>,
    pub tests: Vec<TestProposal>,
    pub risks: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileChangeProposal {
    pub path: String,
    pub action: FileChangeAction,
    pub objective: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeAction {
    Create,
    Modify,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestProposal {
    pub name: String,
    pub objective: String,
}

impl DeveloperProposal {
    pub fn parse_and_validate(json: &str, selected_task_id: &str) -> Result<Self, AppError> {
        let proposal: Self = serde_json::from_str(json).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("Developer proposal is not valid contract JSON: {error}"),
            )
        })?;
        proposal.validate(selected_task_id)?;
        Ok(proposal)
    }

    fn validate(&self, selected_task_id: &str) -> Result<(), AppError> {
        if self.task_id != selected_task_id {
            return validation_error(
                "Developer proposal task_id does not match the delegated task",
            );
        }
        validate_task_id(&self.task_id)?;
        validate_text("Developer summary", &self.summary, 2_000)?;
        validate_collection("Developer assumptions", &self.assumptions, 0, 10, 1_000)?;
        validate_collection("Developer risks", &self.risks, 0, 10, 1_000)?;
        validate_collection(
            "Developer open_questions",
            &self.open_questions,
            0,
            10,
            1_000,
        )?;
        if self.file_changes.len() > 20 {
            return validation_error("file_changes must contain at most 20 items");
        }
        if self.tests.len() > 20 {
            return validation_error("tests must contain at most 20 items");
        }
        match self.decision {
            DeveloperDecision::ProposalReady if self.file_changes.is_empty() => {
                return validation_error("proposal_ready requires at least one file change");
            }
            DeveloperDecision::ClarificationRequired if self.open_questions.is_empty() => {
                return validation_error("clarification_required requires an open question");
            }
            DeveloperDecision::ClarificationRequired if !self.file_changes.is_empty() => {
                return validation_error("clarification_required must not contain file changes");
            }
            _ => {}
        }

        let mut paths = HashSet::new();
        for change in &self.file_changes {
            validate_proposed_path(&change.path)?;
            validate_text("file change objective", &change.objective, 2_000)?;
            if !paths.insert(change.path.as_str()) {
                return validation_error("file change paths must be unique");
            }
        }
        let mut test_names = HashSet::new();
        for test in &self.tests {
            validate_text("test name", &test.name, 200)?;
            validate_text("test objective", &test.objective, 1_000)?;
            if !test_names.insert(test.name.as_str()) {
                return validation_error("test names must be unique");
            }
        }
        Ok(())
    }
}

fn validate_proposed_path(path: &str) -> Result<(), AppError> {
    if path.is_empty()
        || path.len() > 512
        || path.starts_with('/')
        || path.ends_with('/')
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
    {
        return validation_error("file change path is not a conservative repository-relative path");
    }
    let components: Vec<&str> = path.split('/').collect();
    if components.iter().any(|component| {
        component.is_empty()
            || *component == "."
            || *component == ".."
            || matches!(
                component.to_ascii_lowercase().as_str(),
                ".git" | ".agents" | ".codex" | "reports" | "target"
            )
            || is_secret_sensitive_name(component)
    }) {
        return validation_error("file change path contains a forbidden component");
    }
    Ok(())
}

fn is_secret_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == ".env"
        || lower.starts_with(".env.")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower == "id_rsa"
        || lower == "id_ed25519"
}

pub fn validate_user_request(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    if !(1..=MAX_USER_REQUEST_CHARS).contains(&length) {
        return Err(AppError::new(
            ErrorKind::Input,
            "user request must contain between 1 and 16000 characters after trimming",
        ));
    }
    Ok(trimmed.to_owned())
}

fn validate_task(task: &LeadTask) -> Result<(), AppError> {
    validate_task_id(&task.id)?;
    validate_text("task title", &task.title, 200)?;
    validate_text("task objective", &task.objective, 2_000)?;
    validate_collection(
        "task acceptance_criteria",
        &task.acceptance_criteria,
        1,
        20,
        1_000,
    )?;
    if task.depends_on.len() > 20 {
        return validation_error("task depends_on must contain at most 20 items");
    }
    for dependency in &task.depends_on {
        validate_task_id(dependency)?;
    }
    Ok(())
}

fn validate_dependencies(task: &LeadTask, task_ids: &HashSet<&str>) -> Result<(), AppError> {
    let mut dependencies = HashSet::new();
    for dependency in &task.depends_on {
        if dependency == &task.id {
            return validation_error(format!("task {} cannot depend on itself", task.id));
        }
        if !task_ids.contains(dependency.as_str()) {
            return validation_error(format!(
                "task {} depends on unknown task {dependency}",
                task.id
            ));
        }
        if !dependencies.insert(dependency.as_str()) {
            return validation_error(format!(
                "task {} contains duplicate dependency {dependency}",
                task.id
            ));
        }
    }
    Ok(())
}

fn validate_task_id(value: &str) -> Result<(), AppError> {
    let length = value.len();
    if !(1..=32).contains(&length)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return validation_error(
            "task ids must contain 1 to 32 lowercase ASCII letters, digits, or hyphens",
        );
    }
    Ok(())
}

fn validate_collection(
    name: &str,
    values: &[String],
    minimum: usize,
    maximum: usize,
    maximum_chars: usize,
) -> Result<(), AppError> {
    if values.len() < minimum || values.len() > maximum {
        return validation_error(format!(
            "{name} must contain between {minimum} and {maximum} items"
        ));
    }
    for value in values {
        validate_text(name, value, maximum_chars)?;
    }
    Ok(())
}

fn validate_text(name: &str, value: &str, maximum: usize) -> Result<(), AppError> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > maximum {
        return validation_error(format!(
            "{name} values must contain between 1 and {maximum} characters"
        ));
    }
    Ok(())
}

fn validation_error<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::new(ErrorKind::Validation, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> String {
        serde_json::json!({
            "summary": "Plan the requested change.",
            "assumptions": [],
            "acceptance_criteria": ["The requested behavior is covered."],
            "tasks": [{
                "id": "task-1",
                "title": "Implement the change",
                "objective": "Produce the smallest correct implementation.",
                "acceptance_criteria": ["Relevant tests pass."],
                "depends_on": []
            }]
        })
        .to_string()
    }

    #[test]
    fn accepts_valid_response() {
        assert!(LeadResponse::parse_and_validate(&valid_json()).is_ok());
    }

    #[test]
    fn rejects_unknown_fields() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["command"] = serde_json::json!("cargo test");
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_missing_fields() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value.as_object_mut().unwrap().remove("tasks");
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_task_without_depends_on() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["tasks"][0]
            .as_object_mut()
            .unwrap()
            .remove("depends_on");
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_semantically_invalid_task_id() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["tasks"][0]["id"] = serde_json::json!("Task 1");
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_excessive_summary() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["summary"] = serde_json::json!("x".repeat(2_001));
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_excessive_task_collection() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        let task = value["tasks"][0].clone();
        value["tasks"] = serde_json::Value::Array(vec![task; 21]);
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_excessive_task_objective() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["tasks"][0]["objective"] = serde_json::json!("x".repeat(2_001));
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rust_validation_enforces_every_string_maximum() {
        let cases: Vec<(&str, Box<dyn Fn(&mut serde_json::Value)>)> = vec![
            (
                "assumption",
                Box::new(|value| value["assumptions"] = serde_json::json!(["x".repeat(1_001)])),
            ),
            (
                "top-level acceptance criterion",
                Box::new(|value| {
                    value["acceptance_criteria"] = serde_json::json!(["x".repeat(1_001)])
                }),
            ),
            (
                "task id",
                Box::new(|value| value["tasks"][0]["id"] = serde_json::json!("x".repeat(33))),
            ),
            (
                "task title",
                Box::new(|value| value["tasks"][0]["title"] = serde_json::json!("x".repeat(201))),
            ),
            (
                "task acceptance criterion",
                Box::new(|value| {
                    value["tasks"][0]["acceptance_criteria"] =
                        serde_json::json!(["x".repeat(1_001)])
                }),
            ),
        ];

        for (name, mutate) in cases {
            let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
            mutate(&mut value);
            assert!(
                LeadResponse::parse_and_validate(&value.to_string()).is_err(),
                "Rust validation accepted an excessive {name}"
            );
        }

        assert!(validate_task_id(&"x".repeat(33)).is_err());
    }

    #[test]
    fn accepts_reference_to_existing_task() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        let mut second = value["tasks"][0].clone();
        second["id"] = serde_json::json!("task-2");
        second["depends_on"] = serde_json::json!(["task-1"]);
        value["tasks"].as_array_mut().unwrap().push(second);
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_ok());
    }

    #[test]
    fn rejects_unknown_dependency() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["tasks"][0]["depends_on"] = serde_json::json!(["missing-task"]);
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_self_dependency() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["tasks"][0]["depends_on"] = serde_json::json!(["task-1"]);
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_duplicate_dependency() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        let mut second = value["tasks"][0].clone();
        second["id"] = serde_json::json!("task-2");
        value["tasks"].as_array_mut().unwrap().push(second);
        value["tasks"][0]["depends_on"] = serde_json::json!(["task-2", "task-2"]);
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn rejects_duplicate_task_ids() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        let duplicate = value["tasks"][0].clone();
        value["tasks"].as_array_mut().unwrap().push(duplicate);
        assert!(LeadResponse::parse_and_validate(&value.to_string()).is_err());
    }

    #[test]
    fn validates_trimmed_user_request_limits() {
        assert_eq!(
            validate_user_request("  build this  ").unwrap(),
            "build this"
        );
        assert!(validate_user_request("   \n").is_err());
        assert!(validate_user_request(&"x".repeat(16_001)).is_err());
    }

    fn valid_proposal() -> serde_json::Value {
        serde_json::json!({"decision":"proposal_ready","task_id":"task-1","summary":"Focused proposal.","assumptions":[],"file_changes":[{"path":"src/example.rs","action":"create","objective":"Add the component."}],"tests":[{"name":"validates example","objective":"Cover the behavior."}],"risks":[],"open_questions":[]})
    }

    #[test]
    fn selects_first_ready_task_in_lead_order() {
        let mut response = LeadResponse::parse_and_validate(&valid_json()).unwrap();
        response.tasks.insert(
            0,
            LeadTask {
                id: "task-2".into(),
                title: "Blocked".into(),
                objective: "Wait for task one.".into(),
                acceptance_criteria: vec!["Dependency completes.".into()],
                depends_on: vec!["task-1".into()],
            },
        );
        assert_eq!(select_first_ready_task(&response).unwrap().id, "task-1");
    }

    #[test]
    fn developer_request_contains_only_version_and_approved_task_fields() {
        let response = LeadResponse::parse_and_validate(&valid_json()).unwrap();
        let value: serde_json::Value = serde_json::from_str(
            &DeveloperRequest::from_task(&response.tasks[0])
                .to_bounded_json()
                .unwrap(),
        )
        .unwrap();
        let keys: HashSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            HashSet::from([
                "request_version",
                "selected_task_id",
                "selected_task_title",
                "selected_task_objective",
                "selected_task_acceptance_criteria"
            ])
        );
    }

    #[test]
    fn developer_request_enforces_32_kib_limit() {
        let task = LeadTask {
            id: "task-1".into(),
            title: "title".into(),
            objective: "objective".into(),
            acceptance_criteria: vec!["x".repeat(MAX_DEVELOPER_REQUEST_BYTES)],
            depends_on: vec![],
        };
        assert!(
            DeveloperRequest::from_task(&task)
                .to_bounded_json()
                .is_err()
        );
    }

    #[test]
    fn accepts_both_developer_decisions() {
        assert!(
            DeveloperProposal::parse_and_validate(&valid_proposal().to_string(), "task-1").is_ok()
        );
        let mut value = valid_proposal();
        value["decision"] = serde_json::json!("clarification_required");
        value["file_changes"] = serde_json::json!([]);
        value["open_questions"] = serde_json::json!(["Which module owns this?"]);
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_ok());
    }

    #[test]
    fn enforces_developer_decision_invariants_and_task_match() {
        let mut value = valid_proposal();
        value["file_changes"] = serde_json::json!([]);
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        let mut value = valid_proposal();
        value["decision"] = serde_json::json!("clarification_required");
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        assert!(
            DeveloperProposal::parse_and_validate(&valid_proposal().to_string(), "task-2").is_err()
        );
    }

    #[test]
    fn rejects_unknown_fields_and_duplicates() {
        let mut value = valid_proposal();
        value["command"] = serde_json::json!("cargo test");
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        let mut value = valid_proposal();
        let change = value["file_changes"][0].clone();
        value["file_changes"].as_array_mut().unwrap().push(change);
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        let mut value = valid_proposal();
        let test = value["tests"][0].clone();
        value["tests"].as_array_mut().unwrap().push(test);
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
    }

    #[test]
    fn rejects_unsafe_or_secret_sensitive_paths() {
        for path in [
            "/tmp/x",
            "src//x",
            "src/./x",
            "src/../x",
            ".git/config",
            "reports/x",
            "secrets/.env.local",
            "cert.PEM",
            "id_ed25519",
            "src/file name.rs",
        ] {
            let mut value = valid_proposal();
            value["file_changes"][0]["path"] = serde_json::json!(path);
            assert!(
                DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err(),
                "accepted {path}"
            );
        }
    }

    #[test]
    fn rust_enforces_developer_string_limits() {
        for (field, length) in [
            ("summary", 2_001),
            ("assumptions", 1_001),
            ("risks", 1_001),
            ("open_questions", 1_001),
        ] {
            let mut value = valid_proposal();
            if field == "summary" {
                value[field] = serde_json::json!("x".repeat(length));
            } else {
                value[field] = serde_json::json!(["x".repeat(length)]);
            }
            assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
        for (field, length) in [("objective", 2_001), ("path", 513)] {
            let mut value = valid_proposal();
            value["file_changes"][0][field] = serde_json::json!("x".repeat(length));
            assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
        let mut value = valid_proposal();
        value["tests"][0]["name"] = serde_json::json!("x".repeat(201));
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        let mut value = valid_proposal();
        value["tests"][0]["objective"] = serde_json::json!("x".repeat(1_001));
        assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
    }

    #[test]
    fn rust_enforces_developer_collection_limits() {
        for field in ["assumptions", "risks", "open_questions"] {
            let mut value = valid_proposal();
            value[field] = serde_json::json!(vec!["item"; 11]);
            assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
        for field in ["file_changes", "tests"] {
            let mut value = valid_proposal();
            let item = value[field][0].clone();
            value[field] = serde_json::Value::Array(vec![item; 21]);
            assert!(DeveloperProposal::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
    }
}
