# Architecture

## Architectural direction

The orchestrator will be a Rust application. Ollama will expose local language models, while the orchestrator owns role prompts, context selection, schema validation, state transitions, reporting, and approval boundaries.

Agent communication crosses typed JSON boundaries. Model output is untrusted input: it must be parsed and validated before it can affect orchestration or execution.

The architecture supports two related product paths. `agent-factory` is the reusable local orchestration engine. `devops_hub` is its first concrete product and uses the strict approved-source path for a personal DevOps portal. DevOps taxonomy, editorial structure, news policy, and presentation belong to `devops_hub`; generic role, retrieval, provenance, validation, workspace, Runner, and Reviewer boundaries belong to `agent-factory`.

The existing eight-tool V1 registry and E0/E1 implementation remain shared-core assets: a secure reference implementation, deterministic fixture, and template for later approved registries. The six-source V2 policy is the initial live `devops_hub` catalog policy. Approved registries are not mandatory for every future research task.

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

### Approved execution contract

Phase 1 is one synchronous Rust binary crate. It uses a blocking `ureq` client to make exactly one native Ollama `POST /api/chat` request. The request is non-streaming and supplies a static, versioned JSON Schema through Ollama's structured-output `format` field. No async runtime or application framework is used.

The program reads one user request from standard input through a 64 KiB (65,536-byte) hard limit, with at most one additional byte read to detect overflow before allocating the complete input. After trimming, the request must contain between 1 and 16,000 Unicode scalar values. The byte limit is a transport and memory-safety bound; the character limit is the semantic contract. The request is sent only to the configured Lead model and is not persisted in the execution report.

The Ollama endpoint must use plain HTTP and an explicit local host of `localhost`, `127.0.0.1`, or `::1`. Other spellings and hostnames are rejected even if they resolve to a loopback address. Credentials, query strings, fragments, and redirects are rejected. An explicit port is allowed and defaults to `11434`; the application constructs the `/api/chat` path itself.

The connection timeout is two seconds. The response timeout defaults to 300 seconds and may be configured from 1 through 600 seconds. Automatic retries are disabled, and the response body is limited to 1 MiB.

### Lead response contract

The top-level Lead response contains exactly these fields:

- `summary`: a string containing 1 to 2,000 Unicode scalar values;
- `assumptions`: an array of 0 to 10 strings, each containing 1 to 1,000 Unicode scalar values;
- `acceptance_criteria`: an array of 1 to 20 strings, each containing 1 to 1,000 Unicode scalar values;
- `tasks`: an array of 1 to 20 task objects.

Each task contains exactly these fields:

- `id`: 1 to 32 ASCII characters, limited to lowercase letters, digits, and hyphens;
- `title`: a string containing 1 to 200 Unicode scalar values;
- `objective`: a string containing 1 to 2,000 Unicode scalar values;
- `acceptance_criteria`: an array of 1 to 20 strings, each containing 1 to 1,000 Unicode scalar values;
- `depends_on`: an array of 0 to 20 task IDs, using an empty array when the task has no dependency.

Empty or whitespace-only strings are invalid. Every dependency must reference another task in the same response. Self-dependencies, duplicate dependencies, and duplicate task IDs are invalid. Phase 1 does not perform full dependency-cycle detection. Task objects are planning and delegation records only; the contract contains no command, tool, patch, or executable-action field.

Rust validates the model response in layers: a static versioned JSON Schema guides Ollama structured output, strict `serde` types use `deny_unknown_fields`, and explicit Rust checks enforce semantic and size constraints. Ollama 0.32.5 rejects the valid JSON Schema keyword `maxLength` while generating its constrained-output grammar, so the schema sent to Ollama intentionally omits every `maxLength`. All approved string maxima remain enforced explicitly in Rust. The generation schema is guidance and does not define the trust boundary; Rust deserialization and semantic validation are authoritative. The first version does not add a runtime JSON Schema-validation framework. Invalid output fails closed and is not retried.

### Configuration contract

Configuration is a root-level TOML file. Phase 1 configuration is limited to the Lead model, local Ollama endpoint, response timeout, and report directory. Model names contain 1 to 200 characters, endpoint URLs are at most 2,048 characters, and report paths are at most 4,096 characters. Configuration contains no credentials or authorization headers.

### Execution report contract

