# Static Page Generation Studio Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build static page generation inside the V3 intelligent assistant, matching the original assistant UI 1:1 on desktop and mobile, while allowing users to drive planning, layout, data binding, style choice, image preview, and final page generation mainly through natural language.

**Architecture:** The static page flow is not a separate visual product and not a revival of the old static workbench. It becomes a first-class assistant capability embedded inside the assistant page: chat is the control surface, the right insight/result area is the desktop planning and building surface, and mobile enters an in-page static page build mode that keeps the original assistant shell while using a vertical module builder. Draft state is schema-first, model-operated, and later persisted through V3 Rust APIs and queued image/static-page workers.

**Tech Stack:** Next.js 16 / React 19 in `apps/web`, existing `/api/v3/*` proxy, Rust `platform-api`, `contracts`, `domain-model`, `storage`, future static-page runtime/worker/renderer crates, `react-grid-layout` for desktop planning canvas, `@dnd-kit/core` and `@dnd-kit/sortable` for mobile vertical ordering, `@puckeditor/core` as a future component-render adapter, `recharts` for first static charts, optional Apache ECharts for advanced charts, Cloudflare/Codex image queue integration.

---

## Current Plan Audit

I checked `docs/plans/2026-04-27-static-page-generation-studio-plan.md` on 2026-04-27.

Finding:

- The file was still untracked in git, so there is no committed baseline to diff against.
- Its timestamp was `2026-04-27 14:06:04`, after the first draft work.
- The content was not the latest intended direction: it still said `Isolated Popup Studio Route`, `Hard-Coded Style Presets`, and `clean single-purpose generation studio`.
- It did not include the latest decisions: original assistant UI 1:1, mobile parity, model-operated global editing, `react-grid-layout`, `dnd-kit`, Puck capability, commercial-risk notes, and replacing hard-coded styles with three business style directions.

Conclusion:

- Treat the previous file content as an outdated draft, whether caused by another thread or by an earlier unsynced write.
- This document is now the authoritative merged plan.

## Locked Product Decisions

- UI shell must visually match the original `ai-data-platform` intelligent assistant, including desktop and mobile.
- Do not make the static page studio look like a separate SaaS editor, CMS, or admin form.
- Do not resurrect the old static page visual workbench.
- Do not make copied form filling the main interaction.
- Users should describe changes in natural language; the model translates them into draft operations.
- Direct manipulation exists only where it is natural: desktop module drag/resize, mobile vertical module ordering, and light inline corrections.
- Desktop planning and static page construction live inside the assistant page's right-side insight/result area.
- Mobile must also build the static page inside the assistant page. It is not read-only and not just a status card.
- Mobile uses an in-page `静态页构建` mode: vertical module composition, module order changes, content/data/chart summaries, natural-language edits, queue state, preview confirmation, and final render status.
- The earlier "single clean popup page" idea is superseded by the newer 1:1 UI requirement. If a separate window is later needed, it must still use the same original assistant shell language and not introduce a new product skin.
- The "write fixed styles" wording is removed. The product offers three curated style directions that can be model-selected or user-confirmed.

## Original Assistant UI Contract

Use the original project at `C:\Users\soulzyn\Desktop\codex\ai-data-platform` as the visual source of truth.

Reference assets already captured:

- `docs/prototypes/original-assistant-desktop.png`
- `docs/prototypes/original-assistant-mobile.png`
- `docs/prototypes/static-page-studio-original-shell-desktop.png`
- `docs/prototypes/static-page-studio-original-shell-mobile.png`

Desktop contract:

- Keep the dark top toolbar gradient and brand block.
- Keep the AI square logo, `智能助手` identity, navigation pills, and status pills.
- Keep the dark left dataset rail.
- Keep the center chat panel as the primary natural-language control surface.
- Keep the right result panel sparse, dark, and task-oriented.
- Static page planning should feel like a new result mode inside the original assistant, not a modal editor.

Mobile contract:

