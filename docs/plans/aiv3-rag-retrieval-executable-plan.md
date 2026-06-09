# AIV3 多源数据问答检索供料可执行计划

更新时间：2026-06-09 CST

当前性质：可执行计划文档；本文件同时记录当前本地工作树进度、后续开发路线、验收标准和回滚边界。

状态口径：

- `已完成` 只表示当前本地工作树已有相应改动或本地 receipt 证据；不表示已经合并 GitHub、部署 8 服务器或启用线上开关。
- `待完成` 表示仍需开发、补证据、复测或等待上线批准。
- 上线、GitHub 同步、8 服务器部署必须另行获得明确批准；本计划不授权部署。

关联输入：

- 外部建议文档：`/Users/manslive01/Documents/Codex/2026-06-08/pdf-postgresql/outputs/aiv3-rag-retrieval-improvement-plan.md`
- 当前仓库：`/Users/manslive01/Code/AIV3`

## 1. 执行结论

当前 V3 不需要推翻重做。下一阶段应按“小步可回滚”的顺序推进检索供料质量：

1. 先建立 retrieval quality baseline。
2. 先做 PostgreSQL DB-side lexical retrieval MVP，替换 `latest-512` 扫描。
3. 再做 pgvector-first 的真实 embedding/vector 检索。
4. 再做 lexical + vector + fact/database candidates 的 hybrid RRF。
5. 最后把数据库数值/TopN/趋势问题升级为受控 query plan。

当前推荐默认路线：

```text
Phase 0: baseline only
Phase 1: PostgreSQL lexical MVP
Phase 2: pgvector-first vector backend
Phase 3: hybrid RRF and optional rerank
Phase 4: structured database query plan
Phase 5: quality gate and release receipt
```

暂不建议直接把 Qdrant 设为默认主链路。Qdrant 已在 compose 中存在，可以保留 adapter 接口；但当前用户目标更偏“清洗后统一入 PostgreSQL”，所以第一轮落地应优先降低运维复杂度。

Phase 0/1 最小版已经提交 GitHub 并部署到 8 服务器，当前线上仍保持 `legacy_scan` 默认行为。已完成 fixture、baseline smoke、PostgreSQL lexical migration、retrieval worker lexical 写入、storage DB-side lexical search、platform feature flag、8 服务器 schema/code 部署、生产 EXPLAIN、NewBai 检索级 live subset metrics，以及 P1-8 ranking 修正部署复测。P1-8 复测后 NewBai live subset 的 Recall@20=`1.0`、MRR@20=`1.0`、permission leak count=`0`，8 条 expected source 均为 rank `1`。P1-11 已补 NewBai provider-input deterministic smoke、AssistantRun DB route smoke 和脚本 receipt，并用一次性 Postgres fixture 跑通；P1-11 ranking/handoff refinement 已部署 8 服务器并重跑 NewBai live subset，8 条 expected source 仍均为 rank `1`。仍未记录真实 provider/customer-answer live 输出质量。

当前仍不能声称完成的事项：

- 未记录完整 assistant-run/customer-answer 的 live 质量指标；当前已记录的是 8 服务器 `retrieval-search-cli` 检索级 live subset、NewBai provider-input deterministic smoke、以及一次性 Postgres fixture 下的 AssistantRun DB route smoke。
- 未启用 `RETRIEVAL_SEARCH_BACKEND=postgres_lexical`，8 服务器线上仍是默认 `legacy_scan`。
- 未启用 pgvector、Qdrant、hybrid RRF、reranker 或 structured database query plan。
- P1-8 只证明检索级 ranking 修复；P1-11 本地 fixture 已证明 AssistantRun 能把预期证据供给到模型侧，但还不等于真实 provider/live customer-answer 质量闭环。

## 2. 当前证据基线

只读检查已经确认以下事实：

