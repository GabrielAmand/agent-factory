use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const LEAD_SCHEMA_VERSION: &str = "lead-response-v1";
pub const DEVELOPER_SCHEMA_VERSION: &str = "developer-workspace-v1";
pub const DEVELOPER_REQUEST_VERSION: &str = "developer-request-v2";
pub const MAX_DEVELOPER_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_USER_REQUEST_CHARS: usize = 16_000;
pub const MAX_GENERATED_FILE_BYTES: usize = 32 * 1024;
pub const MAX_GENERATED_TOTAL_BYTES: usize = 96 * 1024;
pub const GENERATED_FILE_NAMES: [&str; 4] =
    ["index.html", "app.js", "styles.css", "resources.json"];

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
pub struct DeveloperRequestV2<'a> {
    pub request_version: &'static str,
    pub selected_task_id: &'a str,
    pub selected_task_title: &'a str,
    pub selected_task_objective: &'a str,
    pub selected_task_acceptance_criteria: &'a [String],
    pub lead_acceptance_criteria: &'a [String],
}

impl<'a> DeveloperRequestV2<'a> {
    pub fn from_task(task: &'a LeadTask, lead_acceptance_criteria: &'a [String]) -> Self {
        Self {
            request_version: DEVELOPER_REQUEST_VERSION,
            selected_task_id: &task.id,
            selected_task_title: &task.title,
            selected_task_objective: &task.objective,
            selected_task_acceptance_criteria: &task.acceptance_criteria,
            lead_acceptance_criteria,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperWorkspace {
    pub response_version: String,
    pub task_id: String,
    pub files: Vec<GeneratedFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourcesDocument {
    resources_version: String,
    tools: Vec<ResourceTool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceTool {
    name: String,
    description: String,
    tags: Vec<String>,
}

impl DeveloperWorkspace {
    pub fn parse_and_validate(json: &str, selected_task_id: &str) -> Result<Self, AppError> {
        let workspace: Self = serde_json::from_str(json).map_err(|error| {
            AppError::new(
                ErrorKind::Validation,
                format!("Developer workspace is not valid contract JSON: {error}"),
            )
        })?;
        workspace.validate(selected_task_id)?;
        Ok(workspace)
    }

    pub fn total_bytes(&self) -> usize {
        self.files.iter().map(|file| file.content.len()).sum()
    }

    pub fn file(&self, name: &str) -> Option<&GeneratedFile> {
        self.files.iter().find(|file| file.path == name)
    }

    fn validate(&self, selected_task_id: &str) -> Result<(), AppError> {
        if self.response_version != DEVELOPER_SCHEMA_VERSION {
            return validation_error("Developer workspace response_version is unsupported");
        }
        if self.task_id != selected_task_id {
            return validation_error(
                "Developer workspace task_id does not match the delegated task",
            );
        }
        validate_task_id(&self.task_id)?;
        if self.files.len() != GENERATED_FILE_NAMES.len() {
            return validation_error("Developer workspace must contain exactly four files");
        }

        let mut names = HashSet::new();
        for file in &self.files {
            if !GENERATED_FILE_NAMES.contains(&file.path.as_str()) {
                return validation_error("Developer workspace contains an unapproved filename");
            }
            if !names.insert(file.path.as_str()) {
                return validation_error("Developer workspace filenames must be unique");
            }
            validate_generated_content(file)?;
        }
        if GENERATED_FILE_NAMES
            .iter()
            .any(|name| !names.contains(name))
        {
            return validation_error("Developer workspace is missing a required file");
        }
        if self.total_bytes() > MAX_GENERATED_TOTAL_BYTES {
            return validation_error("combined generated content must not exceed 98304 bytes");
        }

        let html = &self.required_file("index.html")?.content;
        let javascript = &self.required_file("app.js")?.content;
        let css = &self.required_file("styles.css")?.content;
        let resources = &self.required_file("resources.json")?.content;
        validate_dom_consistency(html, javascript)?;
        validate_html(html)?;
        validate_javascript(javascript)?;
        validate_no_external_or_secret_content("styles.css", css)?;
        validate_no_external_or_secret_content("resources.json", resources)?;
        validate_resources(resources)?;
        Ok(())
    }

    fn required_file(&self, name: &str) -> Result<&GeneratedFile, AppError> {
        self.file(name).ok_or_else(|| {
            AppError::new(
                ErrorKind::Validation,
                format!("Developer workspace is missing {name}"),
            )
        })
    }
}

fn validate_generated_content(file: &GeneratedFile) -> Result<(), AppError> {
    if file.content.trim().is_empty() {
        return validation_error(format!("{} must not be empty", file.path));
    }
    if file.content.len() > MAX_GENERATED_FILE_BYTES {
        return validation_error(format!("{} must not exceed 32768 bytes", file.path));
    }
    if file.content.starts_with('\u{feff}') || file.content.contains('\0') {
        return validation_error(format!("{} contains a forbidden BOM or NUL", file.path));
    }
    validate_no_external_or_secret_content(&file.path, &file.content)
}

const REQUIRED_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";

fn validate_html(content: &str) -> Result<(), AppError> {
    let lower = content.to_ascii_lowercase();
    let stylesheet = "<link rel=\"stylesheet\" href=\"styles.css\">";
    let script = "<script src=\"app.js\" defer></script>";
    let csp = format!("<meta http-equiv=\"content-security-policy\" content=\"{REQUIRED_CSP}\">");
    for required in [stylesheet, script, &csp] {
        if !lower.contains(&required.to_ascii_lowercase()) {
            return validation_error("index.html is missing an approved asset reference or CSP");
        }
    }
    let remaining_references = lower.replacen(stylesheet, "", 1).replacen(script, "", 1);
    if remaining_references.contains("src=") || remaining_references.contains("href=") {
        return validation_error("index.html contains an unapproved asset reference");
    }
    if lower.matches("<script").count() != 1
        || [
            "<form",
            "<iframe",
            "<video",
            "<audio",
            "<source",
            "onclick=",
            "onchange=",
            "onload=",
            "onerror=",
            "onsubmit=",
            "oninput=",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return validation_error("index.html contains forbidden active or external-capable markup");
    }
    Ok(())
}

const MISSING_DOM_TARGET: &str = "missing_dom_target";
const DUPLICATE_DOM_ID: &str = "duplicate_dom_id";
const MISSING_APPLICATION_BODY: &str = "missing_application_body";

fn validate_dom_consistency(html: &str, javascript: &str) -> Result<(), AppError> {
    let scan = scan_html(html)?;
    for referenced_id in extract_literal_dom_references(javascript)? {
        if !scan.ids.contains(referenced_id.as_str()) {
            return coded_validation_error(
                MISSING_DOM_TARGET,
                format!("JavaScript references missing HTML id {referenced_id}"),
            );
        }
    }
    if scan.body_count != 1 || !scan.body_closed || !scan.has_application_element {
        return coded_validation_error(
            MISSING_APPLICATION_BODY,
            "index.html must contain one closed body with genuine application markup",
        );
    }
    Ok(())
}

struct HtmlScan {
    ids: HashSet<String>,
    body_count: usize,
    body_closed: bool,
    has_application_element: bool,
}

fn scan_html(html: &str) -> Result<HtmlScan, AppError> {
    let lower = html.to_ascii_lowercase();
    let mut scan = HtmlScan {
        ids: HashSet::new(),
        body_count: 0,
        body_closed: false,
        has_application_element: false,
    };
    let mut inside_body = false;
    let mut position = 0;

    while let Some(relative_start) = html[position..].find('<') {
        let start = position + relative_start;
        if html[start..].starts_with("<!--") {
            let comment_content = start + 4;
            let Some(relative_end) = html[comment_content..].find("-->") else {
                return malformed_html("index.html contains an unterminated comment");
            };
            position = comment_content + relative_end + 3;
            continue;
        }

        let Some(end) = find_html_tag_end(html, start + 1) else {
            return malformed_html("index.html contains an unterminated or malformed tag");
        };
        let tag = &html[start + 1..end];
        let trimmed = tag.trim_start();
        if trimmed.starts_with(['!', '?']) {
            position = end + 1;
            continue;
        }

        let closing = trimmed.starts_with('/');
        let tag_without_slash = if closing {
            trimmed[1..].trim_start()
        } else {
            trimmed
        };
        let Some((name, name_end)) = parse_tag_name(tag_without_slash) else {
            return malformed_html("index.html contains a malformed tag name");
        };
        let name = name.to_ascii_lowercase();

        if closing {
            if name == "body" {
                if !inside_body || scan.body_closed {
                    return malformed_html("index.html contains an unmatched body closing tag");
                }
                inside_body = false;
                scan.body_closed = true;
            }
            position = end + 1;
            continue;
        }

        extract_ids_from_tag(tag_without_slash, name_end, &mut scan.ids)?;
        if name == "body" {
            scan.body_count += 1;
            if scan.body_count > 1 || inside_body || scan.body_closed {
                return malformed_html("index.html must not contain multiple body elements");
            }
            inside_body = true;
        } else if inside_body
            && !is_asset_or_inert_element(&name)
            && !matches!(name.as_str(), "html" | "head")
        {
            scan.has_application_element = true;
        }

        position = end + 1;
        if is_raw_or_inert_element(&name) {
            position = skip_raw_or_inert_content(html, &lower, position, &name)?;
        }
    }

    Ok(scan)
}

fn parse_tag_name(tag: &str) -> Option<(&str, usize)> {
    let end = tag
        .find(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '-' | ':')
        })
        .unwrap_or(tag.len());
    (end > 0).then_some((&tag[..end], end))
}

fn find_html_tag_end(html: &str, content_start: usize) -> Option<usize> {
    let mut quote = None;
    for (relative, byte) in html.as_bytes()[content_start..].iter().copied().enumerate() {
        match (quote, byte) {
            (None, b'\'' | b'"') => quote = Some(byte),
            (Some(active), current) if active == current => quote = None,
            (None, b'>') => return Some(content_start + relative),
            _ => {}
        }
    }
    None
}

fn extract_ids_from_tag(
    tag: &str,
    name_end: usize,
    ids: &mut HashSet<String>,
) -> Result<(), AppError> {
    let bytes = tag.as_bytes();
    let original = tag.as_bytes();
    let mut cursor = name_end;
    let mut id_count = 0;
    while cursor < bytes.len() {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] == b'/' {
            break;
        }
        let attribute_start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'/'))
        {
            cursor += 1;
        }
        if attribute_start == cursor {
            cursor += 1;
            continue;
        }
        let attribute_name = &tag[attribute_start..cursor];
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let has_value = bytes.get(cursor) == Some(&b'=');
        if !has_value {
            if attribute_name.eq_ignore_ascii_case("id") {
                return empty_id_error();
            }
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let (value_start, value_end) = match bytes.get(cursor).copied() {
            Some(quote @ (b'\'' | b'"')) => {
                let value_start = cursor + 1;
                let Some(relative_end) =
                    bytes[value_start..].iter().position(|byte| *byte == quote)
                else {
                    return malformed_html("index.html contains an unterminated attribute value");
                };
                (value_start, value_start + relative_end)
            }
            Some(_) => {
                let value_start = cursor;
                let value_end = bytes[value_start..]
                    .iter()
                    .position(|byte| byte.is_ascii_whitespace() || *byte == b'/')
                    .map_or(bytes.len(), |end| value_start + end);
                (value_start, value_end)
            }
            None => (cursor, cursor),
        };
        cursor = if bytes
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b'\'' | b'"'))
        {
            value_end + 1
        } else {
            value_end
        };

