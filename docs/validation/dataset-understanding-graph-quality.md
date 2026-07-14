# 数据集理解图谱质量验证

## 1. 范围

本页记录“新百项目资料”图谱提质计划的可复现质量门禁。Task 1 冻结旧 `status=empty` fallback 的噪声基线；Task 12 在此基础上补齐单集质量、跨集语义、权限和性能门禁。fixture 结果、单集 canary 结果和跨集 live 结果必须分栏记录，不能互相替代。

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

## 5. Task 12 可执行门禁

### 5.1 单集质量审计

默认 fixture 模式不需要账号或现场数据：

```powershell
node tools/dataset-understanding-quality-audit.mjs `
  --fixture newbai-project-materials
```

该模式验证降噪后的主画布，但会把 `ready snapshot` 和“高质量节点足够时 standard 90–120 节点”明确标记为 `skipped`，不能作为 live ready 快照的替代证据。

对已经脱敏保存的语义 API JSON 执行完整 Task 12 门禁：

```powershell
node tools/dataset-understanding-quality-audit.mjs `
  --input target/dataset-understanding/live-semantic-response.json `
  --profile task12 `
  --require-all
```

`--profile task12` 要求：

- 快照状态为 `ready`；
- 中文开头主标签至少 95%，数字/英文/标点开头合计不超过 5%；
- raw row、SQL/comment、MIME、策略串、扩展名和规范化重复标题均为 0；
- 所有可见边都有 evidence；
- 中心节点为 42px；
- 可用高质量节点至少 90 个时，standard 投影必须为 90–120 个节点；
- 公开 payload 中数据行、SQL、MIME、策略串、内部路径、连接串和 hex64 命中均为 0。

脚本只输出计数和判定，不输出命中的原始字符串。

### 5.2 无凭证跨集语义与权限门禁

```powershell
& .\tools\dataset-cross-graph-security-smoke.ps1 -CompactJson
```

默认模式真实执行现有 Rust fixture 测试，不访问网络、不读取凭证，覆盖：

- exact content identity 折叠和 shared document membership；
- 同名不同来源不折叠；
- 相同中文名只能形成 inferred 线索；
- 明确 FK 可以是 confirmed，inferred 永不升级 confirmed；
- anonymous public、owner private、secret binding、local-thread、cross-tenant 和 mixed visible/hidden list；
- 任一显式数据集不可见时整次请求 masked 404；
- public projection 不返回隐藏标题、隐藏贡献数、raw hash/HMAC/value 或内部路径。

默认无凭证模式不会声称已经检查现场响应和服务日志；这两项在报告中为 `skipped`。若已有受控窗口生成的脱敏响应、日志和禁止标记文件，可离线补齐：

```powershell
& .\tools\dataset-cross-graph-security-smoke.ps1 `
  -ResponsePath target/dataset-understanding/live-response.json `
  -LogPath target/dataset-understanding/live-service.log `
  -ForbiddenMarkerPath target/dataset-understanding/hidden-markers.txt `
  -RequireCapturedArtifacts
```

脚本只报告命中数，不打印响应、日志或禁止标记原文。

### 5.3 浏览器、DOM 和性能门禁

离线运行：

```powershell
node tools/dataset-understanding-browser-smoke.mjs
```

离线结果只验证 100/160 节点预算、180/240 边上限、42px 中心、1440x1000 CSS benchmark 下 720px 画布、单一 `echarts.init` 调用点、实例复用代码路径和纯函数耗时。API p95、首次可交互、真实点击、拖拽缩放 FPS 和现场 DOM 均标记 `skipped`。

feature-off 现场 DOM smoke：

```powershell
node tools/dataset-understanding-browser-smoke.mjs `
  --url <dataset-page-url-with-selected-dataset> `
  --dataset-id d4923d83-6053-4feb-8005-b22ee51e0227 `
  --dataset-title 新百项目资料 `
  --expect-feature-off `
  --require-browser