- Keep the fixed full-screen dark assistant shell.
- Keep the compact topbar identity.
- Keep chat as the default single visible surface.
- Keep bottom composer behavior.
- Keep dataset/result surfaces as drawer or switched panels.
- Static page construction appears as an in-page mobile build panel, entered from an assistant message or a top/bottom action.
- The mobile build panel shows a live static page structure preview, vertical module list, style direction, generation state, and final render state.
- Mobile only supports up/down reordering for modules; no freeform grid dragging or arbitrary resizing on phone.
- Mobile users can still complete the whole build flow: plan, adjust, confirm style, queue image, confirm effect image, and generate the final static page.

## User Flow

Desktop:

1. User chats normally in the intelligent assistant.
2. User asks for a static page, or clicks the persistent `一键生成静态页` action.
3. Assistant creates a `StaticPageDraft` from the current dataset, selected session, and conversation evidence.
4. Chat returns a short explanation of the planned page.
5. Right result panel switches to `静态页规划`.
6. The planning panel shows modules already placed on a 12-column grid.
7. Each module card directly shows title, intended content, data source, and visualization type.
8. User can drag/resize modules on desktop.
9. User can ask natural-language changes in chat, such as `把风险放大一点`, `减少文字`, `换成趋势图`, `整体更像给董事会看的`.
10. Model converts the request into structured operations and refreshes the draft.
11. User confirms one of three style directions.
12. Assistant builds an image prompt payload and submits the queued effect-image job.
13. While waiting, the result panel shows `资源正在排队，可以联系商务开通高级用户跳过等待。`
14. A generated effect image appears for confirmation.
15. User confirms the effect image.
16. Renderer produces a final static page from confirmed modules, chart/data bindings, and style direction.

Mobile:

1. User remains in the original mobile assistant shell.
2. Static page draft starts from chat and opens the in-page `静态页构建` mode.
3. The build mode shows current planning status, live structure preview, style direction, and module list.
4. User can drag modules only vertically to reorder them.
5. User changes content mainly by sending natural-language messages.
6. The model updates the draft and refreshes the mobile build mode.
7. Queue, preview confirmation, and final static page generation all happen in the same mobile build mode.
8. User can return to chat without losing the draft.

## Data Model Draft

Frontend shape:

```js
{
  id: 'draft-local-...',
  datasetId: '...',
  sessionId: '...',
  source: {
    conversationSummary: '...',
    selectedMessageIds: [],
    evidenceIds: []
  },
  status: 'planning',
  objective: '给客户展示当前数据结论，并生成可交付静态页',
  audience: '客户决策层',
  styleDirection: 'decision-brief',
  modelSummary: '模型对当前页面结构的理解',
  mobileOrder: ['hero', 'kpi', 'trend', 'risk', 'next-steps'],
  modules: [
    {
      id: 'hero',
      role: 'hero',
      title: '核心判断',
      content: '先给出一句客户能直接带走的主结论。',
      dataBinding: {
        type: 'conversation_summary',
        label: '来自当前会话摘要',
        sourceId: 'session'
      },
      visualization: {
        type: 'headline',
        label: '大标题 + 关键结论'
      },
      layout: {
        x: 0,
        y: 0,
        w: 12,
        h: 3
      }
    }
  ],
  operations: [],
  imageJob: {
    id: null,
    status: 'idle',
    queuePosition: null,
    queueMessage: ''
  },
  previewImage: null,
  finalPage: null
}
```

Model operation shape:

```js
{
  id: 'op-...',
  type: 'update_module',
  targetModuleId: 'risk',
  reason: '用户要求突出风险',
  patch: {
    title: '主要风险与优先级',
    visualization: { type: 'risk-matrix' },
    layout: { x: 7, y: 3, w: 5, h: 4 }
  }
}
```

Required operation types:

- `update_module`
- `add_module`
- `remove_module`
- `move_module`
- `resize_module`
- `reorder_modules`
- `change_visualization`
- `change_data_binding`
- `change_style_direction`
- `refresh_summary`
- `queue_image_job`
- `confirm_preview`
- `request_final_render`

## Style Directions

Use these as product directions, not hard-coded templates:

- `decision-brief`: 高层决策简报。Dark executive shell, strong conclusion first, KPI and risk emphasis, suited for老板/董事会/甲方决策层.
- `client-delivery`: 客户交付报告。Clean consulting deliverable, more explanation, balanced charts and notes, suited for售前/项目交付/周报月报.
- `data-command`: 数据运营看板。Dense but readable data cockpit, trend/comparison/composition first, suited for运营复盘/经营看板/指标追踪.