        if attribute_name.eq_ignore_ascii_case("id") {
            id_count += 1;
            if id_count > 1 {
                return coded_validation_error(
                    DUPLICATE_DOM_ID,
                    "an HTML start tag contains more than one id attribute",
                );
            }
            let id = std::str::from_utf8(&original[value_start..value_end]).map_err(|_| {
                AppError::new(ErrorKind::Validation, "index.html contains invalid UTF-8")
            })?;
            if id.is_empty() {
                return empty_id_error();
            }
            if !ids.insert(id.to_owned()) {
                return coded_validation_error(
                    DUPLICATE_DOM_ID,
                    format!("index.html contains duplicate id {id}"),
                );
            }
        }
    }
    Ok(())
}

fn skip_raw_or_inert_content(
    html: &str,
    lower: &str,
    content_start: usize,
    name: &str,
) -> Result<usize, AppError> {
    if name == "template" {
        return skip_template_content(html, lower, content_start);
    }
    let closing = format!("</{name}");
    let Some(relative_start) = lower[content_start..].find(&closing) else {
        return malformed_html(format!("index.html contains an unclosed {name} element"));
    };
    let close_start = content_start + relative_start;
    let boundary = lower.as_bytes().get(close_start + closing.len()).copied();
    if !boundary.is_some_and(|byte| byte == b'>' || byte.is_ascii_whitespace()) {
        return malformed_html(format!(
            "index.html contains a malformed {name} closing tag"
        ));
    }
    let Some(close_end) = find_html_tag_end(html, close_start + 2) else {
        return malformed_html(format!("index.html contains an unclosed {name} element"));
    };
    Ok(close_end + 1)
}

