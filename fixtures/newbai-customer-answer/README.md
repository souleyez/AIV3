# NewBai Customer Answer Fixture

This fixture is for deterministic, offline evaluation of NewBai customer-facing
answer quality. It does not call DataMax, does not call a model provider, and
does not publish report pages.

The cases cover:

- NewBai business-answer prompts that previously depended on retrieval ranking.
- Template reuse requests where an existing dataset template should be reused.
- Ordinary non-NewBai questions that must not be routed into report generation.
- Mixed database and document answers where structured rows and document
  narrative must be used together.

Result JSONL supplied to the evaluator should use one object per line:

```json
{"case_id":"newbai-customer-001","answer":"...","evidence":[{"type":"retrieval_evidence","source":"...","text":"..."}],"artifacts":[],"side_effects":{"static_page_published":false,"report_generation_enqueued":false,"new_template_generated":false}}
```

If no result JSONL is supplied, the evaluator uses the fixture's `sample_result`
objects as a self-test corpus.
