# Rules for coding agents

These rules are durable and apply to every future change in this repository.

## Scope and delivery

- Work in small vertical slices that produce a testable outcome.
- Do not add technologies or dependencies without a demonstrated need.
- Avoid premature abstractions; prefer the smallest design that satisfies the current acceptance criteria.
- Do not modify files outside the requested scope.
- Explain important decisions and their trade-offs.
- Explicitly report assumptions, uncertainty, and incomplete information.

## Quality

- Add or update tests whenever behavior changes.
- Run relevant checks before concluding and summarize their results.
- Do not claim that a check passed unless it was actually run.
- Keep agent messages and persisted reports structured, concise, and suitable for validation.

## Change completion and Git workflow

After completing requested changes:

1. Run the relevant checks.
2. Show `git diff --stat`, `git status`, and a concise summary of the changes.
3. Do not commit until the user explicitly approves the commit.
4. After approval, create one commit containing only the requested scope.
5. Do not push unless the user explicitly requests it.

## Security and human control

- Never bypass a security rule, command allowlist, validation boundary, or approval requirement.
- Never expose Git credentials, SSH keys, cloud credentials, kubeconfig files, or other secrets to an agent or model.
- Never send raw execution logs to a model; provide only useful failure counts and concise causes.
- Give each agent only the minimum context required for its task.
- Require human approval for risky actions.
- Stop and ask for human validation when a decision affects architecture or security.

## Role boundaries

- The Lead may interpret requests, define acceptance criteria, and delegate small tasks, but must not write implementation code.
- The Developer owns implementation code and implementation-related tests.
- The runner executes only explicitly allowed commands and does not infer permission.
- The Reviewer evaluates the diff, acceptance criteria, and summarized test results; it should use a different model when practical.
