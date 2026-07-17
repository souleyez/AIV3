# Data Source Live View Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Build a simple, human-facing data-source view with a right-side access-method rail, local display aliases, real field/count summaries, and recent-change refresh.

**Architecture:** Add a pure view-model module that aggregates existing dataset and document summaries. Keep display aliases in browser storage only, render the existing database status/schema data more completely, and reuse the existing document refresh callback on a low-frequency timer.

**Tech Stack:** Next.js client components, React hooks, plain CSS, Node test runner.

---

### Task 1: Source view model

**Files:**
- Create: `apps/web/app/lib/source-workspace-view-model.js`
- Create: `apps/web/app/lib/source-workspace-view-model.test.mjs`

**Step 1: Write failing tests**

Cover dataset grouping, document deduplication, latest-change sorting, field-hint limits, parse counts, and recent-change counts.

**Step 2: Run the test and verify failure**

Run: `node --test apps/web/app/lib/source-workspace-view-model.test.mjs`

Expected: FAIL because the module does not exist.

**Step 3: Implement the minimal pure helpers**

Export `buildConnectedSourceCards`, `sourceDisplayName`, and field/time normalization helpers without browser or React dependencies.

**Step 4: Run the test and verify pass**

Run: `node --test apps/web/app/lib/source-workspace-view-model.test.mjs`

Expected: all tests pass.

### Task 2: Data-source page behavior

**Files:**
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify: `apps/web/app/HomePageClient.js`

**Step 1: Add the source-card rendering**

Replace the content-type document dump with dataset-backed source cards. Show local alias editing, original source name, counts, field chips, parse status, and recent changed documents.

**Step 2: Add low-frequency live refresh**

Pass the existing `onRefreshDocuments` callback into `SourcesPage`, refresh every 60 seconds while visible, and show the real last-check/loading state.

**Step 3: Complete database field/status rendering**

Render schema column names and types after schema inspection. Poll database status only while a real sync run is queued or running.

**Step 4: Run focused tests**

Run: `node --test apps/web/app/lib/source-workspace-view-model.test.mjs apps/web/app/lib/database-source.test.mjs apps/web/app/lib/external-integrations.test.mjs`

Expected: all tests pass.

### Task 3: Right-side access rail and responsive styling

**Files:**
- Modify: `apps/web/app/globals.css`

**Step 1: Implement desktop layout**

Use a two-column source workspace with a flexible main area and a narrow sticky right rail. Keep access methods in one vertical column.

**Step 2: Style source cards and real activity states**

Add restrained industrial dashboard styling for aliases, field chips, metrics, timelines, and real loading/sync pulses.

**Step 3: Implement responsive fallback**

At existing breakpoints, collapse to one column with no sticky positioning and keep controls usable.

**Step 4: Build the Web app**

Run: `corepack pnpm --filter web build`

Expected: Next.js build completes successfully.

### Task 4: Visual verification and deployment

**Files:**
- No additional source files expected.

**Step 1: Verify locally**

Open the data-source page, check desktop and narrow widths, edit and reset a display alias, inspect fields, and confirm timestamps refresh without fake progress.

**Step 2: Review the diff**

Run: `git diff --check` and `git status --short`.

Expected: no whitespace errors and only intended files changed.

**Step 3: Publish through the established V3 path**

Commit the isolated branch, publish it, merge through the repository workflow, then deploy the exact merged SHA using the 8-server local gate.

**Step 4: Verify production**

Confirm services are healthy and `https://doc.elepcloud.com` displays the new data-source layout and behavior.
