# Architecture

## Architectural direction

The orchestrator will be a Rust application. Ollama will expose local language models, while the orchestrator owns role prompts, context selection, schema validation, state transitions, reporting, and approval boundaries.

Agent communication crosses typed JSON boundaries. Model output is untrusted input: it must be parsed and validated before it can affect orchestration or execution.

## Target workflow

```text
User request
    |
    v
Lead -> acceptance criteria + small delegated tasks
    |
    v
Developer -> implementation diff + test requests
    |
    v
Deterministic runner -> concise test summary
    |
    v
Reviewer -> findings against diff and acceptance criteria
    |
    v
Report / next iteration / human approval
```

The target flow is sequential at first. Later isolation will place agent changes in ephemeral Git worktrees or repositories so that changes can be inspected and discarded safely.

## First technical version

The first executable slice contains only:

1. An input request accepted locally.
2. One call to a configured Ollama Lead model.
3. A request for a structured JSON response.
4. Rust parsing and validation against a defined contract.
5. A local execution report containing the request outcome and measurements available at this stage.

It explicitly excludes the Developer, runner command execution, Reviewer, concurrency, isolated worktrees, remote models, and remote Git operations.

### Approved execution contract

Phase 1 is one synchronous Rust binary crate. It uses a blocking `ureq` client to make exactly one native Ollama `POST /api/chat` request. The request is non-streaming and supplies a static, versioned JSON Schema through Ollama's structured-output `format` field. No async runtime or application framework is used.

The program reads one user request from standard input through a 64 KiB (65,536-byte) hard limit, with at most one additional byte read to detect overflow before allocating the complete input. After trimming, the request must contain between 1 and 16,000 Unicode scalar values. The byte limit is a transport and memory-safety bound; the character limit is the semantic contract. The request is sent only to the configured Lead model and is not persisted in the execution report.

The Ollama endpoint must use plain HTTP and an explicit local host of `localhost`, `127.0.0.1`, or `::1`. Other spellings and hostnames are rejected even if they resolve to a loopback address. Credentials, query strings, fragments, and redirects are rejected. An explicit port is allowed and defaults to `11434`; the application constructs the `/api/chat` path itself.

The connection timeout is two seconds. The response timeout defaults to 300 seconds and may be configured from 1 through 600 seconds. Automatic retries are disabled, and the response body is limited to 1 MiB.

### Lead response contract

The top-level Lead response contains exactly these fields:

- `summary`: a string containing 1 to 2,000 Unicode scalar values;
- `assumptions`: an array of 0 to 10 strings, each containing 1 to 1,000 Unicode scalar values;
- `acceptance_criteria`: an array of 1 to 20 strings, each containing 1 to 1,000 Unicode scalar values;
- `tasks`: an array of 1 to 20 task objects.

Each task contains exactly these fields:

- `id`: 1 to 32 ASCII characters, limited to lowercase letters, digits, and hyphens;
- `title`: a string containing 1 to 200 Unicode scalar values;
- `objective`: a string containing 1 to 2,000 Unicode scalar values;
- `acceptance_criteria`: an array of 1 to 20 strings, each containing 1 to 1,000 Unicode scalar values;
- `depends_on`: an array of 0 to 20 task IDs, using an empty array when the task has no dependency.

Empty or whitespace-only strings are invalid. Every dependency must reference another task in the same response. Self-dependencies, duplicate dependencies, and duplicate task IDs are invalid. Phase 1 does not perform full dependency-cycle detection. Task objects are planning and delegation records only; the contract contains no command, tool, patch, or executable-action field.

Rust validates the model response in layers: a static versioned JSON Schema guides Ollama structured output, strict `serde` types use `deny_unknown_fields`, and explicit Rust checks enforce semantic and size constraints. Ollama 0.32.5 rejects the valid JSON Schema keyword `maxLength` while generating its constrained-output grammar, so the schema sent to Ollama intentionally omits every `maxLength`. All approved string maxima remain enforced explicitly in Rust. The generation schema is guidance and does not define the trust boundary; Rust deserialization and semantic validation are authoritative. The first version does not add a runtime JSON Schema-validation framework. Invalid output fails closed and is not retried.

### Configuration contract

