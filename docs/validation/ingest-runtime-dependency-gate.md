# Ingest Runtime Dependency Gate

This gate validates the deployment-target runtime dependency needed for the generic document parsing fallback.

## Contract

- DataMax local ingest parsers remain the primary parsing path.
- MarkItDown is pinned and enabled only as the fallback parser.
- Deployment targets must pass `PYTHON_BIN -m markitdown --version` before MarkItDown fallback parsing is considered ready.
- PaddleOCR PP-StructureV3 is the default PDF parser when `DOCUMENT_PADDLEOCR_PYTHON_BIN` is configured, when the deployment default `/srv/aiv3/venv/paddleocr/bin/python` exists, or when `DOCUMENT_PDF_PARSE_ENGINE=paddleocr_first`. Set `DOCUMENT_PADDLEOCR_ENABLED=false` or `DOCUMENT_PDF_PARSE_ENGINE=native_first` to opt out.
- The dedicated PaddleOCR runtime must be new enough for PP-OCRv6. The default gate requires `paddleocr>=3.7.0`, matching `infra/runtime/paddleocr-requirements.txt`.
- PaddleOCR, rendered OCR, PDF VLM, and presentation VLM parsing no longer apply a default first-pages cap. Use `DOCUMENT_PADDLEOCR_MAX_PAGES`, `DOCUMENT_PDF_OCR_MAX_PAGES`, `DOCUMENT_PDF_VLM_MAX_PAGES`, or `DOCUMENT_PRESENTATION_VLM_MAX_SLIDES` only when a deployment explicitly needs a temporary safety limit. Empty or `0` means full document.
- PaddleOCR readiness is checked when `DOCUMENT_PADDLEOCR_PYTHON_BIN` is configured, `DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN` is configured, the deployment default PaddleOCR venv exists, `DOCUMENT_PADDLEOCR_ENABLED=true`, `DOCUMENT_PDF_PARSE_ENGINE=paddleocr_first`, or `INGEST_GATE_REQUIRE_PADDLEOCR=true`.

## Command

Run on a Linux deployment target:

```bash
bash scripts/run-ingest-runtime-gate.sh
```

The script reads `PYTHON_BIN` from the environment first. If it is not set and `/etc/aiv3/aiv3.env` is readable, it reads the `PYTHON_BIN` entry from that file. It also reads `DOCUMENT_PADDLEOCR_ENABLED`, `DOCUMENT_PDF_PARSE_ENGINE`, `DOCUMENT_PADDLEOCR_PYTHON_BIN`, and `DOCUMENT_PADDLEOCR_DEFAULT_PYTHON_BIN` for optional PaddleOCR readiness checks. If a dedicated `/srv/aiv3/venv/paddleocr/bin/python` exists, the gate uses it for PaddleOCR unless `DOCUMENT_PADDLEOCR_PYTHON_BIN` is already set. The default pinned fallback version is `0.1.5` and can be overridden with `MARKITDOWN_REQUIRED_VERSION`; the default PaddleOCR minimum is `3.7.0` and can be overridden with `PADDLEOCR_REQUIRED_VERSION`.

To force a PaddleOCR package readiness check without relying on env config, run:

```bash
INGEST_GATE_REQUIRE_PADDLEOCR=true bash scripts/run-ingest-runtime-gate.sh
```

The default PaddleOCR check verifies `from paddleocr import PPStructureV3` and rejects PaddleOCR versions older than the PP-OCRv6 runtime requirement. To run a non-customer tiny PDF prediction smoke using PP-OCRv6 medium detection/recognition models, use:

```bash
INGEST_GATE_REQUIRE_PADDLEOCR=true INGEST_GATE_PADDLEOCR_SMOKE=true bash scripts/run-ingest-runtime-gate.sh
```

The smoke may download and cache official PaddleOCR models on first run.

The current dedicated sidecar dependency set is pinned in:

```text
infra/runtime/paddleocr-requirements.txt
```

On `8服务器`, use `/srv/aiv3/venv/paddleocr/bin/python` for PaddleOCR and keep `/srv/aiv3/venv/media/bin/python` for MarkItDown/media parsing. The first PP-StructureV3 / PP-OCRv6 prediction downloads and caches official models under `/root/.paddlex/official_models`.

Reports are written under:

```text
target/ingest-runtime-gate/
```

## Current Deployment Result

- Target: `8服务器`
- Service Python: `/srv/aiv3/venv/media/bin/python`
- Installed fallback: `markitdown 0.1.5`
- Service state checked: `aiv3-ingest-worker.service` active

Future A/B parsing quality comparisons for DOCX/PDF/PPTX should run as a separate evidence smoke using representative documents. That comparison should inspect headings, paragraph order, table retention, chunk quality, and retrieval usefulness before moving any individual format ahead of the current DataMax parser chain.
