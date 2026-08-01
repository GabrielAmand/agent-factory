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

## Component boundaries

- **Orchestrator:** controls the workflow and rejects invalid state transitions or model output.
- **Ollama adapter:** performs the narrowly scoped local model request and captures relevant usage metadata when available.
- **Protocol types and schemas:** define versioned JSON inputs and outputs for each role.
- **Validation:** treats all model output as untrusted and provides actionable, concise errors.
- **Report writer:** persists a local, machine-readable execution record without secrets or raw logs.
- **Policy and runner (later):** map approved actions to fixed command definitions; arbitrary model-generated shell execution is out of scope.
- **Workspace isolation (later):** creates ephemeral Git worktrees or repositories without providing credentials to agents.

## Context and reporting

Context selection is role-specific and follows least privilege. Full conversation histories, unrelated files, and raw command logs should not be forwarded by default. Runner feedback to models is limited to useful failure counts and concise causes.

Reports should eventually capture timestamps, duration, iterations, per-role context volume, removed context, interventions, defects, resource use, success, and estimated remote cost. The schema must be versioned so reports remain interpretable as fields evolve.

## Deferred decisions

The exact Rust crate layout, JSON schema vocabulary, Ollama endpoint configuration, report format and storage path, command policy representation, isolation mechanism, and metric definitions require human validation before implementation commits to them.

