# V3 HTML Artifact Skill Evaluation

Date: 2026-05-15

## Decision

V3 should turn the current HTML documentation practice into a reusable capability, but not by replacing Markdown globally.

The right shape is:

- Keep Markdown or structured JSON as the editable source of truth.
- Generate self-contained HTML as the human-facing review, handoff, report, diagram, or lightweight editor surface.
- Treat HTML as a V3-owned artifact template output, not arbitrary model HTML.
- Add a Codex skill later only as an authoring workflow helper; product safety must live in V3 templates, validators, sandboxing, and artifact manifests.

## References Checked

- `HTML Effectiveness`: https://thariqs.github.io/html-effectiveness/
- `dogum/html-artifacts`: https://github.com/dogum/html-artifacts

The `HTML Effectiveness` examples show why a single `.html` file is often easier to review than a long Markdown wall for planning, code review, diagrams, decks, reports, research, and custom editors.

The `dogum/html-artifacts` repository is an Apache-2.0 Claude skill that operationalizes this as a recognition heuristic: use HTML when layout, visual hierarchy, diagrams, interaction, or round-trip editing help; keep Markdown for short replies, simple notes, code-only output, and terminal-style answers.

## Why This Fits V3

V3 already has the product boundary this pattern needs:

- Safe HTML artifact manifests.
- Sandboxed rendering.
- Trusted V3 templates.
- Redacted payloads.
- JSON patch or action-intent submission instead of direct DOM/database mutation.
- Handoff packages and evidence manifests for third-party delivery.

The new pure third-party guide HTML proved the pattern for external delivery: the Markdown source remains easy to edit, while the HTML version is easier for business and technical reviewers to scan.

## What Should Become Reusable

### Product Capability

Build a small family of trusted templates for:

- Observability-published third-party API guides.
- Codex execution summaries.
- Static-page planning handoff pages.
- Data-quality reports.
- Code review and repository inventory pages.
- Lightweight review editors that export JSON patch or Markdown.

Each template should have:

- Structured input schema.
- Sanitized rendering.
- No remote scripts or remote CSS.
- No provider secrets, raw tokens, private paths, or queue credentials.
- A freshness/check mode when generated from source files.
- Screenshot or smoke validation for important externally shared pages.

### Codex Skill

Create or adopt a local skill only after the product-side template rules are stable.

The skill should teach the agent:

- When HTML is appropriate.
- When Markdown is still better.
- Which V3 template family to use.
- How to preserve source-of-truth files.
- How to validate generated HTML before committing or shipping.

The skill must not say "always output HTML". It should be a recognition and workflow skill.

## Open-Source Integration Stance

Do not vendor or install `dogum/html-artifacts` directly into V3 yet.

Recommended path:

1. Use its category taxonomy and carve-out framing as a reference.
2. Create a V3-specific skill or project guideline that reflects V3's safety rules.
3. Keep any copied wording or code out unless license review and attribution are completed.
4. If a future local Codex skill is installed from GitHub, vet it first and prefer a forked/trimmed internal version.

## Immediate Follow-Up

- Add HTML freshness checks for every committed generated HTML document.
- Keep the third-party API guide renderer as the current external-document path: Markdown remains the source of truth, generated HTML is committed for review, and Web public copies are the only published customer-facing links.
- Later, create a local `v3-html-artifacts` Codex skill that points to this architecture note and the safe HTML artifact rules.