Configuration is a root-level TOML file. Phase 1 configuration is limited to the Lead model, local Ollama endpoint, response timeout, and report directory. Model names contain 1 to 200 characters, endpoint URLs are at most 2,048 characters, and report paths are at most 4,096 characters. Configuration contains no credentials or authorization headers.

### Execution report contract

Each report is versioned JSON written atomically beneath the validated report directory, using a UTC timestamp and run identifier in its filename. A report records status, timestamps, total duration, model name, schema version, validation outcome, the validated structured Lead response on success, compact failure information, input character and byte counts, and Ollama usage fields when supplied: prompt and output token counts plus reported durations.

Reports never contain the raw user request, prompts, raw HTTP or model responses, model reasoning, request or response headers, credentials, secrets, or environment dumps. Token counts come from Ollama's `prompt_eval_count` and `eval_count`; unavailable values remain absent rather than being estimated.

Configuration and report-directory validation occur before a model-call attempt begins. Failures before both validations succeed do not create a report. After they succeed, every terminal model-call path must attempt an atomic report write, including network, timeout, body-limit, parsing, and validation failures. A report-write failure is returned as a distinct error and stated concisely on standard error; persistence cannot be guaranteed across filesystem or hardware failure.

## Component boundaries

## Phase 2 V1: Lead-to-Developer proposal

V1 performs at most two sequential model calls. After a valid Lead response, trusted Rust code selects the first task in Lead-provided order whose `depends_on` array is empty. It does not choose another task after any failure. The Developer model is called at most once and both role requests set `keep_alive: 0`, so models need not remain loaded together.

The versioned `developer-request-v1` object contains `request_version` plus exactly four task fields: selected task ID, title, objective, and acceptance criteria. Its serialized JSON is limited to 32 KiB, and the complete Developer Ollama request body is limited to 64 KiB. The raw user request, Lead prompt or full output, Lead summary, global assumptions and criteria, other tasks, dependency graph, conversation, repository data, reports, logs, metrics, environment, secrets, and reasoning are not transmitted.

The strict `developer-proposal-v1` response contains a decision, matching task ID, summary, assumptions, file-change proposals, test proposals, risks, and open questions. `proposal_ready` requires a file change. `clarification_required` requires an open question and forbids file changes. File changes contain only path, `create` or `modify`, and objective; tests contain only name and objective. Contents, patches, diffs, commands, shell strings, tool calls, executable actions, deletion, and rename are outside the contract.

Proposed paths are limited to 512 ASCII characters from letters, digits, `/`, `.`, `-`, and `_`. They must be relative and have no empty, `.`, or `..` component. Components named `.git`, `.agents`, `.codex`, `reports`, or `target` are forbidden, as are `.env`, `.env.*`, `*.pem`, `*.key`, `id_rsa`, and `id_ed25519`. Duplicate paths and duplicate test names fail validation. Validation never grants filesystem access or write authorization.

Developer limits are: summary 1–2,000 characters; assumptions, risks, and open questions 0–10 strings of 1–1,000 characters; file changes and tests 0–20 items; file objective 1–2,000; test name 1–200; and test objective 1–1,000. As with the Lead schema, Ollama generation schemas omit `maxLength` for Ollama 0.32.5 compatibility. Rust semantic validation remains authoritative.

Root configuration now has separate `lead_model` and `developer_model` fields. Execution-report-v2 stores concrete Lead and Developer sections with separate status, validation, model, schema, metrics, and validated output fields; delegation metadata records the selected task ID, request byte count, request version, and the fixed transmitted-field names. A later-stage failure preserves completed earlier-stage results. Raw inputs, request bodies, prompts, raw responses, reasoning, headers, secrets, and environment dumps remain excluded.

- **Orchestrator:** controls the workflow and rejects invalid state transitions or model output.
- **Ollama adapter:** performs the narrowly scoped local model request and captures relevant usage metadata when available.
- **Protocol types and schemas:** define versioned JSON inputs and outputs for each role.
- **Validation:** treats all model output as untrusted and provides actionable, concise errors.
- **Report writer:** persists a local, machine-readable execution record without secrets or raw logs.
- **Policy and runner (later):** map approved actions to fixed command definitions; arbitrary model-generated shell execution is out of scope.
- **Workspace isolation (later):** creates ephemeral Git worktrees or repositories without providing credentials to agents.