Each report is versioned JSON written atomically beneath the validated report directory, using a UTC timestamp and run identifier in its filename. A report records status, timestamps, total duration, model name, schema version, validation outcome, the validated structured Lead response on success, compact failure information, input character and byte counts, and Ollama usage fields when supplied: prompt and output token counts plus reported durations.

Reports never contain the raw user request, prompts, raw HTTP or model responses, model reasoning, request or response headers, credentials, secrets, or environment dumps. Token counts come from Ollama's `prompt_eval_count` and `eval_count`; unavailable values remain absent rather than being estimated.

Configuration and report-directory validation occur before a model-call attempt begins. Failures before both validations succeed do not create a report. After they succeed, every terminal model-call path must attempt an atomic report write, including network, timeout, body-limit, parsing, and validation failures. A report-write failure is returned as a distinct error and stated concisely on standard error; persistence cannot be guaranteed across filesystem or hardware failure.

## Component boundaries

## Phase 2 V1: Lead-to-Developer proposal

V1 performs at most two sequential model calls. After a valid Lead response, trusted Rust code selects the first task in Lead-provided order whose `depends_on` array is empty. It does not choose another task after any failure. The Developer model is called at most once and both role requests set `keep_alive: 0`, so models need not remain loaded together.

The versioned `developer-request-v1` object contains `request_version` plus exactly four task fields: selected task ID, title, objective, and acceptance criteria. Its serialized JSON is limited to 32 KiB, and the complete Developer Ollama request body is limited to 64 KiB. The raw user request, Lead prompt or full output, Lead summary, global assumptions and criteria, other tasks, dependency graph, conversation, repository data, reports, logs, metrics, environment, secrets, and reasoning are not transmitted.

The strict `developer-proposal-v1` response contains a decision, matching task ID, summary, assumptions, file-change proposals, test proposals, risks, and open questions. `proposal_ready` requires a file change. `clarification_required` requires an open question and forbids file changes. File changes contain only path, `create` or `modify`, and objective; tests contain only name and objective. Contents, patches, diffs, commands, shell strings, tool calls, executable actions, deletion, and rename are outside the contract.

Proposed paths are limited to 512 ASCII characters from letters, digits, `/`, `.`, `-`, and `_`. They must be relative and have no empty, `.`, or `..` component. Components named `.git`, `.agents`, `.codex`, `reports`, or `target` are forbidden, as are `.env`, `.env.*`, `*.pem`, `*.key`, `id_rsa`, and `id_ed25519`. Duplicate paths and duplicate test names fail validation. Validation never grants filesystem access or write authorization.

Developer limits are: summary 1–2,000 characters; assumptions, risks, and open questions 0–10 strings of 1–1,000 characters; file changes and tests 0–20 items; file objective 1–2,000; test name 1–200; and test objective 1–1,000. As with the Lead schema, Ollama generation schemas omit `maxLength` for Ollama 0.32.5 compatibility. Rust semantic validation remains authoritative.

Root configuration now has separate `lead_model` and `developer_model` fields. Execution-report-v2 stores concrete Lead and Developer sections with separate status, validation, model, schema, metrics, and validated output fields; delegation metadata records the selected task ID, request byte count, request version, and the fixed transmitted-field names. A later-stage failure preserves completed earlier-stage results. Raw inputs, request bodies, prompts, raw responses, reasoning, headers, secrets, and environment dumps remain excluded.

- **Orchestrator:** controls the workflow and rejects invalid state transitions or model output.
- **Ollama adapter:** performs the narrowly scoped local model request and captures relevant usage metadata when available.
- **Protocol types and schemas:** define versioned JSON inputs and outputs for each role.
- **Validation:** treats all model output as untrusted and provides actionable, concise errors.
- **Report writer:** persists a local, machine-readable execution record without secrets or raw logs.
- **Policy and runner (later):** map approved actions to fixed command definitions; arbitrary model-generated shell execution is out of scope.
- **Workspace isolation (later):** creates ephemeral Git worktrees or repositories without providing credentials to agents.

Phase 2 V1 contains no subprocess API, shell execution, Git integration, command runner, model tools, retry mechanism, source-file access, file applier, remote API, concurrency, database, or web UI.

## Developer-generated static workspace

The next synchronous slice keeps the existing Lead selection rule and replaces the active proposal-only Developer output with `developer-workspace-v1`. Trusted Rust constructs `developer-request-v2` from exactly the selected task ID, title, objective, and acceptance criteria plus the Lead's top-level acceptance criteria. It does not transmit the raw request, full Lead response, other tasks, assumptions, summary, repository data, reports, logs, metrics, secrets, environment data, or reasoning.

