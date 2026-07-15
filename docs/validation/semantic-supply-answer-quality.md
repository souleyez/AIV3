# 知识图谱辅助供料与问答质量验证账本

## 1. 范围和硬边界

本账本对应：

```text
docs/archive/plans/2026-07-15-datamax-knowledge-graph-assisted-supply-answer-quality-plan.md
```

目标是验证现有语义图谱能否通过选择、排序和补充真实证据提高问答质量。核心原则是：**只编排授权、可追溯的供料，不编排对话、意图、答案、路由或动作**。图谱不得决定答案结构、模型结论、措辞、报表或静态页；模型继续自行理解问题、组织答案并决定表达方式。

允许变化的唯一生产行为是内部供料候选、顺序和最多两条真实补证。用户问题、system prompt、answer policy、provider、公开接口、SSE、citation 类型和工作流动作均保持不变。

## 2. 冻结基线

### 2.1 代码基线

| 项目 | 值 |
| --- | --- |
| 本地 worktree | `C:\Users\soulzyn\Desktop\codex\ai-data-platform-v3-task13` |
| 分支 | `codex/dataset-understanding-mvp` |
| 执行前 HEAD | `e14e924fa40ddb3f12b2366ea07a73bc18e4f467` |
| 基线提交 | `test: close dataset graph rollout gates` |
| 执行启动时计划 SHA-256（historical） | `F4098D9B93F328A8E629D0873C5B39F15BA77C00EE128A507EB90607538FB6D7` |

执行开始前仅有活动计划和活动指针文档改动，没有产品代码改动。

### 2.2 现有图谱对问答的直接影响

执行前 AssistantRun 不读取 `dataset_semantic_snapshots`、`dataset_semantic_link_snapshots` 或 `semantic_dictionary_entries`。因此现有图谱发布对问答供料、排序、模型输入和答案的直接提升为 0。

现场图谱能力：

- “新百项目资料” ready 快照：9 个对象、160 个字段、160 条关系；
- 7 个数据集、21/21 个 pair ready；
- 首对现场只证明 1 个共享资料节点，真实跨数据集关系边为 0。

### 2.3 Platform API PostgreSQL 18 基线

2026-07-15 执行前的全新一次性数据库结果：

| 项目 | 数量 |
| --- | ---: |
| passed | 2819 |
| failed | 32 |
| ignored | 1 |

32 项已逐项在独立全新数据库中复现，排除测试顺序和 smoke 污染。本计划直接相关的红项为：

- 3 项 retrieval evidence 的 `document_id` 归属断言；
- 1 项 CJK lexical 排名；
- 1 项 PostgreSQL 18 deep-old 查询计划；
- NewBai selected-case evaluator 把未执行 009/010 误判为 `missing_result`；
- 两个 smoke 脚本使用过期 Cargo filter，存在零测试假绿。

其余既有红项作为冻结差异；本计划不得新增红项。

证据：

- `target/platform-api-db-test-20260715.log`
  - SHA-256 `4935BA4322089D31D3C60856E8268DAD25A56F0C7B6F5895C3E0EA1527A546E4`
- `target/failure-isolation-matrix-20260715.tsv`
  - SHA-256 `F0D2FF61FBACE2E74686A628050FAB0122FDE6A3F1BE0A51BDAACED0441666FE`

### 2.4 问答质量基线

文档问答专项：

- 11/11 通过；
- 覆盖一字 PDF、邓工、简历统计与排序、考勤、养老、扫描件等；
- 证据：`target/document-quality-smoke/document-quality-smoke-20260714T232149Z-14884.json`。

检索 fixture：

- 35 项；
- 权限泄漏 0；
- 当前报告未记录 live Recall/MRR，只能作为 fixture 和契约基线；
- 证据：`target/retrieval-quality-smoke/retrieval-quality-smoke-20260714T232523Z.json`。

NewBai 真实模型基线：

- 实际执行 10 个 case，网络和 provider 调用均成功；
- 只有 011 ordinary guard 没有 evaluator 失败；
- 001–008 均缺少所需业务证据；
- 005 还出现错误断言；
- 012 误触发静态页/报告副作用；
- 009/010 未执行，但旧 evaluator 误报 `missing_result`；
- 证据：`target/newbai-live-20260715.json` 和 `target/newbai-live-20260715.md`。

## 3. A/B/C 预注册