fn skip_template_content(html: &str, lower: &str, content_start: usize) -> Result<usize, AppError> {
    let mut position = content_start;
    let mut depth = 1;
    while let Some(relative_start) = html[position..].find('<') {
        let start = position + relative_start;
        if html[start..].starts_with("<!--") {
            let comment_content = start + 4;
            let Some(relative_end) = html[comment_content..].find("-->") else {
                return malformed_html("index.html contains an unterminated template comment");
            };
            position = comment_content + relative_end + 3;
            continue;
        }
        let Some(end) = find_html_tag_end(html, start + 1) else {
            return malformed_html("index.html contains malformed template content");
        };
        let trimmed = html[start + 1..end].trim_start();
        let closing = trimmed.starts_with('/');
        let tag = if closing {
            trimmed[1..].trim_start()
        } else {
            trimmed
        };
        let Some((name, _)) = parse_tag_name(tag) else {
            return malformed_html("index.html contains malformed template content");
        };
        let name = name.to_ascii_lowercase();
        position = end + 1;
        if name == "template" {
            if closing {
                depth -= 1;
                if depth == 0 {
                    return Ok(position);
                }
            } else {
                depth += 1;
            }
        } else if !closing && matches!(name.as_str(), "script" | "style" | "textarea" | "title") {
            position = skip_raw_or_inert_content(html, lower, position, &name)?;
        }
    }
    malformed_html("index.html contains an unclosed template element")
}

