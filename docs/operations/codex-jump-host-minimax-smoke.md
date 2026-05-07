# Jump Host MiniMax Smoke Notes

**Scope:** AI Data Platform V3 Rust assistant Codex-kernel work only.

This note deliberately excludes the abandoned `codex-web` remote bridge direction. Jump-host validation here means private MiniMax/Codex Host validation for V3, not public remote Codex routing.

## Host Snapshot

```text
host_alias=windows-jump
host_os=Windows 11
node_version=22.22.1
npm_version=10.9.4
codex_version=codex-cli 0.123.0
codex_home=C:\Users\soulz\.codex
```

## MiniMax Smoke Result

```text
date=2026-05-07
base_url=https://api.minimaxi.com/v1
endpoint=/chat/completions
model=MiniMax-M2.7
result=ok
expected_text=MINIMAX_SMOKE_OK
```

The API key was injected only into the remote process environment for the smoke run. It was not printed and was not written into jump-host Codex config.

Observed behavior:

```text
MiniMax-M2.7 can return leading <think>...</think> text inside message.content.
```

V3 must normalize this before user-facing answer rendering. `llm-gateway` should strip or isolate leading reasoning blocks for OpenAI-compatible provider output.

## Codex Provider Smoke Result

```text
date=2026-05-07
codex_version=codex-cli 0.123.0
direct_minimax_chat_provider=failed
direct_minimax_responses_provider=failed
local_responses_shim_fake=ok
local_responses_shim_to_minimax=ok
expected_text=CODEX_HOST_SMOKE_OK
```

Findings:

- `wire_api="chat"` is rejected by Codex CLI 0.123.0.
- `wire_api="responses"` against `https://api.minimaxi.com/v1` fails because MiniMax has no `/responses` endpoint there.
- A local private Responses-compatible shim can satisfy Codex and translate the request to MiniMax `/chat/completions`.
- The smoke used `--ignore-user-config` so the jump host's old `souleye-cloud` Codex config was not used.
- The MiniMax key was passed through process stdin/environment for the smoke and was not written into Codex config.

Reusable smoke tool:

```text
tools/codex-host-responses-shim-smoke.mjs
```

## Safe Smoke Rules

- Do not run local-machine Codex for this project validation.
- Use `windows-jump` now, and later the dedicated Mac host.
- Do not write MiniMax keys into browser local storage or Codex task prompts.
- Do not expose MiniMax provider endpoints to browsers.
- Do not reuse abandoned public remote bridge routes for V3.
- If Codex itself needs MiniMax later, build a private Responses-compatible shim and validate it separately.
