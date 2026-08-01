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

## Role isolation

The Lead cannot implement code. The Developer cannot authorize commands. The runner is deterministic and cannot expand its own allowlist. The Reviewer reports findings but cannot silently alter the reviewed change.

Later versions should execute agent work in isolated, ephemeral Git worktrees or repositories. Isolation must not copy credential helpers, SSH configuration, tokens, kubeconfig files, or unrelated host data into the workspace.

## Data minimization

Before each model call, the orchestrator should construct context specifically for that role. It should measure both transmitted context and context deliberately removed. Stored reports should contain compact summaries and metrics, not unrestricted prompts, source snapshots, raw process output, or secret-bearing environment data.

## Failure behavior

Invalid JSON, schema violations, disallowed commands, unexpected paths, missing approval, and possible secret exposure must fail closed. Errors should be locally diagnosable while the model receives only the smallest safe summary needed to revise its response.

## Human validation boundaries

Human validation is required before adopting or materially changing:

- the command allowlist and approval policy;
- credential and secret-detection rules;
- sandbox and workspace isolation boundaries;
- network or remote Git capabilities;
- retention and redaction policy for reports;
- any architecture choice that changes a trust boundary.

