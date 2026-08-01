# Phase 2 V1 repeatability baseline

## Experiment objective

This experiment measured the repeatability of Phase 2 V1 Lead task decomposition and the quality of the task chosen by the deterministic first-ready-task rule. It established a Variant A baseline for a controlled prompt experiment without changing execution or security behavior.

## Fixed conditions

The exact user request was:

> Create a minimal static website that displays a list of DevOps tools and allows filtering by tags.

The Lead model was `gemma3:latest`, and the Developer model was `qwen2.5-coder:7b`. The complete sequential Lead-to-Developer proposal workflow ran five times. Prompts, schemas, configuration, model settings, limits, reports, and security boundaries were unchanged.

Every run used the existing selection rule: choose the first Lead task, in Lead-provided order, whose `depends_on` array is empty. Each role was called at most once per run, without retries, and each request used `keep_alive: 0`.

## Results

All five runs completed successfully. Lead schema validation passed in 5/5 runs, and Developer schema validation passed in 5/5 runs.

Selected-task quality was classified using the benchmark's predefined rules:

| Classification | Runs | Share |
|---|---:|---:|
| Core | 0 | 0% |
| Supporting | 3 | 60% |
| Cosmetic | 2 | 40% |
| Invalid | 0 | 0% |

One selected task was structurally related to the tool list and tag-filtering controls, but it did not directly implement either core behavior. No selected task directly implemented tool-list display or tag filtering.

### Token statistics

| Role and metric | Average | Minimum | Maximum |
|---|---:|---:|---:|
| Lead prompt tokens | 214.0 | 214 | 214 |
| Lead output tokens | 333.2 | 301 | 401 |
| Developer prompt tokens | 208.4 | 201 | 218 |
| Developer output tokens | 241.4 | 171 | 305 |

### Duration statistics

| Measurement | Average | Minimum | Maximum |
|---|---:|---:|---:|
| Lead duration | 9.512 s | 8.782 s | 10.305 s |
| Developer duration | 9.792 s | 7.408 s | 13.685 s |
| Total application duration | 19.308 s | 16.194 s | 23.996 s |

## Decomposition pattern

The decomposition was structurally stable even though task identifiers and titles varied. Every run produced four ordered stages resembling:

```text
structure or layout -> tool data -> filtering -> styling
```

The dependency-free task was always the structure or layout stage. Tool data always preceded filtering, and styling was always last. Because the selector always chooses the first dependency-free task, this repeated ordering consistently delegated scaffolding instead of core behavior.

## Observed weaknesses

- No selected task directly implemented the tool list or tag-filtering behavior.
- One run introduced navigation as a prerequisite even though navigation was not required by the request.
- Two selected tasks were limited to a skeleton, heading, or homepage layout.
- Several acceptance criteria were vague rather than behaviorally testable.
- One run showed acceptance-criteria drift between downstream tasks.
- Two Developer proposals contained no proposed tests.
- One Developer proposed `src/index.html`; without repository context, that path was speculative rather than demonstrably grounded.

No duplicate task IDs or invalid dependency references passed validation. The weakness was therefore not schema correctness or dependency coherence. It was the interaction between a consistently scaffold-first Lead decomposition, task ordering, and the first-ready selection rule.

## Privacy and repository integrity

The exact raw request and complete role prompts were absent from all five reports. Reports contained no raw model responses, headers, credentials, secrets, reasoning, or environment dumps. No suspicious sensitive text appeared in validated fields.

Both models were unloaded after every run. The tracked repository fingerprint was identical before and after the benchmark:

```text
f6b998b98e9b749523057afa8f60602e9bca08dcaa82c26d05bcf204de2a7750
```

No tracked project or source file changed during the benchmark. Runtime changes were limited to ignored execution reports, Cargo output, and Ollama runtime state.

## Conclusion and next hypothesis

The current first-ready rule is deterministic, but under the observed Lead decomposition it does not select core value. It repeatedly selects prerequisite structure or cosmetic layout while the requested behaviors remain in downstream tasks.

The next controlled hypothesis is:

> A Lead prompt that requires one independently delegable core vertical slice will improve selected-task value while keeping the existing deterministic first-ready-task selector unchanged.

Variant B should change only the Lead prompt. Holding the selector and every other variable constant will help distinguish a Lead decomposition problem from a selector problem while limiting overfitting risk and preserving the Phase 2 V1 security boundary.

## Evidence

The ignored aggregate artifact is `reports/phase2-v1-repeatability-benchmark.json`. It references the five versioned execution reports used for this baseline. The reports are local runtime evidence and are not copied into this document.
