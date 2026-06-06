# Document Understanding Smoke

This smoke validates the current P0 document-understanding contract for third-party document ranges, parser quality, entity extraction, and model-visible parse status.

## Command

```bash
bash scripts/run-document-understanding-smoke.sh
```

The script writes JSON and Markdown reports under:

```text
target/document-understanding-smoke/
```

## Current Coverage

- PaddleOCR parser contract and one-character PDF low-quality detection.
- MiniMax VLM rescue selection for weak unstructured PDF text when configured.
- Structure-aware chunking, parser section preservation, paragraph samples, and candidate noun terms.
- External parse request/detail compatibility for Java/camelCase callers.
- Third-party `documentExternalId` source inference and unresolved-document model visibility.
- Conversation-scoped temporary document ranges that do not move canonical documents.
- Selected external DOC/DOCX evidence supply for short-name questions such as “邓工是谁”.
- Parse status supply for failed, reparsing, degraded, and selected-range documents.
- Resume/company and general typed entity scan behavior.
- Background enrichment coverage for table structures, elderly-care procedure steps and time thresholds, entity terms, resume profiles, and attendance/spreadsheet metrics.

## Answer-Quality Autofix Escalation

Low-quality answer collection and fixed Codex patch proposal are covered by the local fixed-template smoke:

```powershell
.\scripts\run-cloudflare-codex-fixed-task-smoke.ps1 -Local -PlanOnly -Case answer_quality_autofix,human_exception,runtime_summary
```

This smoke validates only the bounded task package, output validation, human-exception audit, and runtime summary. It does not re-enable a blocking answer gate, deploy changes, or broaden document/data permissions.

## 2026-06-06 Local Background Enrichment Evidence

- Command: `powershell -ExecutionPolicy Bypass -File .\scripts\run-document-quality-smoke.ps1 -Local`
- Result: passed
- JSON report: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.json`
- Markdown summary: `target/document-quality-smoke/document-quality-smoke-20260606T005235Z-6760.md`
- Added coverage includes:
  - elderly-care procedure and threshold facts;
  - table-structure enrichment;
  - resume-profile enrichment;
  - attendance/spreadsheet metric enrichment.

## 2026-05-20 Deployment Target Evidence

- Host: `8服务器`
- Repository: `/srv/aiv3/repo`
- HEAD: `d3bf97a`
- Command: `bash scripts/run-document-understanding-smoke.sh`
- Result: passed
- JSON report: `/srv/aiv3/repo/target/document-understanding-smoke/document-understanding-smoke-20260520T081746Z.json`
- Markdown summary: `/srv/aiv3/repo/target/document-understanding-smoke/document-understanding-smoke-20260520T081746Z.md`
