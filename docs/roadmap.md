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
- Preserve the repository-owned `official-devops-tools-v1` registry as the approved, retrieval-ready eight-source history and security reference. Add `official-devops-tools-v2` as the six-source initial H0 live policy; Ansible and Argo CD remain verified in V1 but are excluded from V2 because their official sources consistently returned HTTP 429.
- Validate the research activation matrix, registry readiness states, Explorer boundaries, provenance, fact invariants, and canonical serialization with deterministic fixtures.
- Prepare privacy-minimized execution-report-v5 types without activating them.
- Add no network access, source retrieval, Explorer model call, or Developer behavior change.

At E0 completion, registry readiness was policy readiness only and no HTTP Retriever was present. E1 adds only an explicit, disabled-by-default manual retrieval path; no normal generation or model workflow invokes it. If an official source moves or violates its registered exact-host, path-prefix, or redirect policy, E1 fails closed rather than broadening policy automatically.

## Explorer E1: bounded retrieval without activation

- Add an explicit, configuration-disabled HTTPS Retriever driven only by approved registry identifiers.
- Pin validated public DNS answers, suppress proxies, handle redirects manually, and enforce exact registry host/path policy.
- Accept bounded UTF-8 HTML and plain text and normalize it deterministically without JavaScript or subresource loading.
- Keep normal Lead/Developer behavior unchanged and defer Explorer invocation, fact-bundle construction, and workspace integration to E2 or later.

E1 is complete when offline deterministic tests cover policy, SSRF, redirects, content and size limits, normalization, and hostile proxy environments. Live checks against official sources remain separately approved smoke tests and may fail closed when external policy assumptions change.

## Revised product roadmap

The roadmap now separates shared orchestration capabilities from the two products that consume them. Completed E0/E1 work is retained and renamed within the shared sequence; this framing does not reopen or invalidate its contracts or security decisions.

### Shared core

- **S0 — contracts and approved-source policy (complete):** existing E0 research contracts, deterministic fixtures, registry validation, historical eight-source V1, and live six-source V2.
- **S1 — bounded official Retriever (complete):** existing E1 HTTPS retrieval, DNS pinning, SSRF protection, proxy suppression, manual redirects, bounded normalization, and dormant identifier-only command.
- **S2 — approved-source Explorer and fact bundle (complete, live-smoke-tested, dormant):** complete retrieval set, one bounded single-document Explorer call per registry entry, strict semantic validation, deterministic Rust tag normalization, Rust-owned call-order provenance and immutable bundle metadata, canonical serialization, and digest. It remains disabled by default and separate from normal V1 generation.
- **S3 — trusted bundle consumption:** Rust serializes data resources from the validated bundle; Developer receives the same immutable facts and cannot replace IDs, names, or URLs.
- **S4 — functional Runner:** derive deterministic assertions from the validated bundle, exercise the published artifact, and record bounded outcomes without replacing artifact validation.
- **S5 — Reviewer and immutable revisions:** Reviewer produces structured observations; Developer applies approved targeted edits into new workspace revisions; rerun validation and support rollback.

### `devops_hub`

- **H0 — initial approved DevOps catalog data path (validated):** the six-tool V2 source-to-FactBundle path has passed its live smoke test; S3 still must connect that trusted bundle to generation before the catalog website is produced.
- **H1 — portal sections:** add DevOps-specific tools, cloud, DevSecOps, AI-agent, documentation, comparison, and learning sections without coupling these taxonomies to the engine.
- **H2 — expanded approved registries:** add separately reviewed Cloud, DevSecOps, observability, delivery, and AI-agent sources.
- **H3 — releases and news:** design distinct changelog, release, engineering-blog, security-advisory, GitHub-release, and selected-social source policies with publication/retrieval dates, freshness, and deduplication.
- **H4 — controlled refresh and publication:** add manual or scheduled refresh, archive policy, human publication approval, and storage only after separate architecture and security review.

Stable catalog data and time-sensitive news remain separate throughout H0–H4.

### Open research and site generation

- **O0 — generic collection contracts:** define validated research-query, candidate, generic item, URL, source, and provenance contracts.
- **O1 — bounded discovery provider:** select a provider and enforce query, result, network, privacy, and cost boundaries without granting models direct network access.
- **O2 — candidate ranking and retrieval:** deterministically validate and rank candidates, then reuse shared secure retrieval controls.
- **O3 — generic Explorer bundle:** extract and validate a general collection without requiring a static domain registry.
- **O4 — Developer and Runner integration:** give the same validated collection to generation and deterministic evaluation.
- **O5 — safety and quality evaluation:** measure factual coverage, provenance, locality, freshness, invented URLs, unsafe destinations, and output usefulness.

No O-phase contract or runtime is active.

## Completed dormant Explorer E2 details

- Require one complete selected-policy E1 retrieval set before any Explorer call.
- Invoke the configured local Explorer once per registry entry with bounded single-document JSON evidence and `keep_alive: 0`.
- Strictly validate and normalize semantic fields before Rust constructs `fact-bundle-v1` with trusted metadata and provenance.
- Canonically serialize the validated bundle and compute its deterministic SHA-256 digest.
- Keep E2 disabled by default and available only through an explicit manual command.
- Preserve the normal V1 workflow; do not activate Developer workspace V2, Rust-owned `resources.json`, functional assertions, or workspace publication.

Shared-core S3 remains responsible for giving only the validated fact bundle to Developer V2 and producing `resources.json` from trusted Rust data.

## Future product direction: DevOps knowledge portal

`agent-factory` and a future DevOps knowledge site have different responsibilities and must remain separate products.

`agent-factory` is the generic local multi-agent engine. It generates, validates, tests, and previews bounded artifacts through reusable role contracts and trusted enforcement boundaries. It should not embed the subject matter, information architecture, editorial policy, or presentation choices of one generated site.

The future DevOps knowledge site would be a concrete personal portal built using `agent-factory`, not a feature folded into the generic engine. Its possible sections include:

- DevOps tools;
- cloud platforms and managed services;
- DevSecOps tools and practices;
- AI coding agents;
- official documentation links;
- comparisons;
- learning resources;
- technology news and recent releases.

Two Explorer modes may eventually support that portal:

### Catalog Explorer

- Retrieves stable facts only from approved official project or vendor documentation.
- Produces validated, sourced fact bundles for tool, cloud, security, and agent catalogs.
- Favors exact official URLs, conservative classifications, and metadata expected to remain stable.
- Keeps source provenance attached to every accepted fact.

### News Explorer

- Retrieves recent announcements only from approved official news, release, changelog, engineering-blog, and security-advisory sources.
- Records both publication date and retrieval date.
- Deduplicates repeated or syndicated announcements.
- Distinguishes official announcements from secondary coverage.
- Produces short sourced summaries rather than copied articles.
- Never treats old content as new merely because it was retrieved recently.

Future product design must consider approved news-source registries, manual or scheduled refreshes, explicit freshness windows, duplicate detection, archived entries, version and release tracking, security-advisory prioritization, end-to-end source provenance, and human approval before publication. Stable catalog data and time-sensitive news data require separate contracts, storage lifecycles, freshness semantics, and presentation rules.

This direction does not broaden S0/E0, S1/E1, or the completed dormant S2 path. The current priority is S3: connect the validated H0 FactBundle to trusted Rust-owned data serialization and bounded Developer generation. No scheduler, news retriever, database, frontend redesign, open-discovery provider, revision system, or additional agent should be implemented now.

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