| Arm | 行为 |
| --- | --- |
| A | 当前供料 |
| B | 候选与 TopK 不变，只做语义溯源重排 |
| C | B 加最多两条有当前可见 document/chunk/source locator 的真实补证 |

Shadow 只计算 B/C，不改变 A 的模型可见供料。

第一版只允许 `confirmed` 和 `observed`；`inferred`、`unresolved`、stale、版本不兼容或无可见 provenance 的节点使用数必须为 0。

## 4. 预注册门禁

### 4.1 供料

- target provenance 可解析率 `>=90%`；
- NewBai 001–008 expected-source Hit@4 = 8/8；
- full Recall@20 不低于 A；
- full MRR@20 相对 A 下降不超过 0.01；
- target MRR@20 至少提升 0.10；
- target Precision@5 至少提升 15 个百分点；
- 无关问题 Top5 来源重叠率相对 A 至少下降 30%；
- 补证 `document_id`、chunk ID 和 locator 完整率 100%；
- 权限泄漏 0。

### 4.2 答案和模型自主

- target answer pass rate 至少提升 20 个百分点；
- 错误断言、内部字段泄漏和图节点伪 citation 均为 0；
- 文档专项保持 11/11；
- database aggregate、TopN、逐行、普通问答不得回归；
- 用户问题、system prompt、answer policy、provider、intent、scope、action 列表在 A/B/C 一致；
- provider 调用和 retry 数一致；
- unexpected workflow/artifact/report/template/static-page delta 为 0。

### 4.3 性能

- 不增加 provider 调用；
- 检索阶段 p95 增量同时 `<=100ms` 和 `<=20%`；
- provider input 增量 `<=15%`；
- 同次 off/A 路径的模型可见供料字节自一致；这不是独立冻结的 feature-off 基线。

## 5. 执行记录

| Task | 状态 | 结果 |
| --- | --- | --- |
| Task 0 测量可信度与基线 | completed | 修复 selected-case 误报和零测试假绿；PostgreSQL 18 隔离基线与专项 smoke 已留证 |
| Task 1 A/B/C corpus/evaluator | completed | 24-case 独立离线检索诊断、独立 runtime input、oracle 隔离、双哈希和 fail-closed evaluator 已完成；旧 58 例没有逐例并入 |
| Task 2 纯语义供料计划 | completed | 仅使用 `confirmed`/`observed`、有可见 provenance 的有界纯计划；不含答案、意图、路由或动作 |
| Task 3 权限溯源 | completed | 只处理本轮明确选择且逐个鉴权通过的数据集；补证前再次检查 dataset/document/ACL |
| Task 4 shadow contract | completed | 只证明实现内同次 off/A 路径字节自一致与 shadow 安全计数；独立冻结 feature-off 基线未测量 |
| Task 5 rerank | completed | B 只重排同一候选池；无语义命中的候选保持原有分数和顺序语义 |
| Task 6 supplement | completed | C 最多补两条真实 `document/chunk/source_locator` 证据，不从图节点生成事实或 citation |
| Task 7 模型自主与公开契约 | completed | 内部语义字段不进入 provider/public citation；模型调用、动作目录和普通问答契约保持不变 |
| Task 8 offline A/B/C | completed_fail_closed | 24 例只作离线检索诊断；旧 precommit 数值已被安全加固后的 fixture/evaluator 变更取代；独立 feature-off 基线缺失使 `retrieval_candidate=null` |
| Task 9 full regression | completed | Web 467/467、storage 46、retrieval-worker 50/0/1 ignored、Platform API 2884/1 frozen/2 ignored；semantic-supply 专项 33/0/1 ignored |
| Task 10 feature-off/shadow rollout | completed_operational_install_only | 8 服务器仅执行 Phase A：代码提交 `9b7ec2bcee7796723c58f781a8bbf113d3285b31` 已安装；模式 `off`、三 allowlist `0/0/0`；只重启 API 且 active/health/ready，非 API 服务快照一致。shadow / final-off 为 `not_executed` / `N/A`，`provider_calls_initiated_by_runbook=0`；不构成 promotion、shadow-safe 或问答提升证据 |
| Task 11 controlled live decision | skipped_fail_closed | Task 8 没有 promotion candidate，因此不执行 live A/B/C，不调用 provider；live harness 只作为未来工具保留 |

### 5.1 Task 0–7 实现与安全证据

