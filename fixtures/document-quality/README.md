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
| Scanned or visual PDF | A visual/scanned document that needs OCR or VLM fallback | `这份扫描件的主要内容是什么？` | PaddleOCR is preferred when configured; MiniMax VLM is used only as fallback/rescue. |

## Manifest

`smoke-cases.json` is the machine-readable case list consumed by `scripts/run-document-quality-smoke.ps1`. Real binary paths can be supplied at runtime through environment variables or script parameters; do not commit private customer documents here.
