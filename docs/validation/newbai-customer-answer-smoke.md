# NewBai Customer Answer Smoke

This smoke evaluates customer-facing NewBai answer quality separately from
retrieval-only metrics. It is deterministic and offline by default.

It exists because a retrieval result can be correct while the final customer
answer still regresses by:

- exposing internal evidence/runtime fields;
- claiming that a report or page was generated without an artifact;
- routing an ordinary question into NewBai/report generation;
- ignoring the existing NewBai report template and creating new visual pages;
- using only document snippets when the answer needs structured database rows;
- using only database rows when the answer needs document/report narrative.

## Command

```bash
bash scripts/run-newbai-customer-answer-smoke.sh
```

Reports are written under:

```text
target/newbai-customer-answer-smoke/
```

The default command uses the fixture `sample_result` answers and does not call
DataMax, a model provider, PostgreSQL, media workers, report rendering, or
static-page publishing.

To evaluate a captured provider/live result set without adding network behavior
to the smoke itself:

```bash
NEWBAI_CUSTOMER_ANSWER_RESULTS_JSONL=target/newbai-customer-answer-smoke/live-results.jsonl \
  bash scripts/run-newbai-customer-answer-smoke.sh
```

To fail the evaluator when any supplied result does not pass:

```bash
npm run smoke:newbai-customer-answer -- \
  --results-jsonl target/newbai-customer-answer-live-capture/<run>.results.jsonl \
  --require-ready
```

Each result line should include at least:

```json
{"case_id":"newbai-customer-001","answer":"...","evidence":[{"type":"retrieval_evidence","source":"...","text":"..."}],"artifacts":[],"side_effects":{"static_page_published":false,"report_generation_enqueued":false,"new_template_generated":false}}
```

## Coverage

- Existing NewBai evidence handoff prompts:
  - `固定与提成取高预警V1 的计算方法是什么？`
  - `低活跃品牌 1月 超过8天无销售 对应哪张表？`
  - `低活跃品牌 2月 超过4天无销售 的数据在哪里？`
  - `主要经营指标 预算 2026 相关内容`
  - `Data Buddy AI 经营分析 5个重点场景`
  - `bi_contract_warning 合同预警 表说明`
  - `表4 固定提成取高20260226 固定提成内容`
  - `固定与提成取高 销售缺口 quekou xuzengxiaoshou`
- Template reuse guards:
  - reuse the existing NewBai report template;
  - do not fall back to HTML;
  - do not create unrelated new visual pages;
  - handle customer changes as existing-template content/module adjustments.
- Ordinary-question guard:
  - non-NewBai questions must remain ordinary chat and must not trigger report
    generation.
- Mixed data guard:
  - database rows and document/report narrative can both be required for one
    answer.

## Metrics

The JSON report records:

```text
case_count
passed_case_count
failed_case_count
answer_pattern_match_rate
evidence_use_rate
internal_leak_count
forbidden_claim_count
template_side_effect_count
ordinary_question_misroute_count
template_reuse_failure_count
ready
```

`ready=true` means the supplied answers passed the local answer-surface and
side-effect checks. It does not prove live provider quality unless the report
was generated from a controlled provider/live `--results-jsonl` capture.

## Release Boundary

This smoke is safe to run locally or on a deployment target because it does not
mutate services and does not enable:

```text
RETRIEVAL_SEARCH_BACKEND=postgres_lexical
pgvector
Qdrant
hybrid RRF
reranker
structured query plan
```

It should be paired with retrieval-quality and AssistantRun handoff smokes
before any release decision.

## P1-12B Live Capture Harness

The result capture harness is separate from the evaluator:

```bash
npm run smoke:newbai-customer-answer-live-capture -- --self-test
```

Self-test writes a synthetic result JSONL from the committed fixture
`sample_result` entries and immediately feeds that JSONL into the evaluator. It
does not call DataMax.

Preflight validates the controlled external-channel payload shape without
network calls:

```bash
npm run smoke:newbai-customer-answer-live-capture -- \
  --preflight \
  --base-url https://v3.elepcloud.com \
  --connection-id generic-chat-main \
  --dataset-external-ids <newbai-dataset-external-id>
```

Live capture is intentionally gated:

```bash
npm run smoke:newbai-customer-answer-live-capture -- \
  --run-live \
  --ack-controlled-live \
  --base-url https://v3.elepcloud.com \
  --connection-id generic-chat-main \
  --bearer <token> \
  --dataset-external-ids <newbai-dataset-external-id>
```

Default live scope skips `template_reuse_guard` cases, because those prompts can
touch static-page routing. Add `--include-template-guards` or explicit
`--case-id ...` only for an approved controlled run. Live mode still records
side-effect signals into the result JSONL so the evaluator can fail if the
answer created or claimed report/static-page output.
