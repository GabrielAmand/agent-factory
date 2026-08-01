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

For Phase 1, the program accepts no credentials, authorization headers, `.env` loading, or secret configuration fields. Users must not place secrets in requests. The application cannot reliably recognize every secret embedded in ordinary text, so it does not persist the raw request, prompts, raw model response, model reasoning, headers, or environment data.

Phase 1 has no subprocess, shell, Git, command-runner, or model-tool capability. Its only network operation is one non-streaming request to an explicitly configured `localhost`, `127.0.0.1`, or `::1` Ollama endpoint. It rejects redirects, other hosts, remote APIs, URL credentials, query strings, and fragments. The response is bounded to 1 MiB, uses a two-second connection timeout and a configurable 1-to-600-second response timeout with a 300-second default, and is never retried automatically.

## Role isolation

The Lead cannot implement code. The Developer cannot authorize commands. The runner is deterministic and cannot expand its own allowlist. The Reviewer reports findings but cannot silently alter the reviewed change.

Later versions should execute agent work in isolated, ephemeral Git worktrees or repositories. Isolation must not copy credential helpers, SSH configuration, tokens, kubeconfig files, or unrelated host data into the workspace.

## Data minimization

Before each model call, the orchestrator should construct context specifically for that role. It should measure both transmitted context and context deliberately removed. Stored reports should contain compact summaries and metrics, not unrestricted prompts, source snapshots, raw process output, or secret-bearing environment data.

## Application restrictions and OS sandboxing

Application-level restrictions constrain intended program behavior. Phase 1 validates configuration, permits only explicit local endpoint spellings, constrains report paths, bounds inputs and responses, and contains no code path for process execution. These checks reduce mistakes and misuse but do not contain a compromised binary or dependency.

OS-enforced sandboxing is a separate deployment boundary. Filesystem permissions, process restrictions, network policy, resource limits, and isolation must be applied by the operating system or a trusted sandbox. The Rust application must not claim that its validation rules provide OS-level isolation.

### Recommended restrictive launch profile

Run Phase 1 with a profile that:

- uses a dedicated, unprivileged user with no administrative rights;
- exposes no Git credentials, SSH keys, cloud credentials, kubeconfig files, credential helpers, agent sockets, or unrelated home-directory content;
- mounts or permits the executable, configuration, prompt, and schema as read-only;
- grants write access only to the validated report directory and necessary OS temporary storage;
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
