use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const LEAD_SCHEMA_VERSION: &str = "lead-response-v1";
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
}
