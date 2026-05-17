# Assistant Chat Contract Smoke

This smoke validates the minimum user-facing chat contract before static-page quality work resumes.

The contract is:

- Plain ordinary chat remains a normal model conversation. V3 awareness is additive, not a capability limit.
- V3-only facts require supplied V3 evidence. When that evidence is absent, answers should mark the V3 fact boundary as currently invisible or unsupplied.
- ReAct invalid-action and step-limit paths must not surface observation JSON, execution trails, runtime manifests, tool traces, provider raw payloads, or similar internal observability fields as the final user answer.
- External-channel callbacks expose task/action/search status through public redacted fields only.

## Command

```bash
bash scripts/run-assistant-chat-contract-smoke.sh
```

The script is non-destructive and does not load `/etc/aiv3/aiv3.env`. It writes machine-readable and Markdown reports under:

```text
target/assistant-chat-contract-smoke/
```

Run a separate deployment-target/provider smoke when real external-model credentials are intentionally enabled.
