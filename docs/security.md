# Security

## Security model

Models and their outputs are untrusted. The orchestrator is the enforcement boundary: prompts may describe policy, but only deterministic code can enforce it.

## Core controls

- Give every agent only the minimum request, files, diff, criteria, and summaries needed for its role.
- Never provide Git credentials, SSH keys, cloud credentials, kubeconfig files, environment secrets, or other authentication material to agents or models.
- Do not include raw logs in model context. Convert runner output into useful failure counts and concise causes, with sensitive values removed.
- Validate every structured model response before using it.
- Execute only commands represented by an explicit allowlist and fixed policy.
- Treat paths and command arguments as untrusted data and constrain them to the active isolated workspace.
- Require human approval for risky actions, privilege changes, destructive operations, network access, policy changes, or scope expansion.
- Preserve an audit trail of approvals and important orchestration decisions without recording secrets.

For Phase 2 V1, the program accepts no credentials, authorization headers, `.env` loading, or secret configuration fields. Users must not place secrets in requests. The application cannot reliably recognize every secret embedded in ordinary text, so it does not persist the raw request, Developer request body, prompts, raw model responses, model reasoning, headers, or environment data.

V1 has no subprocess, shell, Git, command-runner, file-applier, source-file access, or model-tool capability. Its only network operations are one Lead call and at most one Developer call to the same explicitly configured `localhost`, `127.0.0.1`, or `::1` Ollama endpoint. It rejects redirects, other hosts, remote APIs, URL credentials, query strings, and fragments. Each response is bounded to 1 MiB, uses a two-second connection timeout and a configurable 1-to-600-second response timeout with a 300-second default, and is never retried automatically.

The Developer receives only the selected task ID, title, objective, and acceptance criteria. Proposed paths pass a conservative lexical policy that excludes internal control directories, report/build directories, traversal, and common secret-sensitive names. A valid path remains data in a report: it neither authorizes nor causes a filesystem read or write.

The workspace-generation slice additionally sends the Lead's top-level acceptance criteria. The Developer can return contents only for four fixed filenames. Rust validates the entire bundle before creating a staging directory and is the only component that writes generated files. Per-file and total byte limits, fixed filenames, cross-file checks, browser-network restrictions, and secret-sensitive patterns fail closed. Secret-pattern detection is defense in depth only: arbitrary secrets embedded in text cannot be detected reliably, so this check is not a complete security boundary and users must not submit secrets.

Generated sites may make same-origin localhost requests for their four assets. External URLs, CDNs, remote APIs, external media, WebSockets, EventSource, beacons, and forms are forbidden by validation and the fixed CSP. Preview binds only to `127.0.0.1`, preloads one canonical run directory, serves five fixed routes read-only, and cannot browse the filesystem. These application controls do not replace OS-enforced filesystem and network isolation.

Cross-file DOM checks reject empty or duplicate literal HTML IDs, multiple `id` attributes on one element, missing application body markup, and absent targets for recognized direct literal DOM lookups. The scanner ignores complete HTML comments and content inside `script`, `style`, `textarea`, `title`, and `template`; template contents are treated as inert. Unterminated comments, malformed tags, unclosed handled raw or inert elements, multiple bodies, and missing body closure fail closed.

This deliberately small scanner is not a browser parser and does not execute JavaScript. It cannot prove dynamic, computed, aliased, escaped, compound, or otherwise indirect selectors. Such selectors are not rejected unless they also contain a recognized required direct lookup. Conversely, direct-call text inside a JavaScript comment or string can be recognized and conservatively rejected as though it were executable. Browser-level validation remains a later deterministic testing concern.

## Role isolation

The Lead cannot implement code. The Developer cannot authorize commands. The runner is deterministic and cannot expand its own allowlist. The Reviewer reports findings but cannot silently alter the reviewed change.

Later versions should execute agent work in isolated, ephemeral Git worktrees or repositories. Isolation must not copy credential helpers, SSH configuration, tokens, kubeconfig files, or unrelated host data into the workspace.