| 事实 | 当前位置 | 执行含义 |
| --- | --- | --- |
| 当前 `retrieval.search` 存在固定候选扫描上限 | `crates/platform-api/src/lib.rs` 中 `DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT = 512` | 大数据集会漏召回，必须替换 |
| `search_dataset_retrieval_with_state` 仍从 storage list evidence 后在内存排序 | `crates/platform-api/src/lib.rs` | 不是索引级召回 |
| `retrieval-worker` 从 `document_chunks` 生成 `retrieval_evidences` | `crates/retrieval-worker/src/main.rs` | 最小改造可以先索引 evidence，但要去重 |
| schema 已有 `document_chunks`、`retrieval_evidences`、`dataset_fact_snapshots` | `crates/storage/migrations/0001_initial_schema.sql`、`0012_document_fact_index.sql` | 可复用当前数据模型 |
| compose 已有 Qdrant | `infra/compose/docker-compose.local.yml` | 可作为后续 vector backend 选项 |
| `database_aggregate` 和 `dataset_fact_snapshot` 已进入模型供料 | `crates/platform-api/src/lib.rs` | 数据库问答可从启发式升级为 query plan |
| row identity collapse 已有业务决策文档 | `docs/operations/data-source-row-identity-decision.md` | 行级完整性必须作为门禁 |

## 3. 总体边界

### 3.1 目标

- 让大数据集检索不再依赖最近 512 条 evidence。
- 让文档问答、数据库问答、临时附件、多文档比较都能获得可解释、可引用、可检验的 evidence pack。
- 让数值、TopN、趋势、汇总问题优先使用结构化 `database_aggregate` 或 `structured_query_result`，而不是从自然语言 chunk 里推断数字。
- 让每次检索改动都有可量化 receipt：Recall@20、MRR@20、citation accuracy、permission leak count、p95 latency。

### 3.2 非目标

- Phase 1 不接真实 embedding provider。
- Phase 1 不启用 Qdrant。
- Phase 1 不让模型自由生成 SQL。
- 不把 Qdrant 或 pgvector 当 source of truth。
- 不在 row identity collapse 未修复前声称行级明细完整。
- 不只靠 prompt 修复检索供料问题。

## 4. 关键设计决策

### D1：Phase 1 索引对象怎么选

推荐执行策略：

- MVP：先在 `retrieval_evidences` 上补 lexical search 字段，改动最小。
- 必须同步实现：按 `document_chunk_id + indexed_content_hash` 去重，只返回同一 chunk 的最新有效 evidence。
- 后续增强：如果发现 evidence ledger 重建频繁、重复候选过多，再拆出 canonical `retrieval_index_entries` 表。

原因：

- 现有 `retrieval.search` 已经面向 `retrieval_evidences` 返回结果，MVP 最小。
- 但 `retrieval_evidences` 更像执行 ledger，不是天然唯一索引表；不去重会让重复 evidence 挤占 topK。

### D2：中文 lexical 怎么做

不要依赖 PostgreSQL 默认中文分词。Phase 1 的中文召回以 worker 生成的 `search_terms` 为主，`search_tsv` 为英文、数字、空格分词和辅助查询。

建议 tokenizer 输出：

```text
normalized terms
CJK bigram/trigram
domain terms: 合同、取高、销售缺口、经营风险、客流、坪效
numbers and ids
field/table/entity names
```

### D3：权限过滤放在哪里

权限过滤必须在候选召回阶段生效，不能先取 topK 再过滤。

DB-side lexical SQL 必须在 ranking 前过滤：

- tenant
- dataset
- selected documents
- owner user
- dataset membership
- external ACL / supplied document scope

vector backend 也必须用 payload filter 或回表过滤确保未授权候选不参与 topK。

### D4：pgvector 前置条件

Phase 2 才允许引入 pgvector。进入 Phase 2 前必须先做生产预检：

```sql
select name, installed_version, default_version
from pg_available_extensions
where name = 'vector';
```

如果生产 PostgreSQL 没有 pgvector extension 包，不允许直接合并 `create extension vector` migration；应先补运维安装方案，或临时走 Qdrant adapter。

## 5. Feature Flags

