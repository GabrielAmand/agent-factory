# Roadmap

The roadmap advances through small vertical slices. Each phase should have explicit acceptance criteria and relevant automated tests before the next phase begins.

## Phase 1: Single Lead call

- Define the minimal Lead response contract.
- Call one local Ollama Lead model.
- Request structured JSON output.
- Parse and validate the response in Rust.
- Save a local execution report.
- Capture the measurements available from this single call.

No Developer, Reviewer, concurrency, command execution, remote model, or remote Git interaction belongs in this phase.

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