## Data minimization

Before each model call, the orchestrator should construct context specifically for that role. It should measure both transmitted context and context deliberately removed. Stored reports should contain compact summaries and metrics, not unrestricted prompts, source snapshots, raw process output, or secret-bearing environment data.

## Application restrictions and OS sandboxing

Application-level restrictions constrain intended program behavior. Phase 1 validates configuration, permits only explicit local endpoint spellings, constrains report paths, bounds inputs and responses, and contains no code path for process execution. These checks reduce mistakes and misuse but do not contain a compromised binary or dependency.

OS-enforced sandboxing is a separate deployment boundary. Filesystem permissions, process restrictions, network policy, resource limits, and isolation must be applied by the operating system or a trusted sandbox. The Rust application must not claim that its validation rules provide OS-level isolation.

### Recommended restrictive launch profile

Run V1 with a profile that:

- uses a dedicated, unprivileged user with no administrative rights;
- exposes no Git credentials, SSH keys, cloud credentials, kubeconfig files, credential helpers, agent sockets, or unrelated home-directory content;
- mounts or permits the executable, configuration, prompt, and schema as read-only;
- grants write access only to the validated report and workspace directories and necessary OS temporary storage;
- denies filesystem traversal outside explicitly mounted or allowed paths;
- permits network connections only to the selected loopback Ollama port and denies remote network access;
- denies subprocess creation and prevents gaining additional privileges;
- supplies a minimal allowlisted environment without proxy or credential variables;
- applies finite memory, CPU time, file-size, open-file, and process-count limits.

The exact sandbox mechanism is platform-specific and remains a deployment decision. Phase 1 documents this profile but does not implement a sandbox itself.

## Failure behavior

Invalid JSON, schema violations, disallowed commands, unexpected paths, missing approval, and possible secret exposure must fail closed. Errors should be locally diagnosable while the model receives only the smallest safe summary needed to revise its response.

Phase 1 emits only concise lifecycle and error messages to standard error. It uses no logging framework and never logs prompts, user requests, model responses, reasoning, headers, secrets, or environment dumps.

## Human validation boundaries

Human validation is required before adopting or materially changing:

- the command allowlist and approval policy;
- credential and secret-detection rules;
- sandbox and workspace isolation boundaries;
- network or remote Git capabilities;
- retention and redaction policy for reports;
- any architecture choice that changes a trust boundary.

## Dormant E0 research boundary

Phase E0 creates data contracts but no network or Explorer execution path. The Lead and Developer remain offline except for their existing loopback Ollama calls, and the current runtime still uses only V1 contracts. A model cannot supply a URL, domain, path, method, header, command, script, or authentication value through Lead V2 research fields.

An official-source registry is trusted repository policy, never model output. Pending entries must omit all network locations. Structural validity is not authority: retrieval remains blocked until every entry in the selected policy is verified and the complete registry receives explicit human approval. V1 preserves eight verified sources; the H0 V2 policy selects six and excludes the two consistently rate-limited sources without weakening network policy. The phrase “widely used” represents a curated product choice and is not asserted as a fact established by the Explorer.

The Explorer is not its own oracle. In the dormant approved-source S2 contract it cannot return or select IDs, names, URLs, sources, domains, policy metadata, or provenance. Each independent call returns only one description and tag list for one trusted input document. Fact-bundle construction requires every registry entry to succeed and rejects unknown fields, HTML, Markdown markers, scripts, common command-like content, multiline descriptions, and invalid tags. Plain-text and command-sensitive detection is conservative defense in depth, not proof that arbitrary natural language is harmless or that all possible commands and file-like contents can be recognized.

Canonical fact-bundle bytes contain only validated facts and are SHA-256 input in dormant E2; they are not written to reports. Report-V5 preparation stores only compact counts, source IDs, statuses, metrics, and digests. It has no fields for page content, normalized evidence, fact descriptions, names, URLs, headers, DNS values, credentials, full model messages, or reasoning.

## Dormant E1 retrieval boundary