```

指定 `--dataset-id` 后，smoke 会在页面 catalog 加载完成后，从当前可见 catalog 按 ID 找到 title/key，再精确点击唯一匹配项；随后断言 `/api/v3/datasets/{id}/understanding` 返回的根 dataset ID/标题、图谱标题和 chart accessible label 全部对应目标。`--dataset-title` 是可选的额外断言。未知参数、重复参数、缺少参数值或只给 title 不给 ID 都会直接失败，不会退回默认数据集。

feature-off 模式要求“当前数据集”入口存在，跨数据集入口、选择器和集群工具栏全部不存在；同时检查 ECharts 实例只初始化一次、桌面 benchmark 实际画布高度四舍五入后至少 720px（且不超过 821px）、右侧检查器宽度 295–305px、全屏/Esc 和 390px 手机无横向溢出。

需要认证时只允许显式从环境变量读取，不接受 email、local key 或 cookie CLI 参数。8 服务器可先把受控环境加载到当前进程；不要打印变量值：

```bash
set -a
. /etc/aiv3/v3-agent-terminal-smoke.env
set +a
node tools/dataset-understanding-browser-smoke.mjs \
  --url <dataset-page-url> \
  --dataset-id d4923d83-6053-4feb-8005-b22ee51e0227 \
  --dataset-title 新百项目资料 \
  --auth-from-env \
  --auth-base-url <platform-api-origin> \
  --require-browser
```

现有环境名为 `V3_AGENT_TERMINAL_SMOKE_AUTH_EMAIL` 和 `V3_AGENT_TERMINAL_SMOKE_LOCAL_KEY`。脚本也支持 `DATASET_GRAPH_SMOKE_AUTH_EMAIL` / `DATASET_GRAPH_SMOKE_LOCAL_KEY`；若已有受控 session，可用 `DATASET_GRAPH_SMOKE_SESSION_COOKIE` 或 `V3_AGENT_TERMINAL_SMOKE_SESSION_COOKIE`。`--auth-from-env` 优先使用现成 cookie，否则向 `<platform-api-origin>/v1/auth/key/login` 发送 email/local key，只把返回的 `aidp_v3_session` 注入独立 Playwright context。输出只含认证方式和 HTTP 状态，不含 email、key 或 cookie；新建会话结束后调用 `/v1/auth/logout`，并始终关闭 context。传入现成 cookie 时只关闭 context，不注销外部会话。

本机不把 Playwright 加入产品依赖时，可复用 Codex bundled runtime 和系统 Edge：

```powershell
$runtimeNodeModules = 'C:\Users\soulzyn\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules'
$playwrightCoreRoot = Get-ChildItem "$runtimeNodeModules\.pnpm" -Directory -Filter 'playwright-core@*' |
  Sort-Object Name -Descending |
  Select-Object -First 1 -ExpandProperty FullName
if (-not $playwrightCoreRoot) { throw 'bundled playwright-core was not found' }
$env:NODE_PATH = "$runtimeNodeModules;$playwrightCoreRoot\node_modules"
$env:DATASET_GRAPH_SMOKE_BROWSER_EXECUTABLE = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
node tools/dataset-understanding-browser-smoke.mjs <其余参数>
```

bundled runtime 的顶层 `playwright` 与 `.pnpm` 中的 `playwright-core` 都必须出现在 `NODE_PATH` 搜索路径中；上面的命令按实际安装目录发现版本，不硬编码版本号。脚本先尝试 ESM `import('playwright')`，失败后才通过 `createRequire(import.meta.url)('playwright')` 使用 `NODE_PATH`；浏览器可执行文件也只从环境变量读取，不写入产品依赖或报告。2026-07-14 已用这组环境变量完成模块加载和系统 Edge headless launch/close；这只是运行时可用性检查，不等同于 live 页面门禁通过。

跨图开关开启后的真实 UI smoke：

```bash
node tools/dataset-understanding-browser-smoke.mjs \
  --url <dataset-page-url> \
  --dataset-id d4923d83-6053-4feb-8005-b22ee51e0227 \
  --dataset-title 新百项目资料 \
  --auth-from-env \
  --auth-base-url <platform-api-origin> \
  --expect-cross-enabled \
  --require-browser
