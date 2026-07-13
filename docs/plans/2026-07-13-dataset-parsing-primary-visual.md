# Dataset Parsing Primary Visual Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task by task.

**Goal:** 将数据集解析图谱提升为数据集页面主视觉，移除页面内重复的数据集列表和资产库模块，并让所有图谱节点常显不超过 5 个字的主要标签。

**Architecture:** 保留工作区左侧全局数据集选择作为唯一选择入口；数据集页主体改为纵向结构，上方为全宽解析图谱，下方为文档管理。节点短标签在图谱模型层统一生成，ECharts 只负责展示，确保标签规则可测试且完整名称仍保留在提示和证据面板中。

**Tech Stack:** Next.js、React、Apache ECharts、Node.js test runner、CSS。

---

### Task 1: 节点短标签契约

**Files:**
- Modify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Modify: `apps/web/app/lib/dataset-understanding-graph.js`

1. 添加失败测试，要求所有节点包含非空 `shortLabel`，且 Unicode 字符数不超过 5。
2. 实现统一短标签生成，去除常见文件扩展名和空白后截取前 5 个字符。
3. 运行聚焦测试确认通过。

### Task 2: 图谱常显标签与主视觉尺寸

**Files:**
- Modify: `apps/web/app/components/DatasetUnderstandingGraph.js`
- Modify: `apps/web/app/globals.css`

1. 所有节点启用常显标签，并使用模型层的 `shortLabel`。
2. 保留完整节点名称用于悬浮提示和右侧证据详情。
3. 扩大图谱画布及证据栏，使其成为页面首屏主视觉，并保留窄屏响应式布局。

### Task 3: 数据集页面去重与主次重排

**Files:**
- Modify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Modify: `apps/web/app/globals.css`

1. 移除数据集页面主体内的资产库模块。
2. 移除数据集页面主体内重复的数据集列表、创建和设置区域。
3. 改为图谱在上、文档管理在下的单列页面结构。

### Task 4: 验证

**Files:**
- Verify: `apps/web/app/lib/dataset-understanding-graph.test.mjs`
- Verify: `apps/web/app/components/WorkspaceDirectoryPanel.js`
- Verify: `apps/web/app/components/DatasetUnderstandingGraph.js`

1. 运行图谱模型测试。
2. 运行 Web 全量模型测试与生产构建。
3. 本地启动真实页面，验证主体内不再出现资产库和重复数据集列表、图谱全宽居首、节点标签常显且最多 5 个字。