The Retriever accepts registry identifiers only. It rejects arbitrary URLs, credentials, non-HTTPS schemes, explicit ports, fragments, queries, IP literals, non-ASCII host ambiguity, percent-encoded path bypasses, and hosts or paths outside exact registry policy. Redirects are manual, loop-checked, revalidated, and re-resolved; the current registry permits only same-approved-host redirects.

Every DNS answer must be public. Mixed public/forbidden answer sets fail closed. IPv4 and IPv6 loopback, unspecified, private or unique-local, link-local, multicast, carrier-grade NAT, documentation, benchmark, reserved, IPv4-mapped special, and other enumerated special-purpose ranges are rejected. The selected public address is pinned into the HTTP connector while TLS verifies the original hostname. This is application SSRF defense, not an OS egress sandbox.

`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and lowercase variants are not consulted because every E1 client explicitly sets its proxy to `None`. System proxy auto-discovery is not used by this rustls client. Automatic redirects and decompression are disabled. Encoded responses, missing or unsupported content types, non-UTF-8 charsets, excessive headers or bodies, and non-200 final statuses fail closed. No raw page, header set, DNS answer, cookie, credential, or environment value is persisted.

Regular E1 tests use injected deterministic responses and address sets; they do not contact public hosts or weaken production SSRF checks. Live official-source smoke tests are separate human-approved operations because source movement, DNS, certificates, and redirects are mutable external state.

Live failure diagnostics are an explicit allowlist rather than a dependency error dump. URLs are registry-derived or already policy-approved, IP data is reduced to `ipv4` or `ipv6`, header data is limited to normalized Content-Type, charset, and Content-Encoding values, and transferred content is represented only by its byte count. Normalized or raw response bodies are not part of failure diagnostics.

Rate limiting remains fail closed. A 429 response becomes `rate_limited`; valid `Retry-After` seconds or an IMF-fixdate may be reported, but the Retriever never sleeps, retries, rotates identity, accepts cookies, or changes its fixed request identity automatically.

## Dormant E2 Explorer boundary

E2 is reachable only through its explicit command and requires two independent disabled-by-default configuration flags. Normal V1 generation is unchanged. It retrieves the complete fixed registry set exactly once and never invokes the Explorer after a failed or rate-limited source.

Normalized official documents are untrusted evidence. Rust serializes them as JSON data alongside registry-owned IDs, names, and exact URLs; documents cannot supply or replace those fields. The fixed prompt labels evidence as non-instructional and rejects embedded role changes, commands, secret requests, and output-format changes. This separation reduces prompt-injection authority but cannot guarantee that a language model will never be influenced by adversarial prose. Strict response and semantic validation therefore remains the authoritative boundary.

The Explorer receives no filesystem paths, credentials, environment data, headers, raw HTML, tools, shell, or network access. Each loopback Ollama request is non-streaming, uses `keep_alive: 0`, temperature zero, a configured context ceiling, one attempt, and a 64 KiB response ceiling. Rust makes at most one call per document and stops on the first failure without retry or repair. Raw evidence, prompts, descriptions, tags, and unvalidated output are not diagnostic fields. Rust-owned call order confirms which successfully retrieved registered document was supplied for each fact; it does not prove perfect factual entailment. Future policies with multiple documents per fact require a trusted evidence-group contract rather than model-selected source IDs.

Only Rust can construct an approved bundle. Canonical serialization contains validated facts in registry order with sorted tags and source IDs, then SHA-256 hashes those bytes without timestamps or run identifiers. E2 does not persist generated resources or publish a workspace.

Rust also normalizes model-produced tags before acceptance: ASCII whitespace becomes hyphens, ASCII uppercase becomes lowercase, unsupported ASCII punctuation is removed, repeated and edge hyphens are collapsed, and length and uniqueness are checked afterward. Non-ASCII tags fail closed. Only normalized tags reach the bundle; description validation is unchanged. The failed batch and tag-format experiments and final successful evidence are recorded in [s2-approved-source-explorer.md](s2-approved-source-explorer.md).
