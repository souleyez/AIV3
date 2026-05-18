# ADR-0006: Add V3 Edge / Local Data Plane Mode

## Status
Accepted

## Context
V3 already supports standard third-party channel integration where external chat events enter V3 and V3 can manage retrieval, assistant runs, external actions, and artifact status. Some customers need a stricter deployment shape: their source documents, parsed text, vector index, generated pages, and final answers should stay on their own servers. In that shape, V3 should provide capability orchestration, model-routing policy, permission contract, static-page generation rules, and audit summaries without becoming the custodian of third-party source material.

This is an additive mode. It must not replace or weaken the current third-party integration path, inbound bearer protection, external action dispatch, or existing V3 dataset workflows.

Key constraints:

- Third-party browsers must not receive V3 bearer tokens.
- Source documents can remain in third-party storage.
- Parsing, indexing, retrieval, and page hosting can run in the third-party environment.
- V3 can still define the capability contract, route model choices, validate manifests, and collect redacted status.
- The mode must be testable before a real customer installs an edge runtime.

## Decision
Add a new `V3 Edge / Local Data Plane` mode as a parallel integration architecture.

In this mode:

- V3 remains the control plane for capability contracts, model lane policy, permission protocol, page generation rules, and audit summaries.
- A customer-hosted Edge Agent runs in the third-party environment and owns the local data plane.
- The Edge Agent may parse documents, build local indexes, enforce ACLs, run retrieval, generate page blueprints or HTML packages, and publish those packages to the third-party server.
- The browser talks to the third-party gateway or Edge Agent, not directly to V3.
- V3 stores redacted manifests, state transitions, hashes, source counts, and capability status, but not raw third-party source documents by default.

Two execution profiles are allowed:

- `local_strict`: parsing, retrieval, generation, and page publishing all run in the third-party environment.
- `local_retrieval_v3_generation`: documents and indexes stay local, but the Edge Agent sends permission-filtered evidence snippets to V3 for generation when the customer approves that boundary.

## Consequences

### Positive
- Supports customers who require local custody of documents and generated pages.
- Keeps the existing V3 third-party integration intact while adding a stronger deployment option.
- Makes V3 easier to position as an AI capability layer rather than a data-hosting replacement.
- Gives implementation a concrete manifest and validator before building a full edge runtime.

### Negative
- Adds operational complexity because customers must run and monitor an Edge Agent.
- Requires careful contract testing between V3 and the customer-hosted runtime.
- Strict-local generation depends on local model or approved model-gateway availability.

### Neutral
- Some artifacts become remote references instead of V3-hosted objects.
- V3 observability must rely on redacted manifests, hashes, and status summaries.
- Customer-specific network, certificate, model, and storage rules remain deployment-time concerns.

## Alternatives Considered

**Use the existing third-party V3-hosted data path only**

Rejected for customers that cannot send source documents or generated pages to V3 storage.

**Let the third-party system call models directly without V3**

Rejected because it loses V3's capability policy, permission protocol, audit model, and static-page generation discipline.

**Move all V3 services into the customer environment**

Rejected for the first iteration because it is too heavy. The Edge Agent pattern provides a smaller boundary that can later grow into a fuller private deployment if needed.

## References
- `docs/integrations/v3-edge-local-data-plane-mode.zh-CN.md`
- `docs/integrations/v3-edge-local-data-plane.sample.json`
- `tools/validate-v3-edge-local-data-plane.mjs`
