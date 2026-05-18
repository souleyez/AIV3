# Ingest Runtime Dependency Gate

This gate validates the deployment-target runtime dependency needed for the generic document parsing fallback.

## Contract

- V3 local ingest parsers remain the primary parsing path.
- MarkItDown is pinned and enabled only as the fallback parser.
- Deployment targets must pass `PYTHON_BIN -m markitdown --version` before MarkItDown fallback parsing is considered ready.

## Command

Run on a Linux deployment target:

```bash
bash scripts/run-ingest-runtime-gate.sh
```

The script reads `PYTHON_BIN` from the environment first. If it is not set and `/etc/aiv3/aiv3.env` is readable, it reads only the `PYTHON_BIN` entry from that file. The default pinned fallback version is `0.1.5` and can be overridden with `MARKITDOWN_REQUIRED_VERSION`.

Reports are written under:

```text
target/ingest-runtime-gate/
```

## Current Deployment Result

- Target: `8服务器`
- Service Python: `/srv/aiv3/venv/media/bin/python`
- Installed fallback: `markitdown 0.1.5`
- Service state checked: `aiv3-ingest-worker.service` active

Future A/B parsing quality comparisons for DOCX/PDF/PPTX should run as a separate evidence smoke using representative documents. That comparison should inspect headings, paragraph order, table retention, chunk quality, and retrieval usefulness before moving any individual format ahead of the current V3 parser chain.
