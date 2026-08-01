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

See [the vision](docs/vision.md), [architecture](docs/architecture.md), [security model](docs/security.md), and [roadmap](docs/roadmap.md) for the project boundaries and planned evolution.

## Repository layout

```text
docs/       Project design and policy documentation
schemas/    Versioned JSON schemas (empty until the protocol is defined)
```

## Status

The repository is at the documentation stage. No orchestrator implementation or dependency setup is included yet.

