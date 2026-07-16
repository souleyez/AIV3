> **ARCHIVED 2026-07-16 — NON-EXECUTABLE:** 仅作历史设计与验收证据；禁止继续 Task、继承 approval/基线/开关或执行部署命令。唯一活动入口为 `docs/plans/datamax-active-execution-plan.md`。

# Dataset Understanding Graph MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Show an interactive, evidence-based dataset understanding graph on the right side of the dataset page whenever a dataset is selected.

**Architecture:** Build a deterministic frontend view model from the existing dataset summary and document list responses. Render that model with the already-installed ECharts graph runtime, alongside a compact processing pipeline and a selected-node evidence inspector; do not add APIs or invent inferred business relationships.

**Tech Stack:** Next.js 16, React 19, ECharts 6, Node test runner, CSS.

---

### Task 1: Deterministic graph model

**Files:**
- Create: `apps/web/app/lib/dataset-understanding-graph.js`
- Test: `apps/web/app/lib/dataset-understanding-graph.test.mjs`

**Step 1: Write failing model tests**

Cover dataset document scoping, deduplicated knowledge hints, pipeline counts, and the rule that missing fields do not generate fabricated knowledge nodes.

**Step 2: Run the focused test and verify failure**

Run: `node --test app/lib/dataset-understanding-graph.test.mjs` from `apps/web`.

Expected: FAIL because the model module does not exist.

**Step 3: Implement the minimal model**

Map the selected dataset plus its existing documents into five evidence categories: documents, knowledge terms, section structure, material types, and understanding strategies. Limit noisy categories deterministically and expose metrics, pipeline stages, graph nodes, graph links, and human-readable node evidence.

**Step 4: Run the focused test**

Expected: all focused model tests pass.

### Task 2: Dataset understanding panel

**Files:**
- Create: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify: `apps/web/app/globals.css`

**Step 1: Add the panel component**

Render a selected-dataset header, five-stage processing strip, ECharts force graph, category filters, and a node evidence inspector. Lazy-load ECharts in the browser, resize with its container, dispose on unmount, and retain a text fallback for accessibility or chart-loading failure.

**Step 2: Integrate it into the right dataset column**

Place the panel above document search/list. Feed it only the currently selected dataset and documents that actually belong to that dataset. Show an explicit selection prompt when no dataset is selected.

**Step 3: Add responsive visual styling**

Use the current dark DataMax language with a restrained cyan/amber signal palette. Keep the graph useful at desktop width, collapse the evidence inspector below the graph on narrower screens, and avoid expanding the page beyond the existing content column.

### Task 3: Verification

**Files:**
- Modify only if verification finds defects.

**Step 1: Run the focused model tests**

Run: `node --test app/lib/dataset-understanding-graph.test.mjs` from `apps/web`.

**Step 2: Run the complete frontend model suite**

Run: `pnpm --filter @ai-data-platform-v3/web test` from the repository root.

**Step 3: Run the production build**

Run: `pnpm --filter @ai-data-platform-v3/web build` from the repository root.

**Step 4: Inspect the live page visually**

Start the existing Web development surface, select a populated dataset, and verify: the graph changes with selection, only real response fields appear, node clicks update evidence, zero-data states remain honest, and the document list stays usable below the graph.