Phase 2 V1 contains no subprocess API, shell execution, Git integration, command runner, model tools, retry mechanism, source-file access, file applier, remote API, concurrency, database, or web UI.

## Developer-generated static workspace

The next synchronous slice keeps the existing Lead selection rule and replaces the active proposal-only Developer output with `developer-workspace-v1`. Trusted Rust constructs `developer-request-v2` from exactly the selected task ID, title, objective, and acceptance criteria plus the Lead's top-level acceptance criteria. It does not transmit the raw request, full Lead response, other tasks, assumptions, summary, repository data, reports, logs, metrics, secrets, environment data, or reasoning.

The Developer returns exactly four UTF-8 text files named `index.html`, `app.js`, `styles.css`, and `resources.json`, including full contents. Each file is limited to 32 KiB and decoded content is limited to 96 KiB in total. Rust strictly deserializes and validates the complete bundle in memory before any workspace mutation. Validation covers exact filenames, resources data, cross-file references, a fixed document CSP, external-resource and active browser API restrictions, and a best-effort secret-sensitive denylist. The Ollama schema guides generation; Rust validation is authoritative and retains maximum-size enforcement omitted from the generation schema for Ollama compatibility.

V2.1 adds conservative cross-file DOM validation. A bounded scanner skips complete `<!-- ... -->` comments and the contents of `script`, `style`, `textarea`, `title`, and `template` elements. HTML must contain exactly one real, closed body with non-asset, non-inert application markup. Literal `id` attributes may be quoted or unquoted, but must be non-empty and unique; more than one `id` attribute on a start tag is rejected even when the values differ. Unterminated comments, unclosed handled raw or inert elements, malformed tags, multiple bodies, and missing body closure fail closed.

Rust recognizes direct literal `document.getElementById("id")`, `document.getElementById('id')`, `document.querySelector("#id")`, and `document.querySelector('#id')` calls and requires every referenced ID to exist exactly once. Dynamic selectors and general JavaScript behavior are not interpreted or proven; selectors outside these recognized direct forms remain outside this check. Because this is a lexical JavaScript scan, call-like text in comments or string literals can cause conservative false-positive rejection. It is a restricted validator, not a standards-complete HTML or JavaScript parser. Stable failures use `missing_dom_target`, `duplicate_dom_id`, or `missing_application_body`.

Publication creates only `workspaces/.run-<id>.staging`, writes the four files with create-new semantics, syncs them, and atomically renames the staging directory to the previously nonexistent `workspaces/run-<id>`. A failed staging write removes only the staging directory created for that run. Existing final workspaces are never overwritten or deleted. Workspace validation and publication do not grant access to any target repository and introduce no Git, shell, subprocess, retry, async, concurrency, or tool capability.

Execution-report-v3 preserves separate Lead and Developer status, validation, model, and metrics. It records generated byte count, validation failure counts, workspace publication duration and file metadata, plus `preview.status = "not_started"` and `preview.human_approval = "pending"`. It never stores generated file contents. Workspace and report publication are not a single atomic transaction: if the report fails after workspace publication, the completed workspace is kept, a distinct report error is returned, and its relative path is printed to standard error.

The preview command is `agent-factory preview --run-id <id> --port <port>`. It accepts ports 1024 through 65535, binds only to `127.0.0.1`, preloads the four regular non-symlink files from one canonical run directory, and then serves only `/`, `/index.html`, `/app.js`, `/styles.css`, and `/resources.json`. It supports only GET and HEAD, has no directory listing, subprocess, browser launch, or write path, and rejects malformed or ambiguous requests, queries, encoded paths, traversal, unknown routes, and request bodies. Responses use fixed MIME types, a restrictive CSP, `nosniff`, no-referrer, and no-store headers.

## Context and reporting

Context selection is role-specific and follows least privilege. Full conversation histories, unrelated files, and raw command logs should not be forwarded by default. Runner feedback to models is limited to useful failure counts and concise causes.

Reports should eventually capture timestamps, duration, iterations, per-role context volume, removed context, interventions, defects, resource use, success, and estimated remote cost. The schema must be versioned so reports remain interpretable as fields evolve.

## Deferred decisions

The internal Rust module layout, concrete Lead prompt, report-directory default, later command policy representation, isolation mechanism, and cross-phase metric definitions remain deferred. Any choice that changes an approved architecture or security boundary requires human validation before implementation.