```

`--expect-cross-enabled` 与 `--expect-feature-off` 互斥。该模式会点击“跨数据集”，等待 UI `ready`，并同时断言：跨图 API root ID 正确且至少返回 2 个可见数据集；页面出现选择器和至少 2 个 cluster；若 API 有 exact shared 节点，页面共享节点指标及证据清单必须可见，否则至少要有可靠 confirmed/observed 跨边及可见的可靠邻居指标；切换前后 ECharts instance id 不变且 init count 仍为 1。报告只输出计数，不输出节点标签或业务内容。

完整性能 smoke 需要至少 5 个真实样本，建议 20 个：

```powershell
node tools/dataset-understanding-browser-smoke.mjs `
  --url <dataset-page-url-with-selected-dataset> `
  --dataset-id d4923d83-6053-4feb-8005-b22ee51e0227 `
  --dataset-title 新百项目资料 `
  --auth-from-env `
  --auth-base-url <platform-api-origin> `
  --performance-runs 20 `
  --api-url <same-origin-cross-graph-proxy-url> `
  --api-body-file target/dataset-understanding/cross-graph-request.json `
  --measure-fps `
  --require-browser `
  --require-performance
```

只有实际完成测量时才判定：cached API p95 `<500ms` 且 ETag 请求全部 304、响应 `<2MB`；首次可交互 p95 `<=1.5s`；密度选择到 ECharts 更新 p95 `<=100ms`；连续拖拽和缩放交互 `>=45fps` 且可用 Long Task observer 下无 `>200ms` 任务。缺少 URL、请求 body、足够样本或 Playwright 时不会生成虚假的 p95/FPS。

## 6. 2026-07-14 验证回执

### 6.1 单集 canary：新百项目资料

本次已取得的 8 服务器单集 canary 证据：

| 项目 | 结果 | 证据类型 |
| --- | --- | --- |
| 快照状态 | `ready` | live canary |
| 业务对象 / 字段 / 关系 | 9 / 160 / 160 | live canary |
| 中文主标签 | 167 / 167，100% | live canary |
| raw row / SQL / MIME / strategy / extension / duplicate | 0 / 0 / 0 / 0 / 0 / 0 | live canary |
| public path / connection / SQL / MIME / hex64 | 0 / 0 / 0 / 0 / 0 | live canary |
| manifest 大小 | 293,643 bytes | live canary |
| API / cache | 200、ETag、二次请求 304 | live canary |

这组证据证明单集语义快照和公开脱敏门禁通过；它不证明跨数据集 live pair 已建立，也不证明浏览器 p95/FPS 已通过。

### 6.2 本地离线回归

2026-07-14 在仓库根目录执行：

```powershell
node tools/dataset-understanding-quality-audit.mjs --fixture newbai-project-materials
node tools/dataset-understanding-browser-smoke.mjs
& .\tools\dataset-cross-graph-security-smoke.ps1 -CompactJson
pnpm --filter @ai-data-platform-v3/web exec node --test app/lib/dataset-understanding-graph.test.mjs
```

结果：

| 门禁 | 结果 |
| --- | --- |
| 降噪 fixture | 22 节点、47 边、中文开头 100%、六类主画布噪声 0、缺 evidence 0；ready 与 standard 90–120 因 fixture 规模明确 skipped |
| 静态浏览器 fixture | standard 100 节点 / 172 边；expanded 160 / 240；中心 42px；CSS benchmark 720px；`echarts.init` 调用点 1 |
| 离线纯函数耗时 | 30 样本 layout p95 0.37ms；选择/预算投影 p95 0.72ms；只代表本机纯函数，不代表端到端浏览器性能 |
| 跨集契约 | 8 tests passed |
| relation evidence cap | 5 tests passed |
| 跨图 API / 脱敏 | 15 tests passed |
| 数据集权限矩阵 | 7 tests passed |
| Web 图谱聚焦回归 | 27 tests passed |

仓库当前没有 `.github/workflows/ci.yml`，本任务没有为单一 smoke 另建新的 CI 工作流。

### 6.3 仍需 live 完成的项目

以下项目当前必须保持未完成，不得由 fixture 或历史数字代替：

- feature-off 页面上的真实入口隐藏和 DOM 尺寸；
- cached cross-graph API p95 `<500ms`；
- 首次可交互 p95 `<=1.5s`；
- 真实选点/筛选 p95 `<=100ms`；
- 连续拖拽缩放 `>=45fps` 和 long-task gate；
- 两数据集 pair 的共享资料/字段/概念/明确引用 live 结果；
- live mixed visibility 响应与服务日志的隐藏标题、贡献数、hash、原值和内部路径 0 命中。
