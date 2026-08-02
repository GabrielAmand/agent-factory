# Vision

## Purpose

`agent-factory` is a local laboratory for studying how specialized software-engineering agents can collaborate safely and effectively. It prioritizes clear role boundaries, minimal context sharing, deterministic execution, and measurable outcomes over maximum autonomy.

The project is intended to make agent behavior inspectable. Each decision, delegation, validation result, command outcome, and human intervention should contribute to a local execution report without leaking unnecessary data to a model.

`agent-factory` is the generic engine, not a DevOps-content product. Its orchestration, research, artifact-validation, review, and revision boundaries should support multiple bounded product workflows. `devops_hub` is the first concrete product intended to be built with that engine: a personal DevOps knowledge and news website and a practical showcase for the strict official-source path.

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

## Trusted factual context

The dormant approved-source Explorer pipeline lets the laboratory work with bounded official information without giving models unrestricted network access. Rust-owned source registries select the evidence; an Explorer extracts only semantic fields from that evidence; Rust validates and normalizes them and creates one immutable fact bundle. A future S3 integration will feed the same bundle to Developer context, trusted `resources.json` serialization, and functional assertions so those stages cannot silently disagree about facts.

Phase E0 establishes versioned contracts and an approved eight-source DevOps registry. Phase E1 implements the dormant bounded Retriever. The first DevOps catalog remains a curated product set and a deterministic security reference; the Explorer never chooses its own approved sources and never serves as the authority for whether its output is true.

Shared-core S2 implements the dormant `approved_sources` Explorer and validated fact-bundle path. Its six-source V2 data-production flow has passed a live smoke test, making H0 the first validated consumer path; S3 still must connect the bundle to actual site generation. The verified eight-source V1 policy remains preserved as history and a security reference. This does not activate or constrain the separate future `open_discovery` policy.

## Two product paths

### `devops_hub`

`devops_hub` is a product built on `agent-factory`, not content embedded into the generic engine. Its initial live catalog uses Docker, Kubernetes, Terraform, Jenkins, GitLab CI, and Prometheus. Ansible and Argo CD can return in a later policy after their official-source rate limiting is addressed without weakening retrieval controls. Later approved registries may cover AWS, Azure, Google Cloud, GitHub Actions, Flux, Helm, Grafana, Loki, OpenTelemetry, Trivy, Gitleaks, Checkov, Semgrep, Cosign, Falco, OPA, Codex, Claude Code, Gemini CLI, and other explicitly reviewed DevOps, Cloud, DevSecOps, and AI-agent sources.

Possible sections include tools, cloud, DevSecOps, AI coding agents, official documentation, comparisons, learning resources, releases, changelogs, recent news, and security advisories. Stable catalog facts and time-sensitive news require separate contracts, provenance, freshness rules, and storage lifecycles. Scheduling, news ingestion, persistent storage, and frontend expansion are future product work, not current shared-core scope.

### Open site generation

The generic engine should eventually support requests such as finding local services or hotels, comparing products, and creating small curated directories. This cannot require every candidate domain to be present in a static registry. A future `open_discovery` policy may let a validated Lead query drive a bounded discovery provider, candidate selection, secure retrieval, Explorer extraction, a validated generic collection bundle, and downstream generation and evaluation.

That generic bundle may eventually contain stable item IDs, names, concise descriptions, primary URLs, supporting source URLs, optional tags and categories, and provenance metadata. Its exact contract, discovery provider, ranking rules, trust semantics, and evaluation criteria require a dedicated design phase. Open discovery is not active.

## Shared research policies

The approved-source registry is one research policy rather than the mandatory basis for all future research:

- `approved_sources` selects URLs from repository-owned, human-approved registries and is first exercised by `devops_hub`.
- `open_discovery` would discover candidate URLs within separately approved provider, network, ranking, and validation boundaries.

Both policies should reuse strict limits, SSRF and private-address rejection, proxy suppression, manual redirects, bounded normalization, deterministic contracts, provenance, Rust-owned metadata, and model-output validation. A Developer must not silently invent or replace validated URLs, and later Runner and Reviewer stages must consume the same validated bundle.

## Iterative workspace improvement

A future revision workflow should improve an existing generated site without destructively replacing it:

```text
initial Developer output
→ validated workspace
→ preview and functional Runner
→ Reviewer observations
→ structured approved change request
→ Developer edits
→ static and functional validation
→ new immutable workspace revision
```

The original workspace remains available. Revisions preserve unchanged files, record which role requested and applied each change, and support rollback to an earlier valid revision. The Reviewer reports defects or improvements but never edits files; the Developer applies approved targeted changes. Validated facts and URLs remain fixed across revisions unless research is explicitly rerun.