新增或固化以下开关：

```text
RETRIEVAL_SEARCH_BACKEND=legacy_scan | postgres_lexical | hybrid
RETRIEVAL_VECTOR_BACKEND=disabled | pgvector | qdrant
RETRIEVAL_EMBEDDING_PROVIDER=mock | gateway | openai | local
RETRIEVAL_EMBEDDING_MODEL=<model>
RETRIEVAL_HYBRID_ENABLED=false | true
RETRIEVAL_RERANK_BACKEND=disabled | mock | model
RETRIEVAL_LEXICAL_INDEX_BACKFILL_ENABLED=false | true
```

默认安全值：

```text
RETRIEVAL_SEARCH_BACKEND=legacy_scan
RETRIEVAL_VECTOR_BACKEND=disabled
RETRIEVAL_HYBRID_ENABLED=false
RETRIEVAL_RERANK_BACKEND=disabled
```

Phase 1 验收通过后，8 服务器可只切：

```text
RETRIEVAL_SEARCH_BACKEND=postgres_lexical
```

## 6. Phase 0：Baseline

时间：0.5 到 1 天

目标：在改代码前固定当前检索质量，后续每个改动都能对比。

### 6.1 任务

- 新增目录：

```text
fixtures/retrieval-quality/
  cases.jsonl
  README.md
scripts/run-retrieval-quality-smoke.sh
docs/validation/retrieval-quality-smoke.md
```

- 从现有文档质量、外部 scoped document、数据库源、新百报表 case 中抽 30 到 50 个 baseline case。
- 每个 case 记录期望 source、期望 answer pattern、禁止 source、禁止 answer。
- 输出 JSON/Markdown receipt。

### 6.2 必含 case

- deep old chunk：目标 evidence 很旧，但必须召回。
- selected document scope：只允许 doc A，不得召回 doc B。
- owner scope：私有文档不得跨用户召回。
- 中文短语：`订单延期风险`、`取高机会`、`经营风险`。
- 数据库 TopN：必须出现 `database_aggregate`。
- row identity collapse：不得声称行级完整。

### 6.3 验收

Phase 0 分成两层验收。第一层是 fixture 和本地 contract smoke，第二层才是带真实结果的质量指标。

Phase 0A，fixture baseline：

```text
baseline_report_created=true
case_count >= 30
permission_leak_count = 0
required_categories_covered=true
targeted_contract_tests_passed=true
```

Phase 0B，live/result evaluator：

```text
current_recall_mrr_recorded=true
citation_accuracy_recorded=true
p95_latency_recorded=true
all_fixture_cases_have_result=true
permission_leak_count = 0
```

Phase 0A 可以先作为最小门禁；Phase 0B 必须在进入 hybrid、rerank 或线上默认切换前完成。

### 6.4 命令

当前最小命令：

```bash
bash scripts/run-retrieval-quality-smoke.sh --baseline
```

fixture-only 快速检查：

```bash
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline
```

live/result evaluator 命令：

```bash
bash scripts/run-retrieval-quality-smoke.sh \
  --baseline \
  --results-jsonl target/retrieval-quality-smoke/live-results.jsonl \
  --require-metrics
```

`--results-jsonl` 和 `--require-metrics` 已接入 smoke 脚本。默认不传结果文件时，smoke 仍只验证 fixture 和 contract tests，不伪造 Recall/MRR/latency 指标。

### 6.5 Result JSONL 格式

每一行对应一个 fixture case 的真实检索/回答结果：

```json
{
  "case_id": "deep_old_chunk_contract_clause",
  "latency_ms": 842,
  "answer": "回答正文",
  "hits": [
    {
      "rank": 1,
      "source_id": "document:contract-a:chunk-42",
      "document_id": "contract-a",
      "supply_type": "retrieval_evidence",
      "score": 0.93
    }
  ],
  "citations": [
    {
      "source_id": "document:contract-a:chunk-42",
      "quote": "用于定位引用的短摘录"
    }
  ]
}
```

