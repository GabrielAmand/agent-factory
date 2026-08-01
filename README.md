# agent-factory

`agent-factory` is a local multi-agent software engineering laboratory. Its goal is to explore disciplined agent collaboration while keeping execution observable, deterministic, secure, and economical with model context.

The orchestrator will be written in Rust, and Ollama will provide local models. Agents exchange validated, structured JSON objects rather than free-form protocol messages.

## Intended workflow

The target workflow separates responsibilities:

1. A Lead understands the user request, defines acceptance criteria, and delegates small tasks. It must not write implementation code directly.
2. A Developer implements code and implementation-related tests.
3. A deterministic runner executes only explicitly allowed commands.
4. A Reviewer, ideally using a different model, reviews the diff, acceptance criteria, and summarized test results.

Execution will initially be sequential. Later versions may isolate agent work in ephemeral Git worktrees or repositories.

## First technical version

The first version is deliberately small:

- call one Ollama Lead model;
- request a structured JSON response;
- validate that response in Rust;
- save a local execution report;
- do not introduce a Developer or Reviewer agent;
- do not add concurrency or remote Git interaction.

Phase 1 is a single synchronous Rust binary. It sends one non-streaming request to an explicitly configured local Ollama endpoint, validates the Lead response against a versioned contract, and writes a versioned JSON report. It has no subprocess, Git, tool, retry, remote API, database, or web UI capability.

The detailed Phase 1 contract and limits are defined in [the architecture](docs/architecture.md). Application checks narrow normal program behavior, but they are not a substitute for the restrictive OS launch profile described in [the security model](docs/security.md).

See [the vision](docs/vision.md), [architecture](docs/architecture.md), [security model](docs/security.md), and [roadmap](docs/roadmap.md) for the project boundaries and planned evolution.

## Repository layout

```text
docs/       Project design and policy documentation
prompts/    Versioned role prompts
schemas/    Versioned structured-output schemas
workspaces/ Generated run workspaces (ignored except for .gitkeep)
```

## Status

The current synchronous workflow calls one Lead and at most one Developer. The Developer returns a validated four-file static website bundle, and trusted Rust publishes it to a new isolated run directory. Neither model can inspect project files, invoke tools, or choose filesystem operations.

## Workspace-generation usage

Prerequisites are a Rust toolchain, a locally running Ollama server, and both models selected in `agent-factory.toml`. The defaults are `gemma3:latest` and `qwen2.5-coder:7b`.

Create the approved runtime directories before running:

```text
mkdir reports
mkdir workspaces
```

Then provide one request on standard input:

```text
printf 'Create a minimal static website.' | cargo run
```

The application reads `agent-factory.toml` from the current repository root. It accepts only explicit loopback Ollama endpoints, writes reports beneath `reports`, and publishes generated sites beneath `workspaces`. These application checks do not replace the OS sandbox recommended in the security documentation.

Standard input is limited to 64 KiB before UTF-8 and semantic validation. After trimming, the request must still contain between 1 and 16,000 Unicode scalar values.

The program selects the first Lead task whose `depends_on` is empty. Developer request V2 contains only that task's ID, title, objective, and acceptance criteria plus the Lead's top-level acceptance criteria. After validating all four generated files in memory, Rust publishes them atomically to `workspaces/run-<id>` and records metadata—not file contents—in execution-report-v3.

Preview a completed run with a non-privileged port:

```text
cargo run -- preview --run-id <id> --port 8080
```

The synchronous preview server binds only to `127.0.0.1`, serves only the four generated assets through five fixed GET/HEAD routes, and does not open a browser.
