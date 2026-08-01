# Vision

## Purpose

`agent-factory` is a local laboratory for studying how specialized software-engineering agents can collaborate safely and effectively. It prioritizes clear role boundaries, minimal context sharing, deterministic execution, and measurable outcomes over maximum autonomy.

The project is intended to make agent behavior inspectable. Each decision, delegation, validation result, command outcome, and human intervention should contribute to a local execution report without leaking unnecessary data to a model.

## Principles

- Local-first: Ollama runs the models, and the initial system has no remote Git interaction.
- Structured communication: agents exchange JSON objects governed by explicit schemas.
- Least context: each role receives only what it needs for the current task.
- Separation of duties: planning, implementation, execution, and review are distinct responsibilities.
- Deterministic execution: models propose work, while an allowlisted runner controls commands.
- Human authority: risky operations and architecture or security decisions require approval.
- Measurability: improvement is driven by captured outcomes rather than anecdotal impressions.
- Incremental growth: sequential execution and a single role come before multi-agent orchestration and isolation.

## Target roles

### Lead

The Lead interprets the user request, identifies assumptions, defines acceptance criteria, and decomposes work into small tasks. It does not directly write implementation code.

### Developer

The Developer implements an assigned task and its implementation-related tests within the supplied scope and context.

Phase 2 V1 deliberately stops before implementation: its Developer receives one minimal task context and returns only a validated proposal of file and test objectives. It cannot inspect source files, emit contents or patches, or modify a workspace. Those capabilities require a later, separately approved deterministic application boundary.

The static-workspace slice lets the Developer generate only four fixed website files. Trusted Rust, not the model, validates and atomically publishes them in a new run workspace. This is a constrained artifact-generation experiment, not general source access or implementation authority.

### Runner

The runner is deterministic rather than agentic. It executes only commands explicitly permitted by policy and returns a compact result summary.

### Reviewer

The Reviewer checks the diff against the request and acceptance criteria, using concise test summaries. A different model should be used where practical to reduce correlated errors.

## Measures of success

The system should record:

- tokens or context volume per role;
- number of iterations;
- total duration;
- success rate;
- context removed before each model call;
- human intervention time;
- detected defects;
- resource usage;
- estimated remote API cost when a remote API is applicable.

These measurements should be useful for comparing workflows without encouraging agents to optimize a metric at the expense of correctness or safety.
