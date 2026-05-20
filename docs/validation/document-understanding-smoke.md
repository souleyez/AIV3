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

## 2026-05-20 Deployment Target Evidence

- Host: `8服务器`
- Repository: `/srv/aiv3/repo`
- HEAD: `d3bf97a`
- Command: `bash scripts/run-document-understanding-smoke.sh`
- Result: passed
- JSON report: `/srv/aiv3/repo/target/document-understanding-smoke/document-understanding-smoke-20260520T081746Z.json`
- Markdown summary: `/srv/aiv3/repo/target/document-understanding-smoke/document-understanding-smoke-20260520T081746Z.md`