The Developer returns exactly four UTF-8 text files named `index.html`, `app.js`, `styles.css`, and `resources.json`, including full contents. Each file is limited to 32 KiB and decoded content is limited to 96 KiB in total. Rust strictly deserializes and validates the complete bundle in memory before any workspace mutation. Validation covers exact filenames, resources data, cross-file references, a fixed document CSP, external-resource and active browser API restrictions, and a best-effort secret-sensitive denylist. The Ollama schema guides generation; Rust validation is authoritative and retains maximum-size enforcement omitted from the generation schema for Ollama compatibility.

V2.1 adds conservative cross-file DOM validation. A bounded scanner skips complete `<!-- ... -->` comments and the contents of `script`, `style`, `textarea`, `title`, and `template` elements. HTML must contain exactly one real, closed body with non-asset, non-inert application markup. Literal `id` attributes may be quoted or unquoted, but must be non-empty and unique; more than one `id` attribute on a start tag is rejected even when the values differ. Unterminated comments, unclosed handled raw or inert elements, malformed tags, multiple bodies, and missing body closure fail closed.

Rust recognizes direct literal `document.getElementById("id")`, `document.getElementById('id')`, `document.querySelector("#id")`, and `document.querySelector('#id')` calls and requires every referenced ID to exist exactly once. Dynamic selectors and general JavaScript behavior are not interpreted or proven; selectors outside these recognized direct forms remain outside this check. Because this is a lexical JavaScript scan, call-like text in comments or string literals can cause conservative false-positive rejection. It is a restricted validator, not a standards-complete HTML or JavaScript parser. Stable failures use `missing_dom_target`, `duplicate_dom_id`, or `missing_application_body`.

Publication creates only `workspaces/.run-<id>.staging`, writes the four files with create-new semantics, syncs them, and atomically renames the staging directory to the previously nonexistent `workspaces/run-<id>`. A failed staging write removes only the staging directory created for that run. Existing final workspaces are never overwritten or deleted. Workspace validation and publication do not grant access to any target repository and introduce no Git, shell, subprocess, retry, async, concurrency, or tool capability.

Execution-report-v3 preserves separate Lead and Developer status, validation, model, and metrics. It records generated byte count, validation failure counts, workspace publication duration and file metadata, plus `preview.status = "not_started"` and `preview.human_approval = "pending"`. It never stores generated file contents. Workspace and report publication are not a single atomic transaction: if the report fails after workspace publication, the completed workspace is kept, a distinct report error is returned, and its relative path is printed to standard error.

The preview command is `agent-factory preview --run-id <id> --port <port>`. It accepts ports 1024 through 65535, binds only to `127.0.0.1`, preloads the four regular non-symlink files from one canonical run directory, and then serves only `/`, `/index.html`, `/app.js`, `/styles.css`, and `/resources.json`. It supports only GET and HEAD, has no directory listing, subprocess, browser launch, or write path, and rejects malformed or ambiguous requests, queries, encoded paths, traversal, unknown routes, and request bodies. Responses use fixed MIME types, a restrictive CSP, `nosniff`, no-referrer, and no-store headers.

## Context and reporting

Context selection is role-specific and follows least privilege. Full conversation histories, unrelated files, and raw command logs should not be forwarded by default. Runner feedback to models is limited to useful failure counts and concise causes.

Reports should eventually capture timestamps, duration, iterations, per-role context volume, removed context, interventions, defects, resource use, success, and estimated remote cost. The schema must be versioned so reports remain interpretable as fields evolve.

## Official-source Explorer: Phase E0

E0 defines contracts and validation only. It adds no HTTP, DNS, TLS, redirect, HTML-normalization, Explorer-call, Developer-V3, external-link, or workspace-publication path. The executable continues to load the V1 Lead and Developer contracts. All existing V1 prompts and schemas remain versioned and unchanged.

Research mode is `off`, `auto`, or `required`, with configuration defaulting to `off`; a future run interface will choose the effective mode per run. Lead response V2 contains a strict tagged `research` union. A required request can name only the `devops-tools` topic, a supported approved-source policy, a bounded result count, the five fixed fields, and one approved reason code. It has no URL, domain, path, method, header, command, script, or authentication field. S2 compares the requested count with the selected validated registry rather than trusting a model-supplied fixed count.

