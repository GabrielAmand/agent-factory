# Phase 2 V1 vertical-slice prompt variant

## Hypothesis

Variant B tested whether requiring one independently delegable core vertical slice in the Lead prompt would improve selected-task value while leaving the deterministic first-ready selector unchanged.

## Controlled variable

Only `prompts/lead-v1.txt` changed. The added generic guidance required a dependency-free core user-visible behavior, enough supporting structure inside that slice, behaviorally testable acceptance criteria, core work before cosmetic or optional infrastructure, and only genuine dependencies.

The wording did not mention websites, HTML, tool lists, tags, or filtering.

## Unchanged variables

The experiment retained:

- Lead model `gemma3:latest`;
- Developer model `qwen2.5-coder:7b`;
- Developer prompt, schema, and minimum task context;
- Lead and Developer schemas;
- root configuration and model settings;
- first-ready selection implementation;
- sequential execution and `keep_alive: 0`;
- all payload, string, collection, timeout, and response limits;
- reporting and security boundaries;
- the fixed benchmark request and five-run sample size.

No retry, source access, file application, command execution, Git operation, tool use, concurrency, or remote API was introduced.

## Protocol

The same static-website request used in Variant A was submitted to the complete Lead-to-Developer proposal workflow exactly five times. Each run made one Lead call and at most one Developer call. There were no manual retries or changes between runs. After every run, the report was inspected and `ollama ps` confirmed that both models were unloaded.

Selected tasks used the same `core`, `supporting`, `cosmetic`, and `invalid` classifications as Variant A. A core task had to directly implement tool-list display, tag filtering, or both; merely creating structure or controls was not sufficient.

## Variant A baseline

Variant A achieved 5/5 technical successes and 5/5 schema validations for each role. Its selected-task distribution was 0 core, 3 supporting, 2 cosmetic, and 0 invalid. The Lead consistently produced a scaffold-first sequence resembling structure, tool data, filtering, then styling. No selected task directly implemented a core behavior.

Variant A averages were:

| Metric | Average |
|---|---:|
| Lead prompt tokens | 214.0 |
| Lead output tokens | 333.2 |
| Developer prompt tokens | 208.4 |
| Developer output tokens | 241.4 |
| Lead duration | 9.512 s |
| Developer duration | 9.792 s |
| Total application duration | 19.308 s |

## Variant B results

All five runs completed successfully, and both role validations passed in all five runs. The selected-task distribution was:

| Classification | Runs | Share |
|---|---:|---:|
| Core | 1 | 20% |
| Supporting | 2 | 40% |
| Cosmetic | 2 | 40% |
| Invalid | 0 | 0% |

Only one run contained a dependency-free core vertical slice. That task directly implemented tool-list display but not tag filtering. The other four runs continued to place HTML setup, base structure, boilerplate, or a skeleton first.

All five selected tasks had acceptance criteria that were observable in isolation, but four sets of criteria tested only scaffolding rather than a requested core behavior. Three decompositions contained artificial sequencing, criteria drift, or both. In particular, two runs placed styling before tool data, and another placed full validation after styling in a strictly serial chain.

The Developer returned four `proposal_ready` decisions and one `clarification_required` decision. It proposed four tests in total, compared with six in Variant A. Most proposals used `index.html`; `tools-ui/index.html` was speculative because the Developer still had no repository context.

### Variant B token statistics

| Role and metric | Average | Minimum | Maximum |
|---|---:|---:|---:|
| Lead prompt tokens | 316.0 | 316 | 316 |
| Lead output tokens | 351.4 | 162 | 416 |
| Developer prompt tokens | 209.4 | 199 | 232 |
| Developer output tokens | 226.8 | 177 | 273 |

### Variant B duration statistics

| Measurement | Average | Minimum | Maximum |
|---|---:|---:|---:|
| Lead duration | 9.743 s | 6.607 s | 11.227 s |
| Developer duration | 9.076 s | 8.111 s | 9.655 s |
| Total application duration | 18.268 s | 13.831 s | 19.950 s |

## Comparison

| Measure | Variant A | Variant B | Change |
|---|---:|---:|---:|
| Core selections | 0 | 1 | +1 |
| Supporting selections | 3 | 2 | -1 |
| Cosmetic selections | 2 | 2 | 0 |
| Direct core-behavior selections | 0 | 1 | +1 |
| Lead validation | 5/5 | 5/5 | unchanged |
| Developer validation | 5/5 | 5/5 | unchanged |
| Ready Developer proposals | 5 | 4 | -1 |
| Proposed tests | 6 | 4 | -2 |

The prompt added 102 Lead prompt tokens on average. Lead output increased by 18.2 tokens, while Developer output decreased by 14.6 tokens. Average Lead duration increased by 0.231 seconds; Developer duration decreased by 0.716 seconds; total application duration decreased by 1.040 seconds. These small timing changes do not explain the quality result.

Variant B produced a modest improvement from zero to one core selection, but it did not change the dominant scaffold-first pattern. Developer usefulness declined slightly because one run required clarification and fewer tests were proposed. Each variant produced one notably speculative path.

## Success thresholds

| Threshold | Result |
|---|---|
| Lead validation passes 5/5 | Met |
| Developer validation passes 5/5 | Met |
| Dependency-free core slice in 5/5 | Not met: 1/5 |
| At least four core selections | Not met: 1/5 |
| No cosmetic or invalid selections | Not met: two cosmetic |
| At least four direct core-behavior selections | Not met: 1/5 |
| Testable selected acceptance criteria in at least 4/5 | Met: 5/5 |
| No artificial or incoherent dependencies | Not met |
| Privacy, integrity, sequential execution, and unloading pass 5/5 | Met |

Variant B therefore failed its overall success threshold.

## Conclusion

The primary remaining weakness is Lead decomposition, with task ordering as a closely related secondary weakness. The generic prompt instruction was followed once but ignored or only superficially reflected in four runs. The unchanged first-ready selector continued to expose scaffold-first output, but the experiment does not isolate the selector as the cause: in four runs there was no dependency-free core task available for it to choose.

Developer quality was secondary. The Developer generally responded consistently with the narrow selected task, although the clarification response, reduced test proposals, and speculative path show the limitations of minimal context.

## Is a selector experiment justified?

Not yet. The predeclared justification condition required the Lead to produce a dependency-free core task in at least four runs while the selector missed it in at least two. Variant B produced such a task only once, and the selector chose it correctly. Changing the selector now would add complexity without addressing the demonstrated decomposition failure.

A further Lead-focused experiment should be considered before selector work, but this document does not prescribe or implement another change.

## Limitations and overfitting risk

- Five runs are sufficient for a small local comparison but not a general model evaluation.
- Both variants used one request, so results may not transfer to other software tasks.
- Quality and dependency classifications require human judgment.
- Ollama sampling was not explicitly seeded, so natural model variation remains.
- The generic wording limits direct overfitting, but repeated evaluation on the same website request can still encourage experiment-level overinterpretation.
- The Developer receives no repository context by design, so proposed paths cannot be evaluated as grounded implementation locations.

The local aggregate evidence is stored in the ignored file `reports/phase2-v1-variant-b-benchmark.json`; this document contains only concise metrics and task-level observations.
