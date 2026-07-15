> **ARCHIVED 2026-07-16 — NON-EXECUTABLE:** 仅作历史设计与验收证据；禁止继续 Task、继承 approval/基线/开关或执行部署命令。唯一活动入口为 `docs/plans/datamax-active-execution-plan.md`。

# Dataset Understanding Explainability Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 让数据集解析五阶段可以逐项钻取真实结果，并让知识图谱通过核心概念、结构主线和可解释关系体现系统对数据的理解。

**Architecture:** 在 `dataset-understanding-graph` 模型层为五个处理阶段生成来源明确的详情项，同时生成只由现有接口字段汇总的理解摘要。React 图谱组件把现有右侧证据栏升级为统一的理解检查器：默认展示理解摘要，点击阶段展示阶段数据，点击节点或关系展示连接对象、关系类型、置信度和证据。

**Tech Stack:** Next.js、React、Apache ECharts、Node.js test runner、CSS。

---

### Task 1: 真实阶段详情模型

**Files:**
- Modify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Modify: `apps/web/app/lib/dataset-understanding-graph.js`

1. 添加失败测试，要求五个阶段都包含 `source`、`summary` 和 `items`。
2. 断言资料、解析和检索详情来自当前数据集文档；结构来自 `section_title_hints` 与 `document_understanding_strategies`；知识来自 `noun_term_hints`。
3. 实现最小模型转换，并在无明细时明确返回摘要来源而不伪造记录。
4. 运行 `node --test app/lib/dataset-understanding-graph.test.mjs`，预期全部通过。

### Task 2: 可验证的理解摘要

**Files:**
- Modify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Modify: `apps/web/app/lib/dataset-understanding-graph.js`

1. 添加失败测试，要求理解摘要包含核心概念、技术标识、结构主线和检索覆盖。
2. 使用确定性规则将中文业务概念优先展示，将短英文/代码式词条标为技术标识；不推断新的业务事实。
3. 提升有业务语义的知识节点视觉权重，降低技术标识权重。
4. 运行聚焦测试确认通过。

### Task 3: 右侧理解检查器

**Files:**
- Modify: `apps/web/app/components/DatasetUnderstandingGraph.js`

1. 将五个阶段卡片改为可点击按钮，并维护当前检查器模式。
2. 默认展示真实理解摘要；点击阶段后显示该阶段的完整明细、来源和状态。
3. 点击图谱节点时展示所有直接连接对象，标明事实/推断、关系名称和证据；点击连线继续显示单条关系依据。
4. 保留空状态和接口未返回字段的诚实提示。

### Task 4: 图谱理解层次与视觉优化

**Files:**
- Modify: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Modify: `apps/web/app/globals.css`

1. 在图谱顶部加入“核心概念 / 结构主线 / 检索覆盖”理解带。
2. 扩大右侧检查器宽度，增加阶段选中态、详情清单和连接关系卡片。
3. 让业务概念节点更醒目、技术标识更克制，并在关系强调态展示关系名称。
4. 保持 1180px 和 760px 断点下的单列响应式布局。

### Task 5: 验证

**Files:**
- Verify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Verify: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Verify: `apps/web/app/globals.css`

1. 运行聚焦模型测试。
2. 运行 Web 全量测试和生产构建。
3. 使用真实公开数据集验证五个阶段点击内容、节点连接说明、无重复列表、无资产库模块和图谱可读性。
4. 截图检查主视觉、右侧检查器和节点文字是否清晰。