- Task 0 的 NewBai selected-case evaluator 已不再把未执行用例误判为 `missing_result`；两个 smoke 的过期 Cargo filter 已改为当前测试名并要求非零测试数。
- PostgreSQL 18 一次性数据库基线、失败隔离矩阵和既有红项已冻结；本轮相关 provenance、CJK lexical 和 deep-old 合同已聚焦复验。
- Task 1 将 `fixtures/semantic-supply-ab/runtime-inputs.jsonl` 与 evaluator oracle 物理隔离。runtime input 只有问题、明确选择范围、权限、候选池和语义快照；不得包含 expected ranking、答案、意图、路由、动作或对话模板。
- 24 例中的 4 个多数据集用例只验证本轮明确选择、逐个授权的数据集能分别定位两个来源；夹具没有 relation/link snapshot，不证明跨数据集关系理解。既有 35+12+11=58 例仍是独立回归清单，没有与 24 例 A/B/C 回执逐例绑定。
- Task 2–6 的生产逻辑只生成授权可追溯的供料计划。跨数据集不会自动扩展到“相似邻居”，只能使用本轮明确选择且可见的数据集；图节点、相似边和推断边都不是事实或引用。
- Task 4 的 `off` 与 `shadow` 模型可见输入保持字节等价；Task 5/6 也不改用户问题、system prompt、answer policy、provider、动作目录或回答结构。NewBai live-capture 也已移除 `default_prompt`、`output_format`、`render_mode`、`requested_skills` 等编排字段，只提交原始用户问题和显式授权资料范围。
- Task 7 的 targeted Rust tests、`cargo test -p platform-api semantic_supply --lib` 和 9-check assistant-chat contract smoke 通过。公开 citation 仍只允许真实资料、切片、数据库结果或确定性事实供料。
- 代码审阅未发现 Task 2–7 权限边界或模型自主边界的 P0/P1 问题；更宽松的三字中文局部命中已收紧，并增加“数据表/风险表”误匹配负例。

Task 0 关键本地证据：

| 证据 | 结果 | Receipt / SHA-256 |
| --- | --- | --- |
| NewBai evaluator self-test | 12/12 | `target/newbai-customer-answer-smoke/newbai-customer-answer-smoke-20260715020130.json` / `68B02F52A4B7B4535834CC31A2C4E5299CAED24298500152231747437BC2AB84` |
| external direct reply smoke | 15 checks | `target/external-direct-reply-smoke/external-direct-reply-smoke-20260715T020230Z.json` / `34D7C5FBEACF932F0BB668D9DB3C312535B01353D6B9B1E219FF5755605E2A51` |
| document understanding smoke | 33 checks | `target/document-understanding-smoke/document-understanding-smoke-20260715T020337Z.json` / `84E38884E1B17B0C135C8909F746553A69803E8728A8C2C79199511376C5203B` |
| assistant chat contract smoke | 9 checks | `target/assistant-chat-contract-smoke/assistant-chat-contract-smoke-20260715T023741Z.json` / `A6DA3926703293B4AA0486608BBDE8143B7B5424483423301579079993C930C3` |

### 5.2 Task 8 离线三臂检索诊断结果

旧 precommit receipt 已与当前 runtime fixture 和实现失配：其后已收紧显式 selected-dataset 边界、实际 chunk/locator 别名绑定、移除夹具中的关系暗示和答案 oracle，并将 offline route 明确为 `not_run`。因此旧路径、哈希和数值全部仅是历史调试信息，不得用于当前收益、发布或晋级结论。

当前固定 runtime input 只包含问题、显式选择范围、权限、候选池和语义快照：

- runtime input raw SHA-256：`9f566aef003c43c40c02ddc5e1353914899a8b6ea3ec385cfc6a1386ee77483d`；
- runtime input canonical SHA-256：`87aba9c888066838fd4a6ba51cf4cdf0f65e3cd954657e7eadf4e5b076fdc908`；
- 离线 exporter 仅记录 `route.status=not_run`，因此只测未执行一致性，不测模型 route parity；
- clean-HEAD exporter/evaluator 回执必须在最终提交后作为不入库 sidecar 生成，并记录实际 `git rev-parse HEAD` 与 clean worktree。

即使最终 clean-HEAD 检索指标显示方向性改善，当前也必须保持 `retrieval_candidate=null`、`decision_eligible=false`，因为没有独立冻结的 feature-off 基线。不得通过下调 P@5、grounding、holdout、overlap 或 latency 阈值制造候选。

