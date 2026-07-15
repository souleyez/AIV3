> **ARCHIVED 2026-07-16 — NON-EXECUTABLE:** 仅作历史设计与验收证据；禁止继续 Task、继承 approval/基线/开关或执行部署命令。唯一活动入口为 `docs/plans/datamax-active-execution-plan.md`。

# Dataset Understanding Network Graph Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the dataset understanding MVP from a dataset-centered star into an evidence-aware network with factual and explicitly inferred cross-node relationships.

**Architecture:** Keep the frontend-only boundary and extend the deterministic graph model with typed links. Directly observed links, such as document membership and document content type, remain solid; low-risk relationships derived from list co-occurrence or label affinity are dashed and carry confidence plus evidence text. ECharts renders both types and the evidence panel can inspect nodes or edges.

**Tech Stack:** Next.js client component, JavaScript graph model, Apache ECharts force graph, Node test runner, CSS.

---

### Task 1: Relationship model

**Files:**
- Modify: `apps/web/app/lib/dataset-understanding-graph.js`
- Test: `apps/web/app/lib/dataset-understanding-graph.test.mjs`

**Step 1: Write failing tests**

Add assertions for document-to-material factual links, knowledge co-occurrence links, lexical cross-category links, confidence/evidence metadata, deduplication, and empty-state relation counts.

**Step 2: Run the focused test**

Run: `pnpm --filter @ai-data-platform-v3/web exec node --test app/lib/dataset-understanding-graph.test.mjs`

Expected: FAIL because the graph currently emits only dataset-centered links.

**Step 3: Implement the minimal relationship builder**

Add typed link construction, stable deduplication, content-type normalization, label affinity, sparse co-occurrence rings, and relation metrics. Do not claim an inferred edge is factual.

**Step 4: Run the focused test**

Expected: PASS.

### Task 2: Relationship visualization and evidence

**Files:**
- Modify: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Modify: `apps/web/app/globals.css`

**Step 1: Render typed edges**

Use solid lines for observed links and dashed lines for inferred links. De-emphasize dataset membership spokes so cross-node relationships dominate visually.

**Step 2: Add relationship controls**

Add all/factual/inferred filters, a compact legend, relation counts, and edge click handling.

**Step 3: Extend the evidence inspector**

Show source node, target node, relation label, confidence, evidence source, and explicit inference wording for clicked edges.

### Task 3: Verification

**Files:**
- Verify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Verify: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Verify: `apps/web/app/globals.css`

**Step 1: Run focused and full tests**

Run: `pnpm --filter @ai-data-platform-v3/web exec node --test app/lib/dataset-understanding-graph.test.mjs`

Run: `pnpm --filter @ai-data-platform-v3/web test`

Expected: all tests pass.

**Step 2: Run production build**

Run: `pnpm --filter @ai-data-platform-v3/web build`

Expected: production build succeeds with no new warnings.

**Step 3: Perform browser verification**

Open the dataset page with representative existing-field data and verify that non-root edges are visible, inferred edges are dashed, filters work, and clicking a relation updates the evidence panel.
