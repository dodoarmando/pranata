# Every design decision is traced to a plan task and a check

Each decision in a design document gets an ID (`D-01`, `D-02`, …) and names how it is checked. The implementation plan must cite every ID from at least one task, and every task must cite at least one ID. A script in CI fails when either side is missing, and the PR template asks which decisions a PR implements.

The archived attempt was abandoned because decisions agreed in up-front design were never turned into issues and were silently dropped. The best-known case was shadcn, applied only after a corrective issue when dozens of pages already existed without it. Reviews and good intentions did not catch this; a mechanical check does.

## Considered Options

- **Rely on issue templates and review**: what failed before.
- **Track decisions in a spreadsheet or the issue tracker only**: drifts from the documents and is not checked by CI.

## Consequences

Design documents and plans must keep the ID format stable. Decisions that can be checked by a machine (lint rules, tests, CI jobs) are written that way rather than as prose.
