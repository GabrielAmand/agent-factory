# S2 approved-source Explorer: implementation record

## Purpose and current status

S2 is the dormant shared-core `approved_sources` pipeline used by the first `devops_hub` data-production slice, H0. It is implemented and has passed a live end-to-end smoke test, but it is disabled by default and is not invoked by the normal V1 Lead-to-Developer workflow.

The final flow is:

```text
official-devops-tools-v2
  -> six bounded official-source retrievals
  -> six sequential single-document Explorer calls
  -> strict description and tag validation
  -> Rust-owned identity, URLs, order, and provenance
  -> deterministic Rust tag normalization
  -> FactBundleV1 construction
  -> canonical JSON serialization
  -> SHA-256 digest
```

The active H0 policy contains Docker, Kubernetes, Terraform, Jenkins, GitLab CI, and Prometheus in that deterministic order. `official-devops-tools-v1` remains the historical eight-source policy and security reference. Its Ansible and Argo CD entries remain supported by E1, but are excluded from the H0 V2 dataset because repeated live requests to their official sources returned HTTP 429. This exclusion does not weaken or remove their retrieval policy.

S2 does not invoke a Developer, create `resources.json`, publish or preview a workspace, generate the `devops_hub` frontend, run a Reviewer, schedule refreshes, ingest news, or activate `open_discovery`.

## Evolution and failed approaches

### Batch extraction

The initial S2 design sent all six normalized documents in one Explorer request and expected one complete structured collection. This was deterministic in Rust but unreliable with the local `gemma3:latest` model. One live run returned only three items for six inputs. An earlier contract that also asked the model to repeat trusted metadata produced five mixed fact IDs associated with the Prometheus source.

These failures showed that schema-valid generation alone was not enough to make a constrained model copy a complete cross-document identity set reliably.

### Removing metadata from model authority

Registry-owned values were removed from Explorer output. The model no longer returns or selects fact IDs, display names, source IDs, official URLs, source URLs, policy identifiers, order, or provenance. Rust already knows those values from the validated registry and completed retrieval set, so asking the model to reproduce them added failure modes without adding information.

The model now produces only a description and tags. Rust constructs all trusted FactBundle metadata. Provenance therefore records which validated document Rust supplied for a fact; it does not prove perfect semantic entailment of every generated sentence.

### Sequential single-document extraction

Removing metadata did not make batch completeness reliable: another live run returned three semantic items for six documents. S2 therefore changed to one independent call per registry entry. Each call receives exactly one normalized document and must return exactly:

```json
{"description":"...","tags":["..."]}
```

This design provides smaller context, a simpler contract, no opportunity for cross-document identity mixing, precise trusted-fact diagnostics, and better suitability for a limited local model. Calls remain sequential in registry order. The first failure stops the pipeline; there is no retry, repair, fallback, parallel execution, or partial publication. Earlier successful semantic results remain memory-only until every entry succeeds.

Sequential execution trades latency for reliability and lower model and hardware requirements.

## Tag-normalization lesson

The first sequential live run passed Docker but rejected Kubernetes with `semantic_fields_invalid`. Sanitized classification identified `tag_invalid_characters`: the rejected tag was within the byte limit but used formatting outside the conservative stored vocabulary. No tag value was logged or reconstructed.

Rust now normalizes each model-produced tag before final validation and storage:

1. trim leading and trailing ASCII whitespace;
2. lowercase ASCII uppercase letters;
3. convert runs of ASCII whitespace to one hyphen;
4. preserve only `a-z`, `0-9`, and hyphens, removing other ASCII punctuation;
5. collapse repeated hyphens;
6. remove leading and trailing hyphens;
7. reject an empty result;
8. enforce the existing UTF-8 byte limit after normalization;
9. reject duplicates after normalization;
10. store only the normalized form.

Non-ASCII tags are rejected rather than transliterated. Description validation was not weakened.

## Trust and security boundaries

The Retriever owns official registry policy enforcement, HTTPS-only retrieval, SSRF and special-address rejection, DNS validation and address pinning, proxy suppression, manual redirect validation, content-type enforcement, byte and time limits, and bounded deterministic HTML normalization.

The Explorer model owns only candidate descriptions and tags. Its response is untrusted until Rust validates it.

Rust owns policy selection, expected fact count, IDs, names, URLs, registry order, document-to-fact association, provenance, tag normalization, semantic validation, FactBundle construction, canonical serialization, and SHA-256 calculation. No partial FactBundle is approved. On the first semantic failure, later calls are not made and earlier results remain memory-only.

Future policies with multiple documents per fact may need a separate trusted evidence-group contract. They must not restore model-selected source identifiers as retrieval authority.

## Sanitized diagnostics

S2 diagnostics may contain the trusted fact ID and call index; request and response byte counts; per-call and aggregate durations; prompt and output token counts; stable validation reasons; and bounded semantic-failure metadata such as field name, description byte count, tag count, rejected tag index, and rejected tag byte count.

Diagnostics exclude descriptions, tag values, prompts, normalized source text, raw model responses, secrets, full IP addresses, cookies, credentials, arbitrary headers, environment dumps, and dependency error strings.

## Successful H0 live evidence

The final live smoke test used policy `official-devops-tools-v2` and model `gemma3:latest`. All six retrievals, six sequential Explorer calls, six semantic validations, and six Rust-owned FactBundle facts succeeded.

| Fact | Request bytes | Response bytes | Duration (ms) | Prompt tokens | Output tokens |
|---|---:|---:|---:|---:|---:|
| Docker | 7,807 | 122 | 4,990 | 1,881 | 27 |
| Kubernetes | 5,038 | 545 | 6,500 | 1,331 | 102 |
| Terraform | 3,765 | 355 | 5,920 | 1,096 | 70 |
| Jenkins | 2,733 | 378 | 5,885 | 835 | 72 |
| GitLab CI | 7,032 | 327 | 6,270 | 1,887 | 77 |
| Prometheus | 7,384 | 346 | 6,138 | 1,930 | 67 |

The resulting bundle was `fact-bundle-v1`, with 3,271 canonical UTF-8 JSON bytes and SHA-256 digest `2dad8091a23e292684794f649945814497876272ddd6b062d7b390258026f742`. Retrieval took 856 ms, Explorer calls took 35,709 ms in total, and the complete pipeline took 36,566 ms.

There was no retry, repair, fallback, parallel execution, or partial publication. At the verification point, 125 tests passed and `cargo fmt --check`, Clippy with warnings denied, `cargo test`, and `git diff --check` all passed.

## Current limitations and next work

- S0/E0 contracts and policy validation are complete.
- S1/E1 bounded retrieval is complete.
- S2/E2 is implemented, successfully live-smoke-tested, and dormant by default.
- The H0 approved-source data-production path is validated.
- S3/H0 Developer integration and Rust-owned `resources.json` are next.
- S4 functional Runner work is future.
- S5 Reviewer and immutable workspace revision work is future.
- `open_discovery` remains future work with separate contracts and authority boundaries.

S2 currently uses one source document per fact and does not establish sentence-level factual entailment. It has no cache or retry policy, and sequential model unloading favors isolation over speed.

## Architectural lesson

For constrained local models, agent-factory favors controlled degradation: smaller tasks, sequential execution, deterministic host-language ownership, strict validation, and slower but reliable completion. More capable local or paid providers may later support batching or parallel execution behind the same FactBundle contract, but correctness must not depend on model quality.
