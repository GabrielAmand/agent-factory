# Agent Factory V0 status

Agent Factory V0 is a preserved experimental prototype and evidence base. The project is paused while its research question and future direction are reconsidered. The current architecture and artifacts document what was tried; they are not presumed to define a future version.

## What V0 tested

- Local structured model calls through Ollama.
- A Lead producing acceptance criteria and task decomposition.
- Deterministic selection of one dependency-free task.
- Proposal-only and constrained static-site Developer experiments.
- Strict Rust validation of model output.
- Atomic publication and bounded preview of four-file static sites.
- Prompt-variant repeatability experiments.
- Disabled-by-default official-source retrieval and Explorer extraction.
- Privacy-minimized execution reporting and documented security boundaries.

## What V0 successfully demonstrated

- Local models can reliably produce schema-valid structured responses under strict bounds.
- Rust can enforce output, filesystem, network, and publication boundaries around untrusted model output.
- Per-role token and timing metrics can be captured without persisting raw prompts or generated file contents.
- Atomic workspace publication and constrained local preview are practical.
- The official-source Retriever and Explorer can operate within a narrow, fail-closed policy.
- Smaller sequential extraction calls improved reliability for the tested Explorer model and source set.
- Technical or schema success does not imply useful task selection or correct software.
- The tested first-ready delegation rule and Lead prompt refinement did not reliably select core user value.

## What V0 did not demonstrate

- That specialized agent teams outperform one general-purpose agent.
- General repository implementation by a Developer.
- A deterministic command runner or correction loop.
- The value of a Reviewer.
- Meaningful final-artifact success rates across a benchmark suite.
- Controlled comparisons of models, prompts, skills, tools, workflows, context strategies, or budgets.
- General applicability beyond the tested static-site and DevOps-source scenarios.
- Reliable semantic information-loss or context-transfer measurement.
- A reusable architecture for a future Agent Factory version.

## Assumptions encoded too early

- Lead task decomposition as the required workflow entry point.
- Task dependency graphs and first-ready selection.
- Lead and Developer as fixed core roles.
- Exactly one Lead call and one Developer call.
- Exactly four static-site files as the implementation artifact.
- Prompts and schemas compiled into the binary.
- Ollama as the architectural provider boundary rather than one experimental backend.
- DevOps approved-source research as a shared-core direction.
- Retriever and Explorer roles as likely future concepts.
- Phase-oriented report formats and the existing roadmap as architectural commitments.
- Schema validity as a strong proxy for stage success.

## Artifacts preserved for reproducibility

The following V0 artifacts must remain unchanged unless a later, explicitly approved preservation change is required:

- all Rust source and tests;
- Cargo manifests and lockfile;
- configuration;
- prompts and schemas;
- official-source registries;
- fixtures;
- experiment documents;
- execution and benchmark reports;
- generated workspaces;
- Retriever and Explorer artifacts;
- DevOps Hub evidence;
- security, architecture, vision, and roadmap documents;
- Git history.

Preservation records the experiment and does not endorse these artifacts as the basis for a future architecture.

## Unresolved research questions

These questions should be answered outside implementation before work on a future version begins:

- What precise causal question should the first experiment answer?
- What constitutes software-engineering success?
- What benchmark tasks and evaluators can support fair comparison?
- Which variables should be compared first?
- How should total resources be matched between a single agent and a team?
- What should agent, role, prompt, skill, tool, workflow, policy, and evaluator mean in the future research model?
- What context and information-transfer measurements are essential?
- Which deterministic capabilities are necessary for the first experiment?
- What evidence would justify adding planning, review, or correction roles?
- Which V0 components, if any, should be reused?
- Should future work extend this repository or begin from a smaller separate implementation?
- What reproducibility, privacy, isolation, and human-approval boundaries are mandatory?

## Possible future direction

Agent Factory is paused, not abandoned. The project may be revisited after practical experience has been gained from a separate, smaller project focused on producing a real working application rather than designing a general agent framework.

Lessons from that work may support future exploration of:

- building an actual application through an agent workflow;
- DevOps Hub as a possible application or demonstration target;
- reusable and measurable agent skills;
- better separation between agents, skills, tools, workflows, policies, and evaluators;
- repository and GitHub exploration capabilities;
- controlled retrieval of external technical information;
- context reduction and selective information transfer;
- comparison of specialized local agents against simpler approaches.

These are possible future directions, not architectural commitments for V1 or any other version. The purpose of the V0 freeze is to avoid continuing implementation before enough practical evidence exists to design Agent Factory correctly.
