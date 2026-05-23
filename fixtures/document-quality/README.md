# Document Quality Fixtures

This directory tracks the repeatable smoke cases for the V3 parsing and answer-quality thread. The fixtures are intentionally small and may point to externally supplied files when a real PDF/DOCX binary cannot be committed.

## Required Smoke Cases

| Case | Fixture expectation | Primary prompt | Expected signal |
| --- | --- | --- | --- |
| One-character PDF | A PDF whose local text extraction returns only one meaningful character, such as `字` | `这份 PDF 能正常解析吗？` | Parse quality is low/degraded; the answer must not treat the single character as document content. |
| Third-party DOC/DOCX | A DOC/DOCX containing enough context to answer who `邓工` is | `邓工是谁？` | The answer uses selected-document evidence and gives a direct model-authored answer. |
| Resume company statistics | A resume set with company names across multiple candidate files | `简历库里一共提到了多少个公司名？` | Company rows are counted from structured scan rows, with document refs available. |
| Multi-dimension resume ranking | Resume profiles with company, skills, projects, age, gender, school, city, certificate, and time range | `简历按技能数量排序出表` / `简历按公司数排序出表` / `简历按工作年限排序出表` | Sorted candidate tables are produced from `resume_profile_rows`. |
| Table-heavy document | PDF or DOCX with visible rows/cells/headings | `列出文档里的表格结构` | Table/section/entity counts are visible to the model. |
| Attendance XLSX date and work-hour formatting | An attendance workbook with Excel serial dates, first/last punch timestamps, blank punch cells, and work-hour values | `最近有没缺勤的人？工时最长和最短分别是谁？` | Dates render as `YYYY-MM-DD`, punch times render as `HH:MM`, blank cells stay aligned, and work hours keep an hour unit. |
| Frequent attendance absence and work-hour query | The high-frequency attendance workbook query covering absence plus longest/shortest work hours | `这份考勤表里最近有没缺勤的人？工时最长和最短分别是谁？请按日期、员工、班次、工时出表。` | Deterministic row analysis provides absence candidates and longest/shortest work-hour rows with normalized dates and hour units. |
| Scanned or visual PDF | A visual/scanned document that needs OCR or VLM fallback | `这份扫描件的主要内容是什么？` | PaddleOCR is preferred when configured; MiniMax VLM is used only as fallback/rescue. |
| Smart home dissatisfied-customer answer quality | A recent smart-home document, transcript, or customer-feedback material where weak partial-parse wording previously hurt answer quality | `最近的智能家居材料客户不满意，重新回答智能家居系统有哪些功能。` | The answer-quality gate retries weak partial-parse language and the final answer avoids raw parser status. |
| Smart elevator point-list answer quality | A smart elevator/elevator-control material with floors, locations, devices, or point lists | `智能梯控/电梯点位有哪些？请按楼层和位置出表。` | The prompt is treated as structured entity/table work and table-shaped output is required when evidence exists. |

## Manifest

`smoke-cases.json` is the machine-readable case list consumed by `scripts/run-document-quality-smoke.ps1`. Real binary paths can be supplied at runtime through environment variables or script parameters; do not commit private customer documents here.

The smoke report records the quality-gate observability fields needed by the V3 answer-quality loop: original parse status, parse-quality status, quality-gate retry reason, retry attempts, ReAct actions, premium action status, final sanitizer leak check, final answer excerpt, evidence/source refs, and failure reason.

Convenience entrypoint:

```powershell
.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case attendance_final
.\scripts\run-v3-quality-gate-smoke.ps1 -Local -Case resume_company_stats
.\scripts\run-v3-quality-gate-smoke.ps1 -ListCases
```

Aliases accepted by the wrapper include `low_text_pdf_final`, `deng_engineer`, `resume_company_stats`, `resume_ranking_table`, `attendance_final`, `attendance_frequent`, `attendance_hot`, `smart_home`, and `smart_elevator`.

## Server Smoke

Server mode is config driven so private 8-server document ids and dataset ids do not need to be committed. Without `-ServerCaseConfigPath`, server mode only records skipped cases and does not send requests.

Example:

```powershell
.\scripts\run-v3-quality-gate-smoke.ps1 `
  -BaseUrl https://example.internal `
  -BearerToken $env:V3_SMOKE_BEARER_TOKEN `
  -ServerCaseConfigPath .\target\document-quality-smoke\server-cases.private.json `
  -Case attendance_final
```

Private config shape:

```json
{
  "cases": {
    "attendance-xlsx-date-format": {
      "selected_scope": {
        "mode": "selected",
        "documents": [
          {
            "type": "document",
            "id": "00000000-0000-0000-0000-000000000000"
          }
        ]
      },
      "scope_candidates": []
    }
  }
}
```

Each case entry may also use `selectedScope`, `scopeCandidates`, `startupBriefing`, `contextPolicyHint`, `currentArtifact`, and `messages`. Server configs may override assertion fields such as `required_final_answer_terms`, `requires_table`, `requires_normalized_dates`, and `failure_markers` for real private materials whose exact row values differ from committed fixtures. Set `allow_auto_scope` to `true` only when the server should intentionally choose scope from visible datasets.
