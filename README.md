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
schemas/    Versioned JSON schemas (empty until the protocol is defined)
```

## Status

Phase 1 V0 is a single synchronous Rust binary that calls one local Ollama Lead model, validates its structured response, and writes a local JSON execution report.

## Phase 1 V0 usage

Prerequisites are a Rust toolchain, a locally running Ollama server, and the model selected in `agent-factory.toml`. The default model is `gemma3`.

Create the report directory before running:

```text
mkdir reports
```

Then provide one request on standard input:

```text
printf 'Describe a small implementation plan.' | cargo run
```

The application reads `agent-factory.toml` from the current repository root. It accepts only explicit loopback Ollama endpoints and writes reports beneath the configured, existing report directory. These application checks do not replace the OS sandbox recommended in the security documentation.

Standard input is limited to 64 KiB before UTF-8 and semantic validation. After trimming, the request must still contain between 1 and 16,000 Unicode scalar values.