fn is_raw_or_inert_element(name: &str) -> bool {
    matches!(name, "script" | "style" | "textarea" | "title" | "template")
}

fn is_asset_or_inert_element(name: &str) -> bool {
    matches!(
        name,
        "script" | "style" | "link" | "meta" | "title" | "template"
    )
}

fn empty_id_error<T>() -> Result<T, AppError> {
    coded_validation_error(MISSING_DOM_TARGET, "HTML id attributes must not be empty")
}

fn malformed_html<T>(message: impl Into<String>) -> Result<T, AppError> {
    coded_validation_error(MISSING_APPLICATION_BODY, message)
}

fn extract_literal_dom_references(javascript: &str) -> Result<Vec<String>, AppError> {
    let mut references = Vec::new();
    for (method, query_selector) in [
        ("document.getElementById", false),
        ("document.querySelector", true),
    ] {
        let mut remaining = javascript;
        while let Some(found) = remaining.find(method) {
            remaining = &remaining[found + method.len()..];
            if let Some((literal, rest)) = parse_direct_string_argument(remaining) {
                if query_selector {
                    if let Some(id) = literal.strip_prefix('#')
                        && is_simple_literal_id(id)
                    {
                        references.push(id.to_owned());
                    }
                    if literal == "#" {
                        return empty_id_error();
                    }
                } else if literal.is_empty() {
                    return empty_id_error();
                } else if is_direct_literal_id(literal) {
                    references.push(literal.to_owned());
                }
                remaining = rest;
            }
        }
    }
    Ok(references)
}

