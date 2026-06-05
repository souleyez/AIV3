# Xinbai Functional Modular Monthly Template

This directory records the accepted DataMax template contract for the Xinbai/New World monthly business report.

The published artifact id is:

```text
xinbai-functional-modular-template-20260604
```

The template is the only accepted default template for matching Xinbai business-report requests. DataMax should reuse this template with refreshed data and a prompt-derived focus query unless the customer explicitly asks for a redesign.

## Source Contract

- Contract: `template-contract.json`
- Default local artifact: `target/database-static-pages/xinbai-functional-modular-template-20260604`
- Default public URL: `https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html`

The contract is intentionally structural. It verifies the accepted modules, export files, manifest features, and required data arrays without treating generated HTML as the only source of truth.

## Required Capabilities

- Compact monthly filter with date range, district, store, category, and operation mode.
- Prompt focus reordering for overview, take-high opportunity, low-activity risk, and category/business structure requests.
- Business health overview with income, efficiency, take-high, low-activity, and missing traffic markers.
- Health score table with district default view, expandable stores, and average rent-sales ratio as display-only metadata.
- Merged take-high list sorted by distance to the take-high line.
- Store opportunity/risk structure pie chart.
- Rent-sales ratio health pie chart.
- Risk panel with latest low-activity, persistent low-activity, and traffic-drop warning states.
- Export surface: `table-data.csv`, `report.ppt`, and `report.md`.

## Validation

Run local validation:

```powershell
npm run validate:xinbai-report-template
```

Run public artifact validation:

```powershell
npm run validate:xinbai-report-template -- --public-url https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-functional-modular-template-20260604/index.html
```

If this contract fails, fix the artifact or generator before changing routing to prefer the template.
