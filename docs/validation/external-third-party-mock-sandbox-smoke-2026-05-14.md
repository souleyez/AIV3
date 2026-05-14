# External Third-Party Mock/Sandbox Smoke - 2026-05-14

## Target

- Deployment target: `8服务器`
- Repository path: `/srv/aiv3/repo`
- Validated commit: `aaafd6f`
- Database environment: platform service environment from `/etc/aiv3/aiv3.env`

## Result

Status: passed.

This validation extends the previous external bot and third-party deployment smoke with two database-backed mock/sandbox checks:

- a customer-hosted generic chat page posts normalized events through `POST /v1/external/channels/{connection_id}/events`;
- V3 dispatches a confirmed external action to a local third-party mock endpoint with dispatch-specific Bearer and HMAC headers.

## Passed Checks

```text
cargo test -p platform-api generic_chat_page_event_endpoint_accepts_idempotent_normalized_messages --lib -- --nocapture
```

Result: `1 passed; 0 failed`.

This confirms:

- normalized generic chat messages are accepted through the public route;
- the route returns an AssistantRun id and channel-safe task-status reply;
- a duplicate idempotency key returns the original run id;
- only one external message event is persisted for the same idempotency key.

```text
cargo test -p platform-api external_action_dispatch_posts_signed_payload_to_mock_endpoint --lib -- --nocapture
```

Result: `1 passed; 0 failed`.

This confirms:

- V3 posts the redacted external action payload to a third-party mock endpoint;
- `Authorization: Bearer ...` uses only dispatch-specific credentials;
- `X-V3-Signature` is generated from method, path/query, timestamp, nonce, and body hash;
- platform callback tokens are not reused for outbound dispatch;
- raw prompt text and callback tokens do not leave in the dispatch body;
- third-party response bodies are summarized without storing arbitrary message text or secret fields.

## Notes

- The target host still lacks PowerShell, so these steps were run directly with cargo instead of the PowerShell smoke wrapper.
- The first combined remote command hit a Windows newline issue on the second `--nocapture` argument; the second test was rerun as a single remote command and passed.
- This is still a deterministic mock/sandbox layer. It does not replace a live customer sandbox test for a real third-party gateway.