Default selection:

- If the user says `给老板`, `决策`, `汇报`, default to `decision-brief`.
- If the user says `给客户`, `交付`, `方案`, default to `client-delivery`.
- If the user says `运营`, `数据`, `指标`, `看板`, default to `data-command`.
- If unclear, model picks one and explains why in one short assistant sentence.

## Visualization Types

First version:

- `headline`: conclusion or hero statement.
- `kpi-cards`: 2-4 key metrics.
- `bar-chart`: category comparison.
- `line-chart`: trend over time.
- `donut-chart`: composition/share.
- `table`: compact evidence table.
- `timeline`: phased roadmap or sequence.
- `risk-matrix`: risk/priority grid.
- `text-insight`: narrative insight block.

Chart layer:

- Use `recharts` for first version because it is React-native, SVG-based, and enough for KPI/bar/line/donut.
- Add Apache ECharts later only for complex dashboards, rich interactions, large-data charts, or advanced composition.

## Open Source Capability Decisions

Dependencies to add in the first implementation slice:

```powershell
pnpm --filter @ai-data-platform-v3/web add react-grid-layout @dnd-kit/core @dnd-kit/sortable @puckeditor/core recharts
```

Commercial and license notes:

- `react-grid-layout`: MIT license. Good direct fit because layout data is `{ x, y, w, h }`, matching `StaticPageDraft.modules[].layout`. No paid requirement for current use.
- `@dnd-kit/core` and `@dnd-kit/sortable`: MIT license. Use for mobile vertical module reordering and accessible drag behavior. No paid requirement.
- `@puckeditor/core`: MIT license. Use carefully as a future page-component config/render adapter. Do not depend on paid Puck AI or hosted services for first version.
- `recharts`: MIT license. Good first chart layer. No paid requirement.
- `echarts`: Apache-2.0 license. Optional later. No paid requirement, but configuration and bundle complexity are higher.

Puck positioning:

- Do not expose Puck as a generic page builder UI in version one.
- Keep our own simple assistant UI.
- Borrow Puck's useful pattern: component config + editable data + independent renderer.
- Add a thin adapter only when the static page component schema stabilizes.

## Implementation Sequence

### Task 0: Protect The Current Direction

**Files:**

- Modify: `docs/plans/2026-04-27-static-page-generation-studio-plan.md`
- Keep reference: `docs/plans/2026-04-25-static-page-visual-workbench-v3-plan.md`
- Keep reference: `docs/prototypes/static-page-studio-original-assistant-shell.html`

**Steps:**

1. Keep this document as the active plan.
2. Keep the old visual workbench plan marked deferred.
3. Do not continue implementing `static-page-studio` as a separate clean popup route unless the user explicitly reverses the 1:1 assistant-shell decision.
4. Use prototype screenshots only as visual references, not as production source.
5. Commit after this plan is accepted.

### Task 1: Match The Original Assistant Shell

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Create: `apps/web/app/components/HomeWorkspaceToolbar.js`
- Create: `apps/web/app/components/HomeMobileShell.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Compare V3 shell with original `C:\Users\soulzyn\Desktop\codex\ai-data-platform\apps\web\app\HomePageClient.js`.
2. Add a top workspace toolbar matching the original desktop toolbar.
3. Keep original nav labels: `智能会话`, `数据集`, `采集源`, `静态页`, `成员`, `审计`.
4. Keep original status pill style for system/data/model status.
5. Move V3 current dataset/status summary into the original toolbar visual language.
6. Add `HomeMobileShell` behavior matching the original mobile assistant: topbar, single active panel, bottom composer, drawer-style dataset/results access.
7. Preserve current V3 data fetching and report-service behavior.
8. Run `pnpm --filter @ai-data-platform-v3/web build`.
9. Manual check desktop at `http://localhost:3100`.
10. Manual check mobile width around `390px`.
11. Commit message: `feat(web): align assistant shell with original UI`.

### Task 2: Add Static Page Draft Model

**Files:**

- Create: `apps/web/app/lib/static-page-draft.js`
- Create: `apps/web/app/lib/static-page-draft.test.mjs`