指标计算规则：

- Recall@20：`expected_sources` 至少一个出现在 top 20 hits。
- MRR@20：第一个 expected source 的倒数排名。
- citation accuracy：引用来源必须命中 `expected_sources` 或同一允许文档范围。
- answer pattern match：回答必须匹配 `expected_answer_patterns`，且不得匹配 `forbidden_answer_patterns`。
- permission leak：hits、citations 或 answer 命中 `forbidden_sources/forbidden_answer_patterns` 记为泄漏。
- p95 latency：按 `latency_ms` 计算第 95 百分位。

## 7. Phase 1：PostgreSQL Lexical MVP

时间：1 到 2 天

目标：用 DB-side lexical candidate recall 替换 latest-512 scan，不接 vector。

### 7.1 Schema

新增 migration，当前下一号可用为：

```text
crates/storage/migrations/0014_retrieval_lexical_index.sql
```

建议字段：

```sql
alter table retrieval_evidences
    add column if not exists search_text text,
    add column if not exists search_terms jsonb not null default '[]'::jsonb,
    add column if not exists search_tsv tsvector,
    add column if not exists search_language text,
    add column if not exists indexed_content_hash text,
    add column if not exists indexed_at timestamptz;

create index if not exists retrieval_evidences_scope_chunk_idx
    on retrieval_evidences (tenant_id, dataset_id, document_id, document_chunk_id, created_at desc);

create index if not exists retrieval_evidences_search_terms_gin_idx
    on retrieval_evidences using gin (search_terms);

create index if not exists retrieval_evidences_search_tsv_gin_idx
    on retrieval_evidences using gin (search_tsv);
```

不在 Phase 1 加 `embedding`、`vector_point_id` 或 `create extension vector`。

### 7.2 Retrieval Worker

修改位置：

- `crates/retrieval-worker/src/main.rs`
- 必要时新增 `crates/retrieval-worker/src/lexical.rs`

任务：

- 为每个 chunk 生成 `search_text`。
- 生成 `search_terms`，覆盖中文 ngram、数字、字段名、实体名、标题 hint。
- 生成 `search_tsv`。
- 写 `indexed_content_hash`，同内容重跑不重复更新。
- 保留原 `local-lexical-v1` manifest，但标清它是 lexical，不是真 dense embedding。

### 7.3 Storage API

修改位置：

- `crates/storage/src/lib.rs`

新增：

```rust
pub struct LexicalRetrievalQuery {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub query: String,
    pub document_ids: Vec<DocumentId>,
    pub owner_user_id: Option<UserId>,
    pub limit: usize,
    pub candidate_limit: usize,
}
```

新增方法：

```rust
search_lexical_retrieval_evidences(query: LexicalRetrievalQuery)
```

硬要求：

- SQL 内先做权限过滤，再排序。
- 同一 `document_chunk_id` 只保留最新有效 evidence。
- 返回 shape 能映射到旧 `RetrievalEvidenceView`。
- diagnostics 中记录 candidate counts，但不暴露 raw internal blob。

### 7.4 Platform API

修改位置：

- `crates/platform-api/src/lib.rs`

替换路径：

- `search_dataset_retrieval_with_state`
- `retrieval_search_scan_limit`
- 相关 dataset output retrieval supply 路径

新逻辑：

```text
if RETRIEVAL_SEARCH_BACKEND=postgres_lexical:
    call storage.search_lexical_retrieval_evidences
else:
    existing legacy_scan
```

API 兼容：

- 不删除 `hits` 字段。
- 只新增 debug-safe diagnostics：

```json
{
  "retrieval_mode": "postgres_lexical",
  "candidate_counts": {
    "lexical": 80,
    "deduped": 50,
    "final": 8
  },
  "fallback_reason": null
}
```

### 7.5 Tests

新增或改造：

```bash
cargo test -p storage retrieval_lexical --lib
cargo test -p retrieval-worker lexical --lib
cargo test -p platform-api retrieval_search --lib
cargo test -p platform-api assistant_run_supplies_ranked_retrieval_evidence_for_selected_scope --lib
```

