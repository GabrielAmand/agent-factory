# agent-factory

`agent-factory` is a local multi-agent software engineering laboratory. Its goal is to explore disciplined agent collaboration while keeping execution observable, deterministic, secure, and economical with model context.

> **V0 status:** This repository is frozen as an experimental prototype and evidence base. Its Lead, Developer, Retriever, Explorer, static-site workflow, schemas, and roadmap are V0 experiments and are not presumed to define V1. See [the V0 status](docs/v0-status.md).

The repository is the generic orchestration engine. `devops_hub` is the planned first product built with it: a personal DevOps knowledge and news portal using approved official sources. A separate future open-discovery path may support general curated websites without requiring every domain to be statically registered; that path is not active.

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

## Explorer Phase E0 contracts

Phase E0 adds dormant, versioned contracts for a future official-source Explorer pipeline. It does not alter the current V1 Lead-to-Developer runtime, perform network access, or call an Explorer model. `default_research_mode = "off"` is parsed from configuration for future per-run selection; E0 does not activate research.

The repository-owned `official-devops-tools-v1` registry preserves the previously approved eight-source security reference. The initial live H0 dataset uses `official-devops-tools-v2`: Docker, Kubernetes, Terraform, Jenkins, GitLab CI, and Prometheus. Ansible and Argo CD remain in V1 but are excluded from V2 because their official pages consistently returned HTTP 429 to the bounded Retriever. “Widely used” remains a curated product decision, not a model conclusion.

Future phases will validate bounded official text, create one immutable fact bundle, give only that bundle to the Developer, and derive both trusted `resources.json` and functional assertions from it. The Explorer will extract from a Rust-supplied evidence set and will never define its own sources or oracle.

## Dormant official-source Retriever

Phase E1 adds a bounded HTTPS Retriever but does not connect it to normal generation, research activation, Explorer, fact-bundle, Developer, or workspace execution. It is disabled by default. Its manual entry point accepts registry identifiers, never a URL:

```text
cargo run -- retrieve-official --policy official-devops-tools-v1 --fact-id docker --source-id docker-official
```

Before an explicitly approved live smoke test, set `retriever.enabled = true`. The command prints compact retrieval metadata to standard error and normalized official text to standard output. It performs no Ollama call and publishes no workspace.

## Dormant official-source Explorer

Phase E2 adds a separate fail-closed command that requires both the Retriever and Explorer flags to be explicitly enabled:

```text
cargo run -- explore-official --policy official-devops-tools-v2
```

The command accepts only the policy ID. It retrieves every source in the selected validated registry once, then calls the configured local `gemma3:latest` Explorer at most once per document in deterministic registry order. Each strict response contains only one description and tag list. Rust associates it with that call's trusted registry entry, and only after every call succeeds does Rust construct `FactBundleV1`. The first failure stops later calls without retry or partial publication. The command writes no bundle, resources file, report, or workspace.

`[explorer] enabled = false` is the default. Each Explorer request is capped at 160 KiB of JSON and contains at most 16 KiB of normalized evidence; each response is capped at 64 KiB. Retrieved text is untrusted evidence data, while fact IDs, display names, source IDs, and official/source URLs are populated from the approved registry. E2 does not change the normal V1 workflow or activate Developer workspace V2.

The complete S2 evolution, trust boundary, failed batch designs, deterministic tag normalization, and successful H0 smoke evidence are recorded in [docs/s2-approved-source-explorer.md](docs/s2-approved-source-explorer.md).