**Steps:**

1. Add style direction constants.
2. Add visualization type constants.
3. Add `buildInitialStaticPageDraft({ datasetId, sessionId, conversationSummary, evidenceIds })`.
4. Add `applyStaticPageOperation(draft, operation)`.
5. Add `applyStaticPageOperations(draft, operations)`.
6. Add `buildStaticPageImagePayload(draft, { oneClick })`.
7. Add `buildStaticPageFinalRenderPayload(draft)`.
8. Add bounds validation for desktop grid layout.
9. Add mobile order validation.
10. Add tests for initial draft, module update, desktop resize, mobile reorder, style direction change, and image payload.
11. Run `node --test apps/web/app/lib/static-page-draft.test.mjs`.
12. Commit message: `feat(web): add static page draft model`.

### Task 3: Add Assistant Static Page State

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Create: `apps/web/app/components/static-page/StaticPageAssistantNotice.js`

**Steps:**

1. Add `staticPageDrafts` state keyed by dataset/session.
2. Add `activeStaticPageDraftId` state.
3. Add `handleStartStaticPageDraft({ oneClick })`.
4. Add `handleApplyStaticPagePrompt(prompt)` for natural-language updates.
5. Add persistent `一键生成静态页` action near the chat controls, matching original button style.
6. When user asks for a static page in chat, create a draft without requiring a form.
7. Show a compact assistant notice when a draft is created or refreshed.
8. Keep report-service entry behavior unchanged.
9. Run `pnpm --filter @ai-data-platform-v3/web build`.
10. Commit message: `feat(web): add static page assistant state`.

### Task 4: Add Desktop Planning Result Panel

**Files:**

- Modify: `apps/web/app/components/InsightPanel.js`
- Create: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Create: `apps/web/app/components/static-page/StaticPagePlanningCanvas.js`
- Create: `apps/web/app/components/static-page/StaticPageModuleCard.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Add a `静态页规划` card in the right insight/result panel.
2. Use `react-grid-layout` for the desktop planning canvas.
3. Configure a 12-column desktop layout.
4. Render module cards from `draft.modules`.
5. Each card must show title, content summary, data binding, visualization type, and model note.
6. On drag/resize, emit `move_module` and `resize_module` operations.
7. Keep visual styling aligned with original dark result panel.
8. Keep editing controls light; avoid a large form-first editor.
9. Run `pnpm --filter @ai-data-platform-v3/web build`.
10. Commit message: `feat(web): add desktop static page planning panel`.

### Task 5: Add Mobile Static Page Build Mode

**Files:**

- Modify: `apps/web/app/components/HomeMobileShell.js`
- Modify: `apps/web/app/components/ChatPanel.js`
- Create: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Create: `apps/web/app/components/static-page/StaticPageMobileModuleList.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Add a mobile `静态页构建` panel inside the existing assistant page, not a popup and not a separate product skin.
2. Let the user enter the panel from an assistant message, `一键生成静态页`, or the mobile result switcher.
3. Show status, style direction, live structure preview, image preview area, and module list.
4. Use `@dnd-kit/core` and `@dnd-kit/sortable`.
5. Use `verticalListSortingStrategy`.
6. Only allow vertical reordering on mobile.
7. Convert reorder results into `reorder_modules` operations.
8. Show each module's title, content summary, data source, and visualization type.
9. Do not show desktop grid handles on mobile.
10. Keep the original mobile topbar and navigation behavior.
11. Keep natural-language editing available from the mobile composer or an inline prompt bar.
12. Support the complete mobile build flow: planning, style confirmation, queue, effect preview confirmation, and final static page render status.
13. The mobile panel may use a full-height in-page view, but it must preserve a clear return path to chat.
14. Run `pnpm --filter @ai-data-platform-v3/web build`.
15. Manually test touch-like ordering and build-flow progression in browser mobile emulation.
16. Commit message: `feat(web): add mobile static page build mode`.

### Task 6: Add Model-Operated Intent Handling

**Files:**

- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/lib/static-page-draft.test.mjs`
- Modify: `apps/web/app/HomePageClient.js`
- Create: `apps/web/app/components/static-page/StaticPageIntentSummary.js`

**Steps:**

1. Add `interpretStaticPagePromptLocally(draft, prompt)` as a deterministic first slice.
2. Return structured operations, not free text patches.
3. Support Chinese prompts for changing tone, adding data, reducing text, highlighting risk, switching chart type, reordering modules, and changing style direction.
4. Show `模型理解` summary in the static page planning panel and mobile build panel.
5. Let users override by sending another natural-language message.
6. Tests cover at least six prompt examples.
7. Run `node --test apps/web/app/lib/static-page-draft.test.mjs`.
8. Run `pnpm --filter @ai-data-platform-v3/web build`.
9. Commit message: `feat(web): interpret static page edit prompts`.

### Task 7: Add Style Direction Confirmation

**Files:**

- Create: `apps/web/app/components/static-page/StaticPageStyleDirectionPicker.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Show three style directions: `高层决策简报`, `客户交付报告`, `数据运营看板`.
2. Let the model preselect one direction from the user's wording.
3. Let the user confirm or switch direction.
4. Keep the picker compact and visual, not a template marketplace.
5. Persist style choice as `change_style_direction`.
6. Run `pnpm --filter @ai-data-platform-v3/web build`.
7. Commit message: `feat(web): add static page style directions`.

### Task 8: Add Queue And Effect Preview Mock

**Files:**

- Modify: `apps/web/app/lib/static-page-draft.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Add image job states: `idle`, `queued`, `running`, `preview_ready`, `failed`.
2. Add queue copy: `资源正在排队，可以联系商务开通高级用户跳过等待。`
3. Add a deterministic mock preview before real image bytes are connected.
4. Add `确认效果图`, `重新生成`, and `继续修改规划`.
5. Confirmed preview moves draft status to `effect_confirmed`.
6. Run `pnpm --filter @ai-data-platform-v3/web build`.
7. Commit message: `feat(web): add static page image queue mock`.

### Task 9: Add Final Static Page Mock Render

**Files:**

- Create: `apps/web/app/components/static-page/StaticPageFinalRender.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`
- Modify: `apps/web/app/globals.css`

**Steps:**

1. Render a final static page mock from confirmed modules.
2. Use `recharts` for first real chart rendering where data exists.
3. Use placeholders only when data binding has no numeric data yet.
4. Keep final render visually close to the selected style direction.
5. Show clear status that backend renderer is not connected yet.
6. Run `pnpm --filter @ai-data-platform-v3/web build`.
7. Commit message: `feat(web): add static page final render mock`.

### Task 10: Add Rust Domain And Contracts

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`

**Steps:**

1. Add `StaticPageDraftId`, `StaticPageImageJobId`, and `StaticPageRenderOutputId`.
2. Add domain structs for draft, module, operation, image job, and final render output.
3. Add statuses matching frontend draft states.
4. Add contract request/response views.
5. Add storage tables and repository methods.
6. Add tests for draft creation, operation append, image job state transition, and final render output persistence.
7. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model -p contracts -p storage"`.
8. Commit message: `feat(static-page): add draft domain contracts`.

### Task 11: Add Static Page API

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`
- Modify: `apps/web/app/HomePageClient.js`

**Steps:**

1. Add `POST /v1/chat-sessions/{session_id}/static-page-drafts`.
2. Add `POST /v1/datasets/{dataset_id}/static-page-drafts`.
3. Add `GET /v1/static-page-drafts/{draft_id}`.
4. Add `PATCH /v1/static-page-drafts/{draft_id}`.
5. Add `POST /v1/static-page-drafts/{draft_id}/operations`.
6. Add `POST /v1/static-page-drafts/{draft_id}/intent`.
7. Wire frontend to use API when available and local model only as development fallback.
8. Add platform-api tests.
9. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api static_page"`.
10. Run `pnpm --filter @ai-data-platform-v3/web build`.
11. Commit message: `feat(static-page): add draft API`.

### Task 12: Add Provider-Backed Intent Interpretation

**Files:**