必须新增 case：

- `deep_old_chunk_recall`
- `selected_document_scope_blocks_sibling`
- `owner_scope_private_doc_blocks_other_user`
- `cjk_phrase_retrieval_prefers_exact_phrase`
- `legacy_api_shape_preserved`

### 7.6 验收

```text
deep_old_chunk_recall=pass
selected_document_scope=pass
owner_scope_private_doc=pass
cjk_phrase_retrieval=pass
legacy_api_shape=pass
permission_leak_count=0
explain_uses_index=true
legacy_scan_flag_still_available=true
```

### 7.7 回滚

```text
RETRIEVAL_SEARCH_BACKEND=legacy_scan
```

Migration 字段保留，不影响旧逻辑。

## 8. Phase 2：pgvector-first Vector Backend

时间：2 到 4 天

目标：在 Phase 1 稳定后增加真实 dense vector recall。

### 8.1 前置预检

生产/8 服务器先跑：

```sql
select name, installed_version, default_version
from pg_available_extensions
where name = 'vector';
```

如果不可用：

- 不合并 pgvector migration。
- 先补运维安装步骤。
- 或保留 Qdrant adapter 作为临时 backend。

### 8.2 Schema 推荐

MVP 可在 `retrieval_evidences` 上加 embedding 字段，但更推荐单独表，避免模型维度锁死：

```sql
create table if not exists retrieval_embeddings (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null,
    retrieval_evidence_id uuid references retrieval_evidences(id) on delete cascade,
    document_chunk_id uuid not null,
    provider text not null,
    model text not null,
    dimension int not null,
    content_hash text not null,
    embedding_status text not null default 'indexed',
    embedding vector(1536),
    error text,
    indexed_at timestamptz not null default now(),
    unique (tenant_id, document_chunk_id, provider, model, content_hash)
);
```

如果确定只用一个 1536 维模型，也可先简化；但文档和 migration 必须说明维度锁定的代价。

### 8.3 Code

新增 traits：

```rust
trait EmbeddingProvider;
trait VectorIndexWriter;
trait VectorRetriever;
```

实现：

- `DeterministicTestEmbeddingProvider`
- `PgvectorEmbeddingWriter`
- `PgvectorRetriever`
- `NoopVectorRetriever`

### 8.4 验收

```text
vector_backend_disabled_fallback=pass
embedding_coverage_observable=pass
embedding_failure_does_not_block_lexical=pass
same_content_hash_no_duplicate_embedding=pass
permission_filter_before_vector_topk=pass
```

### 8.5 回滚

```text
RETRIEVAL_VECTOR_BACKEND=disabled
```

## 9. Phase 3：Hybrid RRF

时间：2 到 4 天

目标：合并 lexical、vector、facts、database candidates，提升召回和排序。

### 9.1 Candidate Sources

```text
lexical topK
vector topK
dataset_fact_snapshot
document_facts_scoped_aggregate
database_aggregate
selected document fallback chunks
```

### 9.2 RRF

```text
score = rrf(lexical_rank) + rrf(vector_rank) + small_source_quality_boost
RRF_K = 60
source_quality_boost <= 0.03
recency_boost <= 0.02
```

### 9.3 Context Packer

要求：

- 去重。
- 同一文档最多 3 条。
- 跨文档比较时每个文档至少 1 条强证据。
- 相邻 chunk 最多扩展 1 条。
- 数据库 aggregate 优先于 retrieval chunk。

### 9.4 验收

```text
Recall@20 improves over baseline
MRR@20 improves over baseline
permission_leak_count=0
p95_latency <= target
hybrid_disabled_fallback=pass
```

### 9.5 回滚

```text
RETRIEVAL_HYBRID_ENABLED=false
RETRIEVAL_SEARCH_BACKEND=postgres_lexical
RETRIEVAL_RERANK_BACKEND=disabled
```

