# Roadmap

The roadmap advances through small vertical slices. Each phase should have explicit acceptance criteria and relevant automated tests before the next phase begins.

## Phase 1: Single Lead call

- Create one synchronous Rust binary crate with no async runtime.
- Read one bounded user request from standard input.
- Call one Lead model through one non-streaming native Ollama `/api/chat` request using `ureq`.
- Restrict the configured endpoint to explicit `localhost`, `127.0.0.1`, or `::1` hosts.
- Supply the approved static, versioned Lead JSON Schema to Ollama.
- Apply strict `serde` deserialization with unknown fields denied, followed by semantic and size validation in Rust.
- Save a versioned JSON execution report after configuration and report-directory validation succeed.
- Capture Ollama-reported token counts and durations when available, without estimation.
- Use concise standard-error lifecycle messages without recording sensitive or raw model data.
- Document a restrictive OS launch profile while keeping OS sandboxing outside the application.

No Developer, Reviewer, concurrency, subprocess, command execution, command runner, model tool, retry, remote model, remote API, Git operation, database, web UI, or application framework belongs in this phase.

Phase 1 is complete only when malformed or excessive inputs and outputs fail closed, the 1 MiB response bound and approved timeouts are enforced, report failures are distinguishable, representative contract tests pass, and one documented local Ollama smoke test produces a valid report.

## Phase 2: Deterministic local runner

- Introduce an explicit command allowlist and argument validation.
- Require human approval for classified risky actions.
- Record execution outcomes locally.
- Reduce output to failure counts and concise causes before any model receives it.

The command policy and approval model require human validation before implementation.

## Phase 3: Developer role

- Add small, Lead-defined task delegation.
- Supply only task-relevant context to the Developer.
- Let the Developer produce implementation code and related tests.
- Keep orchestration sequential and measure iteration behavior.

## Phase 4: Reviewer role

- Review the diff against the user request and acceptance criteria.
- Include summarized test results, not raw logs.
- Prefer a model different from the Developer model.
- Record detected defects and subsequent iterations.

## Phase 5: Workspace isolation

- Run changes in isolated, ephemeral Git worktrees or repositories.
- Ensure credentials and unrelated host data are unavailable.
- Make cleanup safe, deterministic, and auditable.

The isolation design requires architecture and security approval.

## Phase 6: Evaluation and controlled expansion

- Compare success rate, defects, duration, iterations, context volume, removed context, intervention time, and resource use.
- Estimate remote API cost only when remote APIs are introduced.
- Consider concurrency only after sequential correctness and isolation are demonstrated.
- Consider remote Git interaction only with an approved credential-free design and explicit human control.
