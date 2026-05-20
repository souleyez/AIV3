# External User History Memory Scope Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to execute this plan task-by-task.

## Goal

When a third-party channel request carries a stable external user id, V3 should maintain that user's historical conversation context as an intent-gated user history scope. The history must not be supplied by default. It should only enter the answer context when the model/scope planner judges the user is asking for prior-user-context, such as "what did I say before", "continue from last time", or "based on my previous conversation".

This should reuse the existing global conversation-memory rule instead of creating an always-on chat transcript injection path.

## Current Findings

- Third-party requests already carry `sender_external_id` in `selected_scope` metadata.
- Current external conversation replay is keyed by:
  `external:{platform}:{tenant_external_id}:{bot_external_id}:{conversation_external_id}`
  so it only covers the current third-party conversation, not the same user across conversations.
- Runtime scope planning already has the desired global gate:
  `conversation_memory_available && prompt_references_conversation_history(prompt)`.
- `conversation_memory_items` are currently keyed by `local_thread_id`; evidence supply only loads memory for the request's current `local_thread_id`.
- External ACL boundaries already use external principal mapping from `sender_external_id`; user-history memory must preserve the same tenant/bot/user isolation.

## Proposed Architecture

Reuse `conversation_memory_items` as the storage primitive and add a deterministic external-user memory thread key:

```text
external-user:{platform}:{tenant_external_id}:{bot_external_id}:{sender_external_id}
```

For every completed external turn with `sender_external_id`, mirror a compact memory item into that external-user key. On future requests from the same external user, expose this user-history key as a conversation-memory candidate, but only supply its memory items when the selected scope explicitly includes conversation memory.

Do not append cross-conversation user history to `assistant_request.messages`. The existing current-conversation history can remain as ordinary short conversation continuity; cross-conversation user history must stay intent-gated.

## Task 1: Lock The Global Intent Gate

Files:

- `crates/assistant-runtime/src/lib.rs`

Work:

- Review existing `conversation_memory_is_only_candidate_when_prompt_references_history` coverage.
- Add or extend a test that uses external-style scope metadata and verifies:
  - generic prompts do not select conversation memory;
  - history-referencing prompts do select conversation memory when available;
  - selected scope keeps `historyPolicy = intent_gated_selected` only when memory is selected.

Validation:

```powershell
cargo test -p assistant-runtime conversation_memory
```

## Task 2: Add External User Memory Key Helpers

Files:

- `crates/platform-api/src/lib.rs`

Work:

- Add helper functions near the current external local-thread helpers:
  - `external_user_memory_local_thread_id(platform, tenant_external_id, bot_external_id, sender_external_id)`
  - optional parser/metadata helper for tests and diagnostics.
- Keep key components tenant- and bot-scoped to avoid cross-customer leakage.
- Treat blank or missing user id as "no user-history scope".

Validation:

- Unit test stable key derivation.
- Unit test different tenant/bot/user values produce isolated keys.

## Task 3: Mirror External Turns Into User Memory

Files:

- `crates/platform-api/src/lib.rs`
- storage repository only if an existing insert/upsert method is insufficient.

Work:

- After an external assistant run has a provider-authored answer, write a compact memory record under the external-user memory key.
- Store enough metadata to debug and filter later:
  - source local conversation thread id;
  - external channel connection id;
  - conversation external id;
  - message external id;
  - sender external id;
  - run id;
  - timestamp.
- Keep memory content compact. Prefer existing memory candidate/summary shape if available; otherwise store a bounded turn summary made from user text and assistant answer.
- Do not write memory for failed/no-answer fallback states.

Validation:

- Unit/integration test that an external completed turn creates a memory item for the user key.
- Test repeated idempotent event handling does not duplicate memory when `idempotency_key` repeats.

## Task 4: Expose User History As A Candidate, Not Default Context

Files:

- `crates/platform-api/src/lib.rs`
- possibly `crates/assistant-runtime/src/lib.rs` if candidate metadata needs richer labels.

Work:

- When building the external assistant run request and `sender_external_id` is present:
  - check whether the external-user memory key has visible memory items;
  - set `conversation_memory_available = true` or add a concrete user-history candidate only when memory exists;
  - keep generic prompts on ordinary/external document scope.
- Preserve existing current-conversation history loading; do not merge user-history memory into `messages`.
- Add selected-scope metadata that can identify the chosen external-user memory source if the planner selects conversation memory.

Validation:

- Test same external user across different `conversation_external_id` sees a user-history candidate.
- Test generic prompt does not supply that candidate.
- Test "结合我上次说的..." selects it.

## Task 5: Supply Selected External User Memory In Evidence

Files:

- `crates/platform-api/src/lib.rs`

Work:

- Extend evidence-state memory loading from "current local_thread_id only" to "selected memory source ids":
  - current conversation thread memory, when selected;
  - external-user memory key, when selected;
  - no user-history load when memory is not selected.
- Keep output item type compatible with existing `conversation_memory_items` evidence.
- Include parse/status style supply notes if useful, but avoid turning memory into a document dataset by default.

Validation:

- Test selected external user memory appears in evidence state.
- Test another `sender_external_id` cannot load the first user's memory.

## Task 6: External API And Model Behavior Tests

Files:

- `crates/platform-api/src/lib.rs`
- `scripts/run-external-direct-reply-smoke.sh`

Work:

- Add external channel regression tests:
  - first turn creates user memory;
  - second conversation from same sender can use it only when intent-gated;
  - different sender is isolated;
  - third-party direct replies remain provider-authored and never degrade to the old fixed "已收到指令..." fallback.
- Extend smoke script with an intent-gated user-memory case if the local test harness can seed memory cheaply.

Validation:

```powershell
cargo test -p platform-api external_user_memory
bash scripts/run-external-direct-reply-smoke.sh
```

## Task 7: Documentation And Operational Notes

Files:

- `docs/integrations/pure-third-party-integration-guide.zh-CN.md`
- sendable third-party docs only after confirming current dirty edits are intended to include this change.

Work:

- Document `sender_external_id` as the stable key used for optional user-history memory.
- Clarify `mention_external_user_ids` is not this feature; it is mention metadata and should not grant access to another user's history.
- Add behavior wording:
  - user history is retained as an internal optional memory scope;
  - it is not supplied by default;
  - it is selected only when the request intent asks for historical/personal context;
  - tenant/bot/user boundaries are enforced.

Validation:

- Re-read docs for third-party implementer ambiguity.

## Rollout Order

1. Tests around existing global memory gate.
2. External user memory key and storage mirroring.
3. Candidate exposure without default supply.
4. Evidence supply when selected.
5. External direct-reply smoke.
6. Docs.

## Risks And Guardrails

- Do not treat `mention_external_user_ids` as permission to read mentioned users' histories.
- Do not inject cross-conversation user history into `messages`; keep it selected-scope evidence only.
- Do not create document memberships for chat history unless later needed. The existing conversation-memory storage is a better fit for this global rule.
- Keep memory compact and bounded to avoid bloating prompts or leaking stale context.
- Preserve current third-party document temporary dataset behavior and ACL filters.