## 10. Phase 4：Structured Database Query Plan

时间：3 到 5 天

目标：数据库数值、TopN、趋势、汇总问题走受控 query plan，不靠 chunk 检索推断数字。

### 10.1 Query Plan

新增内部结构：

```rust
enum StructuredQueryIntent {
    AggregateTopN,
    AggregateTrend,
    AggregateComparison,
    ExactLookup,
    RowLevelList,
    SchemaExplain,
}

struct DatabaseQueryPlan {
    source_id: String,
    table: String,
    intent: StructuredQueryIntent,
    dimensions: Vec<String>,
    metrics: Vec<String>,
    filters: Vec<SafeFilter>,
    time_range: Option<TimeRange>,
    limit: u32,
    requires_row_level_completeness: bool,
}
```

模型或规则只能生成 plan，不允许直接生成 SQL。Rust 负责 allowlist validate 和 SQL/template 转换。

### 10.2 Row-level Completeness Gate

问题包含以下意图时必须检查 completeness：

```text
全部
每一条
逐行
明细
列表
所有合同
所有记录
```

必须检查：

```text
source_row_count
unique_document_count
unique_chunk_count
collapsed_duplicate_row_count
retrieval_evidence_count
```

如果 collapse > 0 且未确认聚合口径，回答必须说明：

```text
当前数据源只具备实体/快照级证据，不能证明行级完整。
```

### 10.3 验收

```text
topn_uses_database_aggregate=true
trend_uses_structured_query_result=true
numeric_answer_exact_match >= 0.95
collapse_table_does_not_claim_row_complete=true
raw_sql_not_exposed=true
raw_rows_not_exposed=true
```

## 11. Phase 5：Quality Gate

时间：1 到 2 天

目标：检索变更必须有可复制 receipt。

### 11.1 指标

```text
Recall@20
MRR@20
NDCG@20
answer exactness
citation accuracy
permission leak count
p95 latency
fallback rate
```

### 11.2 门槛

冷启动：

```text
Recall@20 >= 0.80
MRR@20 比 baseline 提升 >= 20%
permission_leak_count = 0
database numeric exact match >= 0.95
citation accuracy >= 0.90
```

稳定后：

```text
Recall@20 >= 0.90
MRR@20 >= 0.70
citation accuracy >= 0.95
p95 retrieval latency <= 1500ms for 10k evidence local dataset
```

## 12. 最小可交付版本 Checklist

只做第一版时，范围固定如下。当前勾选项是本地工作树状态，不是线上状态。

本地已试做：

- [x] `fixtures/retrieval-quality/cases.jsonl`
- [x] `scripts/run-retrieval-quality-smoke.sh`
- [x] `docs/validation/retrieval-quality-smoke.md`
- [x] `0014_retrieval_lexical_index.sql`
- [x] retrieval worker 写 `search_terms/indexed_content_hash`，storage upsert 同步 `search_text/search_tsv`
- [x] storage DB-side lexical search
- [x] DB-side ACL-before-ranking
- [x] `search_dataset_retrieval_with_state` feature-flag 切换
- [x] deep-old-chunk test
- [x] selected document scope test
- [x] owner scope test
- [x] CJK phrase retrieval test
- [x] legacy API shape test
- [x] rollback flag verified

仍需补齐后才能进入提交/灰度评审：