- Create: `crates/static-page-runtime/Cargo.toml`
- Create: `crates/static-page-runtime/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add runtime function accepting draft, prompt, dataset context, and conversation evidence.
2. Return `StaticPageDraftOperation[]`.
3. Keep deterministic fallback for tests.
4. Put provider-backed model call behind config.
5. Reject unsafe or unrecognized operations instead of applying freeform JSON blindly.
6. Store model reasoning summary and operation list.
7. Add tests for Chinese prompts and invalid operation rejection.
8. Run `wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p static-page-runtime"`.
9. Commit message: `feat(static-page): add model intent runtime`.

### Task 13: Add Image Queue Integration

**Files:**

- Create: `crates/static-page-worker/Cargo.toml`
- Create: `crates/static-page-worker/src/main.rs`
- Modify: `Cargo.toml`
- Modify: `crates/workflow-definitions/src/lib.rs`
- Modify: `crates/platform-api/src/lib.rs`

**Steps:**

1. Add workflow kind or task payload for `static_page_image_generation`.
2. Add `POST /v1/static-page-drafts/{draft_id}/image-jobs`.
3. Add `GET /v1/static-page-image-jobs/{job_id}`.
4. Build image prompt payload from confirmed modules, style direction, and data bindings.
5. Use the configured Cloudflare/Codex image endpoint.
6. Treat text-only completion with no image artifact as a failed job.
7. Persist queue status, queue position if available, preview asset key, and failure reason.
8. Add queue worker tests with mocked provider.
9. Commit message: `feat(static-page): queue effect image generation`.

### Task 14: Add Preview Confirmation

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/lib/platform-api.js`
- Modify: `apps/web/app/components/static-page/StaticPagePlanningPanel.js`
- Modify: `apps/web/app/components/static-page/StaticPageMobileBuilder.js`

**Steps:**

1. Add `POST /v1/static-page-image-jobs/{job_id}/confirm`.
2. Persist confirmed preview asset key.
3. Block final render until preview is confirmed.
4. Allow `重新生成效果图` to submit a new job from the same draft.
5. Show confirmation state in desktop and mobile build surfaces.
6. Add tests.
7. Commit message: `feat(static-page): confirm effect previews`.

### Task 15: Add Final Static Page Renderer

**Files:**

- Create: `crates/static-page-renderer/Cargo.toml`
- Create: `crates/static-page-renderer/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/platform-api/src/lib.rs`
- Modify: `apps/web/app/components/static-page/StaticPageFinalRender.js`

**Steps:**

1. Add renderer consuming confirmed draft modules, data bindings, style direction, and confirmed preview metadata.
2. Produce final HTML and asset manifest.
3. Render charts from bound data.
4. Add `POST /v1/static-page-drafts/{draft_id}/renders`.
5. Add frontend polling for render output.
6. Add tests that final HTML includes all module titles, data labels, chart placeholders or chart data, and style direction class.
7. Commit message: `feat(static-page): render final static pages`.

## First Recommended Execution Slice

Implement Tasks 1-9 first.

Reason:

- It validates the full customer-visible flow before backend queue and renderer complexity.
- It locks the 1:1 original assistant shell before adding more product logic.
- It gives backend work a concrete draft schema and operation contract.
- It avoids wasting time on a separate popup/editor surface the product direction no longer wants.

Do not start Tasks 10-15 until desktop and mobile static page building both feel correct in the assistant shell.

## Verification Commands

Frontend:

```powershell
node --test apps/web/app/lib/static-page-draft.test.mjs
pnpm --filter @ai-data-platform-v3/web build
```

Rust later:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo check --workspace"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test"
```

Manual acceptance:

- Desktop visually matches the original intelligent assistant shell.
- Mobile visually matches the original mobile assistant shell.
- Static page generation can start from natural-language chat.
- `一键生成静态页` is always reachable but does not dominate the UI.
- Desktop right panel shows planned modules on a grid.
- Desktop modules can be moved and resized.
- Module cards show title, content, data source, and visualization type without opening a form.
- Mobile shows an in-page static page build mode, not just a status card.
- Mobile modules can be reordered vertically.
- Mobile can complete the static page build flow through final render status.
- Natural-language changes update the whole draft through model operations.
- Three style directions are available and model-selectable.
- Queue state shows the business upgrade copy.
- Effect preview must be confirmed before final static page render.
- Final static page is rendered from confirmed modules/data, not from image pixels alone.
