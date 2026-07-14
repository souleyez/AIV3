# 数据集理解图谱质量验证

## 1. 范围

本页记录“新百项目资料”图谱提质计划的可复现质量门禁。Task 1 只冻结旧 `status=empty` fallback 的噪声基线，不代表这些标签应继续保留在主画布。

基线使用脱敏 fixture：

```text
apps/web/app/lib/fixtures/newbai-project-materials-noisy-fallback.js
```

fixture 只保留已观察到的噪声形态，例如年份、数字开头多列文本、SQL 表达式、MIME、解析策略标识、英文文件名以及带/不带扩展名的重复标题。文档名、编号、金额、日期、UUID、hash 和路径均为合成值，不复制线上业务原值。

## 2. 基线复现

在仓库根目录运行：

```powershell
pnpm --filter @ai-data-platform-v3/web exec node --test app/lib/dataset-understanding-graph.test.mjs
node tools/dataset-understanding-quality-audit.mjs --fixture newbai-project-materials
```

2026-07-14 Task 1 基线：

| 指标 | 数量 |
| --- | ---: |
| 总节点 | 49 |
| 总边 | 97 |
| 数据集节点 | 1 |
| 文档节点 | 18 |
| 知识词节点 | 16 |
| 章节节点 | 10 |
| 资料类型节点 | 3 |
| 理解策略节点 | 1 |
| 中文开头标签 | 29 |
| 数字开头标签 | 5 |
| 英文开头标签 | 12 |
| 标点开头标签 | 3 |
| 疑似多列数据行 | 2 |
| SQL 或注释片段 | 4 |
| MIME 值 | 3 |
| 策略标识 | 3 |
| 带扩展名文档标题 | 9 |
| 规范化后重复文档标题 | 9 |
| 缺少 evidence 的边 | 0 |

审计脚本的标准输出只允许包含 fixture 名、模式、状态、节点/边计数、静态节点类别和噪声计数。自动测试断言标准输出模型中不包含 fixture 的模拟数据行、SQL 表名、策略原串或英文文件名。

Node 可能提示 `MODULE_TYPELESS_PACKAGE_JSON`；这是当前 Web package 的既有模块类型提示，不影响测试结果或审计计数。

## 3. Task 2 目标门禁

Task 2 完成后使用同一 fixture 复验，主画布必须满足：

- 疑似多列数据行、SQL/comment、MIME、解析策略串为 0；
- 带扩展名与无扩展名的重复文档节点为 0；
- 没有可信中文名称的线索不进入主画布，只进入待解释清单；
- `status=empty` 继续明确标记为 fallback，并展示“资料来源图（语义生成中）”，不得声称系统已形成真实业务理解；
- evidence 缺失边保持为 0。

基线与目标使用同一条审计命令，避免通过更换 fixture 掩盖质量回归。

## 4. Task 2 验证结果

2026-07-14 使用同一 fixture 复验：

| 指标 | Task 1 基线 | Task 2 |
| --- | ---: | ---: |
| 总节点 | 49 | 22 |
| 总边 | 97 | 47 |
| 中文开头标签 | 29 | 22 |
| 数字 / 英文 / 标点开头 | 5 / 12 / 3 | 0 / 0 / 0 |
| 疑似多列数据行 | 2 | 0 |
| SQL 或注释片段 | 4 | 0 |
| MIME 值 | 3 | 0 |
| 策略标识 | 3 | 0 |
| 带扩展名文档标题 | 9 | 0 |
| 规范化后重复文档标题 | 9 | 0 |
| 缺少 evidence 的边 | 0 | 0 |

实现后的 fallback 明确标记为“资料来源图（语义生成中）”。18 个低质量或技术项进入聚合待解释清单，9 个带/不带扩展名的重复资料标题被真实文档优先合并；原始数据行、SQL、路径、编号和 hash 不在界面回显。高质量标签先通过门禁再应用节点限额，因此不会因前排噪声占满限额而丢失后续可信中文线索。