- [x] 重新跑完整本地验收命令并确认当前工作树无格式/空白错误。
- [x] 复核 migration 对现有库的兼容性，不包含 destructive operation；8 服务器部署前仍需确认全表回填和建索引锁窗口。
- [x] 补 Phase 0B live/result evaluator，默认不传结果文件时不伪造指标。
- [x] 用真实受控结果文件记录 8 服务器 NewBai retrieval-level live subset metrics。
- [x] 完成 8 服务器只读 preflight，记录当前服务、schema、表规模和默认 legacy runtime 状态。
- [x] 完成 8 服务器 schema/code 部署，保持 `RETRIEVAL_SEARCH_BACKEND=legacy_scan` 默认行为。
- [x] 记录 PostgreSQL lexical 查询的生产/8 服务器 `EXPLAIN` 证据。
- [x] 本地修复 NewBai live subset 暴露的 ranking 弱点，并补 Sheet-summary、月份/天数、ASCII 字段名回归测试。
- [x] 部署 ranking 修正后，用同一 NewBai live subset 重跑 MRR/rank 证据，8 条 expected source 均为 rank `1`。
- [x] 补 NewBai assistant-answer provider-input deterministic smoke，验证模型侧供料包含可答业务证据。
- [x] 用一次性 Postgres fixture 补跑 NewBai AssistantRun DB route smoke，不把共享库保护跳过视为闭环。
- [x] 部署 P1-11 handoff/ranking refinement 到 8 服务器，仅重启 `aiv3-platform-api.service`，并重跑 NewBai live subset，8 条 expected source 均为 rank `1`。
- [ ] 8 服务器启用 `RETRIEVAL_SEARCH_BACKEND=postgres_lexical` 灰度。
- [x] 针对新百报表/数据库混合文档样例补至少 3 条 retrieval-quality case。
- [x] 代码审查确认 selected documents、owner scope、external temporary document scope 没有回退。

最小版不做：

- [ ] pgvector
- [ ] Qdrant
- [ ] real embedding provider
- [ ] reranker
- [ ] structured query plan

## 13. 验收命令

提交或申请发版前必须重跑：

```bash
cargo fmt --check -p storage -p retrieval-worker -p platform-api
cargo check -p storage -p retrieval-worker -p platform-api
cargo test -p storage --lib
cargo test -p retrieval-worker --lib
cargo test -p platform-api postgres_lexical_retrieval_search_prefers_cjk_phrase_match --lib
cargo test -p platform-api postgres_lexical_retrieval_search_recalls_deep_old_chunk_beyond_latest_window --lib
cargo test -p platform-api retrieval_search_backend_parser_defaults_to_legacy_scan --lib
cargo test -p platform-api search_dataset_retrieval_returns_ranked_hits_with_document_ids --lib
cargo test -p platform-api assistant_run_supplies_ranked_retrieval_evidence_for_selected_scope --lib
bash scripts/run-newbai-assistant-answer-smoke.sh
bash scripts/run-retrieval-quality-smoke.sh --baseline
git diff --check
```

快速文档/fixture 验证可用：

```bash
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline
```

后续新增后再纳入 gate：

```bash
cargo test -p platform-api retrieval_hybrid --lib
cargo test -p platform-api structured_query_plan --lib
cargo test -p platform-api row_level_completeness_gate --lib
bash scripts/run-retrieval-quality-smoke.sh --baseline --results-jsonl <path> --require-metrics
bash scripts/run-retrieval-quality-smoke.sh --baseline --live-subset --cases <live-cases.jsonl> --results-jsonl <live-results.jsonl> --require-metrics
```

## 14. 8 服务器发版门槛

Phase 1 允许部署前必须满足：

```text
local tests passed
retrieval-quality baseline generated
deep_old_chunk_recall=pass
permission_leak_count=0
legacy_scan rollback flag verified
no migration destructive operation
GitHub main pushed
user explicitly approves 8 server deployment
```

部署范围：

- 只部署受影响服务。
- Phase 1 通常涉及 `retrieval-worker` 和 `platform-api`。
- 如只新增 schema/storage 但 worker/API 未启用新路径，不重启无关服务。
- 不触碰 120 服务器。
- 不顺手启用 Qdrant/pgvector/录屏/其他无关开关。

部署后 smoke：

```bash
bash scripts/run-retrieval-quality-smoke.sh --base-url https://v3.elepcloud.com
RETRIEVAL_QUALITY_SMOKE_SKIP_CARGO=true bash scripts/run-retrieval-quality-smoke.sh --baseline --live-subset --cases <live-cases.jsonl> --results-jsonl <live-results.jsonl> --require-metrics
curl -s http://127.0.0.1:3000/healthz
curl -s http://127.0.0.1:3000/readyz
journalctl -u aiv3-platform-api.service --since "<deploy time>" -p warning --no-pager
```