fn parse_direct_string_argument(source: &str) -> Option<(&str, &str)> {
    let source = source.trim_start();
    let source = source.strip_prefix('(')?.trim_start();
    let quote = source.as_bytes().first().copied()?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let value_start = 1;
    let value_end = source.as_bytes()[value_start..]
        .iter()
        .position(|byte| *byte == quote)?
        + value_start;
    let rest = source[value_end + 1..].trim_start();
    let rest = rest.strip_prefix(')')?;
    Some((&source[value_start..value_end], rest))
}

fn is_simple_literal_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn is_direct_literal_id(value: &str) -> bool {
    !value.is_empty() && !value.contains('\\') && !value.chars().any(char::is_control)
}

fn validate_javascript(content: &str) -> Result<(), AppError> {
    let compact: String = content
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let lower = compact.to_ascii_lowercase();
    if !lower.contains("fetch(\"resources.json\")") && !lower.contains("fetch('resources.json')") {
        return validation_error("app.js must load resources.json with a same-origin request");
    }
    let allowed = lower
        .replace("fetch(\"resources.json\")", "")
        .replace("fetch('resources.json')", "");
    if allowed.contains("fetch(")
        || [
            "xmlhttprequest",
            "websocket",
            "eventsource",
            "sendbeacon",
            "eval(",
            "newfunction(",
        ]
        .iter()
        .any(|marker| allowed.contains(marker))
        || [
            "window.open(",
            "location=",
            "location.href",
            "location.assign(",
            "location.replace(",
            "document.location",
            "import(",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return validation_error("app.js contains a forbidden network or dynamic-code API");
    }
    Ok(())
}

fn validate_no_external_or_secret_content(name: &str, content: &str) -> Result<(), AppError> {
    let lower = content.to_ascii_lowercase();
    if [
        "http://",
        "https://",
        "ftp://",
        "src=\"//",
        "src='//",
        "href=\"//",
        "href='//",
        "url(//",
        "@import",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return validation_error(format!("{name} contains an external resource reference"));
    }
    if [
        "-----begin ",
        "authorization:",
        "bearer ",
        "api_key=",
        "api-key=",
        "apikey=",
        "client_secret=",
        "client-secret=",
        "password=",
        "access_token=",
        "access-token=",
        "akia",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return validation_error(format!("{name} contains secret-sensitive content"));
    }
    Ok(())
}