The activation decision matrix is:

| Mode | Lead says not required | Lead says required |
|---|---|---|
| `off` | continue without research | fail `research_forbidden` |
| `auto` | continue without research | accept a semantically valid request |
| `required` | fail `required_research_missing` | accept a semantically valid request |

The repository-owned registry separates four properties: structural validity, policy completeness, pending authoritative verification, and retrieval readiness. Completeness requires the selected policy's approved fact IDs and exact display names in registry order. Retrieval readiness additionally requires verified HTTPS policy fields for every entry and whole-registry human approval. Pending entries must contain no domain or URL, preventing speculative policy data from becoming executable later. Registry source and fact IDs use 1 to 64 lowercase ASCII ID characters, display names use 1 to 100 characters, domains use at most 253 bytes, each entry has at most eight conservative 512-byte path prefixes, and canonical URLs use at most 2,048 bytes.

Explorer request V1 is trusted Rust output and is limited to 160 KiB with exactly one document containing at most 16 KiB normalized UTF-8 text, fixed trusted metadata, 64-byte conservative IDs, and 2,048-byte credential-free HTTPS URLs without queries or fragments. Each request contains one registry-owned fact ID, display name, source ID, official URL, source URL, and normalized evidence text. Rust constructs these fields in registry order; retrieved text cannot override them. Explorer response V1 is limited to 64 KiB and is exactly one object containing only a description and tags. Descriptions contain 1 to 500 Unicode scalar values, and each result has 1 to 10 unique lowercase ASCII tags of at most 32 bytes.

Rust associates semantic outputs by deterministic position and builds `fact-bundle-v1` only after validating exact count, the complete registry-ordered retrieval set, plain-text descriptions, and conservative tags. Rust supplies every fact ID, name, official URL, source URL, and successfully retrieved source ID. Canonical digest input is compact UTF-8 JSON with fixed struct-field order, registry fact order, and lexically sorted tags and source IDs. Timestamps are excluded. E0 established byte-for-byte determinism; dormant E2 now computes SHA-256 over those bytes.

Developer workspace V2 is dormant and contains exactly three untrusted files: `index.html`, `app.js`, and `styles.css`. It cannot contain `resources.json`. A later phase will have Rust serialize `resources.json` from the same validated fact bundle used to derive functional assertions, then combine the three untrusted files and one trusted data file before existing validation and atomic publication.

Execution-report-v5 preparation types contain compact research mode/status, policy, source counts and IDs, retrieval/Explorer/provenance status, metrics, fact count, and future document/bundle digest fields. E0 does not activate V5. These structures deliberately have no names, URLs, descriptions, page text, full request/response, headers, DNS data, or reasoning fields.

## Explorer E1: dormant bounded Retriever

E1 resolves a retrieval request containing only policy, fact, and source IDs against the validated repository registry. The registry supplies the canonical source URL and exact host, path-prefix, and redirect policy. Normal generation does not invoke this path; the explicit `retrieve-official` subcommand is disabled by configuration by default and makes no model call.

Each hop is parsed as a strict HTTPS URL, resolved once through the operating-system resolver with a two-second default bound, and rejected if any returned address is non-public. A custom `ureq` resolver returns only one deterministically selected validated socket address to the connector. The original hostname remains in the URI for TLS SNI and WebPKI certificate verification. A new resolver and client are constructed for every permitted redirect. Because standard blocking name resolution cannot be cancelled, the OS resolver helper thread can outlive an application timeout; its late result is never used for a connection.

The client disables ambient proxies and automatic redirects, sends GET with Rust-owned `User-Agent`, `Accept`, and `Accept-Encoding: identity` headers, and accepts only status 200 `text/html` or `text/plain` UTF-8. Defaults are 2-second DNS, 3-second connect, 15-second request, 60-second total, two redirects, 32 KiB response headers, 512 KiB transferred and decompressed bodies, and 16 KiB normalized text. Hard ceilings are 10, 10, 30, and 120 seconds, four redirects, 64 KiB headers, 1 MiB bodies, and 16 KiB normalized text.

HTML normalization uses a structural HTML5 parser, skips script, style, noscript, SVG, canvas, template, metadata, and comments, preserves deterministic block separation, decodes entities, collapses whitespace, and emits plain UTF-8. It performs no JavaScript, subresource loading, article ranking, summarization, or general boilerplate removal. Identical input bytes produce identical normalized text.