代码提交 `9b7ec2bcee7796723c58f781a8bbf113d3285b31` 的 clean-worktree exporter/evaluator sidecar 已完成。它只是一组离线检索诊断，不是 feature-off 基线或答案质量证据：

| 项目 | A control | B graph rerank | C rerank + evidence |
| --- | ---: | ---: | ---: |
| Recall@20 | 0.416667 | 0.791667 | 0.791667 |
| MRR@20 | 0.416667 | 0.791667 | 0.791667 |
| P@5 | 0.083333 | 0.183333 | 0.183333 |
| grounding | 0.85 | 0.85 | 0.85 |
| p95 ms | 35.5670 | 35.1052 | 50.1643 |
| permission leaks | 0 | 0 | 0 |

- experiment id：`5229b4911f4b2435f846f8bfa601f63ad9d4a1e6ee99b5ce755c82c668d3ca1e`；fixture SHA-256：`6592667a5826795a42f5c0d262a327cfa0b86b66604a1b678204adf26b5cda85`；
- runtime input raw / canonical SHA-256：`9f566aef003c43c40c02ddc5e1353914899a8b6ea3ec385cfc6a1386ee77483d` / `87aba9c888066838fd4a6ba51cf4cdf0f65e3cd954657e7eadf4e5b076fdc908`；
- evaluator receipt SHA-256：`51b1549ce522a0eab7b15de97c3b3644b2ed253015b934d4c38f9aa9867c8e18`；
- 最终状态：`acceptance_status=retrieval_gate_failed`、`retrieval_candidate=null`、`decision_eligible=false`、`answer_evaluation.status=not_run`；三臂 grounding 都只有 17/20，且没有独立冻结的 feature-off 基线。

离线与全答案命令必须区分：

```bash
# 只验证离线检索、grounding、权限、契约和延迟；允许 answer_evaluation=not_run
bash scripts/run-semantic-supply-ab-smoke.sh --require-retrieval-metrics

# answer receipt v2 诊断；因缺少逐 claim 来源绑定和旧 58 例 manifest，固定非晋级
bash scripts/run-semantic-supply-ab-smoke.sh --require-metrics
```

最终 `--require-retrieval-metrics` 应因独立 feature-off 基线缺失按预期非零退出。`--require-metrics` 的 v2 路径即使收到格式正确的回执，也会因为没有可重算的逐 claim → document/chunk/source locator/retrieval hit 绑定、且没有旧 58 例逐例 manifest 而固定非零退出；当前状态仍是 `answer_evaluation.status=not_run`。本计划不调用 provider，也不做任何答案质量、自然度或 citation 准确率提升结论。

### 5.3 Task 9 全量本地回归

| 命令 / Gate | 结果 |
| --- | --- |
| `cargo fmt --all`、`cargo fmt --all -- --check` | passed |
| `git diff --check` | passed |
| `npm --prefix apps/web test` | 467 passed、0 failed，13 suites |
| `bash scripts/run-dataset-semantic-understanding-smoke.sh --no-credentials` | 8 checks passed；无凭证、无网络变更 |
| `bash scripts/run-retrieval-quality-smoke.sh --baseline` | 35 cases passed、permission leak 0；该 baseline 不记录 live Recall/MRR |
| `cargo test -p storage` | 46 passed、0 failed |
| `cargo test -p retrieval-worker --all-targets` | 50 passed、0 failed、1 ignored |
| `cargo test -p platform-api --lib` | 2884 passed、1 frozen known failure、2 ignored；没有本计划新增失败 |
| `cargo test -p platform-api semantic_supply --lib` | 33 passed、0 failed、1 ignored |
| document quality smoke | 11/11 passed |
| NewBai customer-answer self-test | 12/12 passed；provider 未调用 |

Platform API 唯一失败仍是冻结基线中的 `assistant_run_provider_input_truncates_long_history_messages`，不得归为本计划新回归，也不得隐去。

Task 9 receipts：

| Receipt | SHA-256 |
| --- | --- |
| `target/retrieval-quality-smoke/retrieval-quality-smoke-20260715T053918Z.json` | `59025307881F2B862848A3B54B8EF4EDE4ED995E225049EE85529AD3F99AC1FC` |
| `target/document-quality-smoke/document-quality-smoke-20260715T054012Z-37500.json` | `AFBC0F126765686F404397DF294C9002B0EE68FF946145356A3AA42C87350A57` |
| `target/newbai-customer-answer-smoke/newbai-customer-answer-smoke-20260715054012.json` | `93090C075B2A6F38444B8E5E58ABBB132456BAD76169C37DA20A478281285412` |