fn validate_resources(content: &str) -> Result<(), AppError> {
    let resources: ResourcesDocument = serde_json::from_str(content).map_err(|error| {
        AppError::new(
            ErrorKind::Validation,
            format!("resources.json is invalid: {error}"),
        )
    })?;
    if resources.resources_version != "resources-v1" || !(1..=50).contains(&resources.tools.len()) {
        return validation_error("resources.json has an unsupported version or tool count");
    }
    let mut names = HashSet::new();
    for tool in resources.tools {
        validate_text("resource tool name", &tool.name, 100)?;
        validate_text("resource tool description", &tool.description, 500)?;
        if !names.insert(tool.name) {
            return validation_error("resource tool names must be unique");
        }
        if !(1..=10).contains(&tool.tags.len()) {
            return validation_error("resource tool tags must contain between 1 and 10 items");
        }
        let mut tags = HashSet::new();
        for tag in tool.tags {
            if !(1..=50).contains(&tag.len())
                || !tag
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            {
                return validation_error(
                    "resource tags must use 1 to 50 lowercase letters, digits, or hyphens",
                );
            }
            if !tags.insert(tag) {
                return validation_error("resource tags must be unique within each tool");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeveloperDecision {
    ProposalReady,
    ClarificationRequired,
}

#[cfg(test)]
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

#[cfg(test)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileChangeProposal {
    pub path: String,
    pub action: FileChangeAction,
    pub objective: String,
}

#[cfg(test)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeAction {
    Create,
    Modify,
}

#[cfg(test)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestProposal {
    pub name: String,
    pub objective: String,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

fn coded_validation_error<T>(
    code: &'static str,
    message: impl Into<String>,
) -> Result<T, AppError> {
    Err(AppError::coded(ErrorKind::Validation, code, message))
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
            &DeveloperRequestV2::from_task(&response.tasks[0], &response.acceptance_criteria)
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
                "selected_task_acceptance_criteria",
                "lead_acceptance_criteria"
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
            DeveloperRequestV2::from_task(&task, &[])
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

    fn valid_workspace_json() -> serde_json::Value {
        serde_json::json!({
            "response_version": "developer-workspace-v1",
            "task_id": "task-1",
            "files": [
                {"path":"index.html","content":"<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"></head><body><main id=\"tool-list\"></main><script src=\"app.js\" defer></script></body></html>"},
                {"path":"app.js","content":"fetch(\"resources.json\").then(response => response.json());"},
                {"path":"styles.css","content":"body { color: black; }"},
                {"path":"resources.json","content":"{\"resources_version\":\"resources-v1\",\"tools\":[{\"name\":\"Docker\",\"description\":\"Containers\",\"tags\":[\"containers\"]}]}"}
            ]
        })
    }

    #[test]
    fn accepts_complete_developer_workspace() {
        assert!(
            DeveloperWorkspace::parse_and_validate(&valid_workspace_json().to_string(), "task-1")
                .is_ok()
        );
    }

    #[test]
    fn rejects_missing_duplicate_extra_and_nested_files() {
        let mut missing = valid_workspace_json();
        missing["files"].as_array_mut().unwrap().pop();
        assert!(DeveloperWorkspace::parse_and_validate(&missing.to_string(), "task-1").is_err());

        let mut duplicate = valid_workspace_json();
        duplicate["files"][3]["path"] = serde_json::json!("index.html");
        assert!(DeveloperWorkspace::parse_and_validate(&duplicate.to_string(), "task-1").is_err());

        for path in ["extra.txt", "assets/app.js", "../app.js", "/app.js"] {
            let mut invalid = valid_workspace_json();
            invalid["files"][1]["path"] = serde_json::json!(path);
            assert!(
                DeveloperWorkspace::parse_and_validate(&invalid.to_string(), "task-1").is_err()
            );
        }
    }

    #[test]
    fn enforces_generated_file_and_total_byte_limits() {
        let mut excessive_file = valid_workspace_json();
        excessive_file["files"][2]["content"] =
            serde_json::json!("x".repeat(MAX_GENERATED_FILE_BYTES + 1));
        assert!(
            DeveloperWorkspace::parse_and_validate(&excessive_file.to_string(), "task-1").is_err()
        );

        let mut excessive_total = valid_workspace_json();
        excessive_total["files"][0]["content"] =
            serde_json::json!("x".repeat(MAX_GENERATED_FILE_BYTES));
        excessive_total["files"][1]["content"] =
            serde_json::json!("x".repeat(MAX_GENERATED_FILE_BYTES));
        excessive_total["files"][2]["content"] =
            serde_json::json!("x".repeat(MAX_GENERATED_FILE_BYTES));
        excessive_total["files"][3]["content"] = serde_json::json!("x");
        assert!(
            DeveloperWorkspace::parse_and_validate(&excessive_total.to_string(), "task-1").is_err()
        );
    }

    #[test]
    fn rejects_invalid_resources_and_duplicate_resource_values() {
        for content in [
            "not json",
            "{\"resources_version\":\"wrong\",\"tools\":[]}",
            "{\"resources_version\":\"resources-v1\",\"tools\":[{\"name\":\"A\",\"description\":\"D\",\"tags\":[\"Bad Tag\"]}]}",
            "{\"resources_version\":\"resources-v1\",\"tools\":[{\"name\":\"A\",\"description\":\"D\",\"tags\":[\"tag\",\"tag\"]}]}",
            "{\"resources_version\":\"resources-v1\",\"tools\":[{\"name\":\"A\",\"description\":\"D\",\"tags\":[\"tag\"]},{\"name\":\"A\",\"description\":\"D\",\"tags\":[\"tag\"]}]}",
        ] {
            let mut value = valid_workspace_json();
            value["files"][3]["content"] = serde_json::json!(content);
            assert!(DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
    }

    #[test]
    fn rejects_missing_cross_file_references_and_forbidden_browser_features() {
        for (file_index, content) in [
            (0, "<html></html>"),
            (1, "console.log('no resources');"),
            (1, "fetch(\"https://example.com\")"),
            (1, "new WebSocket('ws://example.com')"),
            (0, "<form action=\"/submit\"></form>"),
        ] {
            let mut value = valid_workspace_json();
            value["files"][file_index]["content"] = serde_json::json!(content);
            assert!(DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
    }

    #[test]
    fn rejects_confirmed_workspace_with_missing_dom_target_and_body() {
        let mut value = valid_workspace_json();
        value["files"][0]["content"] = serde_json::json!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"><script src=\"app.js\" defer></script>"
        );
        value["files"][1]["content"] = serde_json::json!(
            "fetch('resources.json').then(response => response.json()).then(data => { const toolsContainer = document.getElementById('tools-container'); data.tools.forEach(tool => { const toolDiv = document.createElement('div'); toolsContainer.appendChild(toolDiv); }); });"
        );
        let error =
            DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").unwrap_err();
        assert_eq!(error.code(), Some(MISSING_DOM_TARGET));
    }

    #[test]
    fn rejects_missing_get_element_by_id_target() {
        let mut value = valid_workspace_json();
        value["files"][1]["content"] = serde_json::json!(
            "fetch('resources.json'); document.getElementById(\"missing-target\");"
        );
        let error =
            DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").unwrap_err();
        assert_eq!(error.code(), Some(MISSING_DOM_TARGET));
    }

    #[test]
    fn rejects_duplicate_html_ids() {
        let mut value = valid_workspace_json();
        let html = value["files"][0]["content"].as_str().unwrap();
        value["files"][0]["content"] =
            serde_json::json!(html.replace("</main>", "<section id='tool-list'></section></main>"));
        let error =
            DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").unwrap_err();
        assert_eq!(error.code(), Some(DUPLICATE_DOM_ID));
    }

    #[test]
    fn accepts_matching_literal_dom_targets() {
        for access in [
            "document.getElementById(\"tool-list\")",
            "document.getElementById('tool-list')",
            "document.querySelector(\"#tool-list\")",
            "document.querySelector('#tool-list')",
        ] {
            let mut value = valid_workspace_json();
            value["files"][1]["content"] =
                serde_json::json!(format!("fetch('resources.json'); {access};"));
            assert!(
                DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").is_ok(),
                "rejected {access}"
            );
        }
    }

    #[test]
    fn rejects_asset_only_html_and_accepts_a_minimal_application_body() {
        let mut asset_only = valid_workspace_json();
        asset_only["files"][0]["content"] = serde_json::json!(
            "<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"></head><body><script src=\"app.js\" defer></script></body></html>"
        );
        let error =
            DeveloperWorkspace::parse_and_validate(&asset_only.to_string(), "task-1").unwrap_err();
        assert_eq!(error.code(), Some(MISSING_APPLICATION_BODY));

        assert!(
            DeveloperWorkspace::parse_and_validate(&valid_workspace_json().to_string(), "task-1")
                .is_ok()
        );
    }

    fn workspace_error_for(html: &str, javascript: &str) -> AppError {
        let mut value = valid_workspace_json();
        value["files"][0]["content"] = serde_json::json!(html);
        value["files"][1]["content"] = serde_json::json!(javascript);
        DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").unwrap_err()
    }

    fn valid_html_with(body: &str) -> String {
        format!(
            "<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"></head><body>{body}<script src=\"app.js\" defer></script></body></html>"
        )
    }

    #[test]
    fn rejects_multiple_id_attributes_on_one_tag_regardless_of_case() {
        for element in [
            "<div id=\"first\" id=\"second\"></div>",
            "<div ID=\"first\" id=\"second\"></div>",
        ] {
            let error = workspace_error_for(
                &valid_html_with(element),
                "fetch('resources.json'); document.getElementById('first');",
            );
            assert_eq!(error.code(), Some(DUPLICATE_DOM_ID));
        }
    }

    #[test]
    fn rejects_empty_quoted_unquoted_and_direct_lookup_ids() {
        for element in [
            "<main id=\"\"></main>",
            "<main id=''></main>",
            "<main id=></main>",
        ] {
            let error = workspace_error_for(&valid_html_with(element), "fetch('resources.json');");
            assert_eq!(error.code(), Some(MISSING_DOM_TARGET));
        }
        let error = workspace_error_for(
            &valid_html_with("<main id=\"app\"></main>"),
            "fetch('resources.json'); document.getElementById('');",
        );
        assert_eq!(error.code(), Some(MISSING_DOM_TARGET));
    }

    #[test]
    fn ignores_ids_inside_complete_html_comments() {
        for comment in [
            "<!-- <div id=\"required\"></div> -->",
            "<!-- > <div id=\"required\"></div> -->",
        ] {
            let body = format!("<main></main>{comment}");
            let error = workspace_error_for(
                &valid_html_with(&body),
                "fetch('resources.json'); document.getElementById('required');",
            );
            assert_eq!(error.code(), Some(MISSING_DOM_TARGET));
        }
    }

    #[test]
    fn rejects_unterminated_html_comments() {
        let html = valid_html_with("<main></main><!-- <div id=\"required\"></div>");
        let error = workspace_error_for(&html, "fetch('resources.json');");
        assert_eq!(error.code(), Some(MISSING_APPLICATION_BODY));
    }

    #[test]
    fn ignores_fake_ids_inside_raw_and_inert_content() {
        for content in [
            "<script><div id=\"required\"></div></script>",
            "<style><div id=\"required\"></div></style>",
            "<textarea><div id=\"required\"></div></textarea>",
            "<title><div id=\"required\"></div></title>",
            "<template><div id=\"required\"></div></template>",
            "<template><template></template><div id=\"required\"></div></template>",
        ] {
            let body = format!("<main></main>{content}");
            let error = workspace_error_for(
                &valid_html_with(&body),
                "fetch('resources.json'); document.getElementById('required');",
            );
            assert_eq!(error.code(), Some(MISSING_DOM_TARGET));
        }
    }

    #[test]
    fn inert_template_does_not_satisfy_application_body() {
        let error = workspace_error_for(
            &valid_html_with("<template><main id=\"app\"></main></template>"),
            "fetch('resources.json');",
        );
        assert_eq!(error.code(), Some(MISSING_APPLICATION_BODY));
    }

    #[test]
    fn ignores_commented_and_raw_body_like_text() {
        for html in [
            "<!-- <body><main></main></body> --><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"><script src=\"app.js\" defer></script>",
            "<style><body><main></main></body></style><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><link rel=\"stylesheet\" href=\"styles.css\"><script src=\"app.js\" defer></script>",
        ] {
            let error = workspace_error_for(html, "fetch('resources.json');");
            assert_eq!(error.code(), Some(MISSING_APPLICATION_BODY));
        }
    }

    #[test]
    fn rejects_multiple_or_unclosed_body_elements() {
        let multiple = format!(
            "{}<body><main></main></body>",
            valid_html_with("<main></main>")
        );
        assert_eq!(
            workspace_error_for(&multiple, "fetch('resources.json');").code(),
            Some(MISSING_APPLICATION_BODY)
        );

        let unclosed = valid_html_with("<main></main>").replace("</body>", "");
        assert_eq!(
            workspace_error_for(&unclosed, "fetch('resources.json');").code(),
            Some(MISSING_APPLICATION_BODY)
        );
    }

    #[test]
    fn accepts_genuine_application_markup_in_one_closed_body() {
        let html = valid_html_with("<main id=\"app\"><h1>Tools</h1></main>");
        assert!(
            DeveloperWorkspace::parse_and_validate(
                &{
                    let mut value = valid_workspace_json();
                    value["files"][0]["content"] = serde_json::json!(html);
                    value["files"][1]["content"] = serde_json::json!(
                        "fetch('resources.json'); document.getElementById('app');"
                    );
                    value.to_string()
                },
                "task-1"
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_secret_sensitive_content_bom_and_nul() {
        for content in [
            "api_key=private",
            "Authorization: Bearer value",
            "-----BEGIN PRIVATE KEY-----",
            "\u{feff}text",
            "text\0value",
        ] {
            let mut value = valid_workspace_json();
            value["files"][2]["content"] = serde_json::json!(content);
            assert!(DeveloperWorkspace::parse_and_validate(&value.to_string(), "task-1").is_err());
        }
    }
}