Normalized evidence is intended for future `ExplorerRequestV1`; the URL chain, status, byte counts, timestamp, and selected address family are audit metadata. IP addresses, headers, raw HTML, cookies, credentials, and proxy data are not report fields. E2 remains responsible for invoking and validating the Explorer.

The manual E1 command emits one compact diagnostic object on success or failure. It contains only the requested and final attempted URLs, approved redirect chain, status, normalized Content-Type and charset, Content-Encoding, bounded transferred byte count, IP family, stable error code, and elapsed time. It never includes full headers, response bodies, full addresses, cookies, authorization, proxy state, certificate internals, or dependency errors.

HTTP 429 maps to the stable `rate_limited` code without retrying or sleeping. The diagnostic includes `Retry-After` only when it is valid decimal seconds or an IMF-fixdate; malformed values are omitted.

## Shared research-policy architecture

Two conceptual policy families are planned:

- `approved_sources`: Rust resolves registry-owned identifiers to exact approved URLs. This is the implemented E0/E1 foundation and the initial `devops_hub` path.
- `open_discovery`: a future bounded discovery provider returns candidate URLs for general website requests. Candidates must still pass URL, DNS, SSRF, redirect, content, size, provenance, and model-output validation before use.

Open discovery is not implemented and must not reuse a model-generated URL as direct retrieval authority. It requires new versioned query, candidate, ranking, generic collection, and provenance contracts. Applicable protections shared by both policies include strict time and byte limits, proxy suppression, private-address rejection, manual redirects, bounded HTML normalization, deterministic JSON, Rust-owned trusted metadata, and validated bundles shared by Developer, Runner, and Reviewer.

## Explorer E2 / shared-core S2: dormant validated extraction

S2 implements the shared `approved_sources` Explorer path: complete source retrieval, one independent bounded Explorer extraction per registry entry, strict semantic validation, and Rust-owned `FactBundleV1`. It remains disabled by default and available only through the explicit manual command; normal V1 generation does not activate it. H0 is its first intended consumer and uses the verified six-source `official-devops-tools-v2` policy. The eight-source V1 policy remains available as history and a security reference.

The command retrieves every entry in the selected registry once in registry order. Any missing, empty, failed, or rate-limited source ends the pipeline before semantic extraction. There is no retry, partial bundle, cache, or fallback. Rust then makes at most one non-streaming local Ollama call per document in registry order. Each call uses the configured Explorer model, temperature zero, `keep_alive: 0`, no tools, and a 64 KiB response limit. The first failed call stops later calls, and earlier validated semantic results remain unpublished in memory.

The S2 boundary strictly parses one description-and-tags JSON object per call. It rejects model-produced identifier or URL fields, associates each result with that call's registry entry and completed retrieval, deterministically normalizes tags, constructs `FactBundleV1`, and produces canonical bytes and a digest. Provenance proves which retrieved document was supplied for a fact, not perfect semantic entailment of every sentence. A future policy with multiple documents per fact will require a separate trusted evidence-group contract rather than model-selected source IDs. S2 does not activate Developer V2, `resources.json`, workspace publication, or preview; those remain S3 work. `open_discovery` has no contract or runtime dependency on S2.

The implementation evolution and successful six-source H0 evidence are documented in [s2-approved-source-explorer.md](s2-approved-source-explorer.md).

## Future immutable workspace revisions

Workspace improvement will use immutable revisions rather than overwrite. A validated initial workspace becomes revision zero. Runner results and Reviewer observations may produce a structured change request, but the Reviewer has no file-write authority. The Developer receives only approved defects, relevant files, and the same validated data bundle, then proposes or applies targeted changes through trusted Rust validation.

Publishing a revision creates a new run-scoped revision directory, preserves unchanged files, records requesting and applying roles, and never mutates an earlier valid revision. Every revision reruns static and functional validation and can be rolled back by selecting an earlier immutable revision. Facts, names, and URLs remain identical to the prior validated bundle unless a separately authorized research rerun creates a new bundle version. This Reviewer and revision architecture is future S5 work and is not active.

## Deferred decisions

The internal Rust module layout, concrete Lead prompt, report-directory default, later command policy representation, isolation mechanism, and cross-phase metric definitions remain deferred. Any choice that changes an approved architecture or security boundary requires human validation before implementation.
