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

## Phase 2: Lead-to-Developer proposal

- Add exactly one configurable Developer model after the Lead.
- Select the first dependency-free Lead task and transmit only its ID, title, objective, and acceptance criteria.
- Validate a strict proposal containing create/modify path objectives and test objectives, without file contents, patches, commands, source access, or workspace changes.
- Keep calls sequential, unload each role model after its call, and report role metrics separately.

Phase 2 is complete when both role contracts and payload limits fail closed, later failures preserve earlier results in execution-report-v2, and a local two-model smoke test succeeds.

## Phase 3: Deterministic local runner and application design

Before a command runner, generate one fixed four-file static website bundle and publish it through trusted Rust to a new isolated run workspace. Validate all contents before mutation, publish through a staging-directory rename, preserve existing workspaces, and provide a read-only `127.0.0.1` preview server. Store only artifact metadata in execution-report-v3. This slice adds no command execution, target-repository access, Git, Reviewer, retry, async, or concurrency.

After the workspace slice:

- Design a deterministic file applier before allowing proposal application.
- Introduce an explicit command allowlist and argument validation only after separate approval.
- Require human approval for classified risky actions.
- Reduce output to failure counts and concise causes before any model receives it.

The file-application, command-policy, and approval models require human validation before implementation.

## Phase 4: Reviewer role

- Review the diff against the user request and acceptance criteria.
- Include summarized test results, not raw logs.
- Prefer a model different from the Developer model.
- Record detected defects and subsequent iterations.

## Explorer E0: official-source contracts without execution

- Preserve the active V1 runtime while adding Lead V2, Explorer V1, fact-bundle V1, and Developer workspace V2 contracts.
- Add a repository-owned eight-item DevOps registry whose network fields remain pending authoritative verification.
- Validate the research activation matrix, registry readiness states, Explorer boundaries, provenance, fact invariants, and canonical serialization with deterministic fixtures.
- Prepare privacy-minimized execution-report-v5 types without activating them.
- Add no network access, source retrieval, Explorer model call, or Developer behavior change.

Before E1, humans must verify and approve every canonical domain, path prefix, official URL, documentation URL, and redirect rule. E1 then requires separate approval for the HTTP/TLS dependency set, DNS and proxy boundary, retrieval limits, and live-network smoke-test procedure.

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