## 15. 回滚策略

代码回滚优先顺序：

1. 环境变量回滚到 `RETRIEVAL_SEARCH_BACKEND=legacy_scan`。
2. 重启 `aiv3-platform-api.service` 和必要 worker。
3. 如果 worker 写入新字段有问题，关闭 `RETRIEVAL_LEXICAL_INDEX_BACKFILL_ENABLED`。
4. 保留 migration 字段，不做破坏性 drop。
5. 如必须代码回滚，再回到上一个 Git commit 并重建 affected binaries。

禁止：

- 不直接删除新字段。
- 不清空 retrieval evidence。
- 不修改源业务数据库。
- 不临时绕过权限过滤。

## 16. 下一步执行建议

建议下一步仍只收口 Phase 0 + Phase 1，不进入 pgvector/hybrid。当前不建议直接打开 `postgres_lexical`；P1-8 ranking 已完成检索级闭环，下一步应补 assistant-run/customer-answer 级质量证据，再处理历史 evidence reindex/search_terms 和 legacy vs postgres_lexical 对比：

```text
P1-8: 已完成；8 服务器 NewBai live subset MRR@20=1.0，8 条 expected source 均 rank1
P1-9: 对历史 NewBai evidence 做 reindex 或补 search_terms 回填策略
P1-10: 用同一 live subset 对比 legacy_scan 与 postgres_lexical，记录 MRR/latency 差异
P1-11: provider-input deterministic smoke 和一次性 Postgres fixture AssistantRun DB route smoke 已补；真实 provider/live customer-answer 仍需另行审批补证
P1-12: 通过后再申请 8 服务器 `RETRIEVAL_SEARCH_BACKEND=postgres_lexical` 小流量灰度
```

完成 P1 后再决定是否进入 pgvector。

## 17. 新百/多源数据质量复测

这部分用于覆盖最近出现的回答质量下降和数据库/文档混合供料问题。它不替代报表模板绑定主线；本计划只负责检索供料、证据引用、数值来源和权限范围。

必须补进 `fixtures/retrieval-quality/cases.jsonl` 的 case 类型：

- 新百报表 TopN：回答必须优先使用 `database_aggregate` 或后续 `structured_query_result`，不得只从 retrieval chunk 推断排名。
- 新百报表数值解释：答案中的关键数值必须能回溯到允许的数据源、字段或聚合快照。
- 文档 + 数据库混合：文档负责解释口径，数据库负责数值结论，两者都要进入 evidence pack。
- 模板绑定相关问法：检索回答不得触发“重新生成一百多个无关页面”的副作用；该行为应由报表模板服务的 dataset-template binding 控制。
- row identity collapse：涉及“每一条、全部、明细、列表”时，如果 completeness 不足，必须拒绝行级完整性结论。

通过标准：

```text
newbai_topn_uses_database_candidate=true
newbai_numeric_answer_exact_match=true
mixed_database_document_evidence_pack=true
row_identity_collapse_guard=true
permission_leak_count=0
no_report_template_regeneration_side_effect=true
```

后续如果报表模板主线另建计划，应把以下策略放到模板计划中，而不是塞进 retrieval 实现：

- 数据集有模板后默认长期复用。
- 客户新增要求优先改现有模板。
- 不用 HTML 兜底冒充高质量报表。
- 需要新模板时先生成效果图，再按效果图生成动态数据可视化页面。

## 18. 不建议事项

- 不建议先调 prompt。
- 不建议先直接接 Qdrant。
- 不建议 Phase 1 同时引入 pgvector migration。
- 不建议让模型直接写 SQL。
- 不建议用 retrieval chunk 回答精确数字、TopN、趋势。
- 不建议在 row identity collapse 未修复前声称行级完整。
- 不建议把 vector backend 作为 source of truth。