### 5.4 Task 10 Phase A feature-off operational install

8 服务器只完成 Phase A operational install。运行二进制由代码提交 `9b7ec2bcee7796723c58f781a8bbf113d3285b31` 构建；没有执行 shadow、Task 11 或问答请求，rollout runbook 未发起 provider 调用，也没有修改 Web/worker、migration、图谱快照或业务数据。

第一次切换在 `/srv/aiv3/backups/assistant-semantic-supply-20260715T055657Z` 留下备份。聚焦测试、构建、显式 feature-off 和 API 健康均已通过，但旧版 `journalctl` 拒绝带 `T/+08:00` 的时间参数，发布脚本因此按 fail-closed 规则恢复发布前配置与二进制并确认 API 健康。随后只将日志窗口改为该服务器兼容的本地时间格式，完整重跑并成功。

| 回执字段 | 结果 |
| --- | --- |
| 成功窗口 | `2026-07-15T06:05:44Z` → `2026-07-15T06:05:53Z` |
| approved code / GitHub / Phase A server HEAD | `9b7ec2bcee7796723c58f781a8bbf113d3285b31`；worktree clean |
| PostgreSQL 服务端 | `18.4` |
| API binary SHA-256 before | `158cfb2e47b3b171a4bc5ddd1616f7bbebf358dcb8243251dc9d74ad66aa1ef4` |
| API binary SHA-256 built / running | `aceede30ad2a0c6d20eb4c138f4cd8266d167e8e4e7598ae90f3f4f884cfc227` / 相同 |
| 服务器聚焦门禁 | shadow 2、model autonomy 2、public citation 4、model supply brief 1、ordinary chat unrestricted 1；A/B/C self-test passed |
| feature-off | API active；health/ready passed；mode `off`；allowlist `0/0/0`；值不入回执 |
| shadow / final-off | `not_executed` / `N/A` |
| 服务边界 | 非 API 服务快照 byte-equal；only API restarted |
| provider | `provider_calls_initiated_by_runbook=0` |
| 日志安全 | 247 行；semantic receipt 0；unsafe receipt 0；fatal/readiness error 0 |
| 备份与 cleanup | `/srv/aiv3/backups/assistant-semantic-supply-20260715T060548Z`；`cleanup_manifest_auto_delete=false` |
| Phase A receipt SHA-256 | `70cca35e7cd05e1a5c2fb13d1fbf38e06366f21b2b11fbbd496b747eca66eb94` |
| 结论 | `feature_off_operational_install_only` |

该回执只证明 feature-off operational install、配置和 API 服务边界；不证明独立 feature-off 基线、shadow 安全、检索收益、答案质量、自然度或 citation 准确率提升。当前决策仍为 `keep_graph_visual_only`。

## 6. 最终决策

当前决策：`keep_graph_visual_only`。

理由不是“图谱无价值”，而是不能从已失配当前 fixture/实现的旧 precommit 数值宣称当前收益；而且独立冻结 feature-off 基线、跨数据集关系证据、逐 claim 引用绑定和旧 58 例逐例关联都没有完成。因此：

- 不晋级 rerank；
- 不晋级 supplement；
- 不执行 Task 11 live A/B/C，不调用 provider；
- 不声称答案质量、自然度或 citation 准确率提升；
- feature mode 保持 `off`，图谱继续用于数据集理解可视化。

Task 10 Phase A 的 operational install 不改变该决策，也不构成 `feature_off_shadow_safe` 或问答质量证据。

`scripts/smoke/semantic-supply-main-live.mjs` 只保留未来采集契约的 fail-closed 脚手架。其本地 `--self-test --pretty` 已通过，记录 `network_requests=0`、`filesystem_writes=0`、`decision_eligible=false`；当前版本还会对所有 preflight/execute 固定返回 `live_execution_disabled_without_offline_promotion_candidate`，只读并校验仓库固定夹具后，在创建回执文件、认证和联网前退出。以后即使离线候选通过，也必须另行改代码解除硬锁并重新审查，不能把当前 self-test、preflight 或任何单臂 receipt 解释为晋级证据。
