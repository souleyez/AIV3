#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_id="${HY_SQL_TRAFFIC_SOURCE_ID:-hy-sql-traffic-area}"
preferred_table="${HY_SQL_TRAFFIC_TABLE:-bi_traffic_area}"
api_base="${PLATFORM_API_BASE_URL:-http://127.0.0.1:3000}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
work_dir="${HY_SQL_TRAFFIC_REPORT_DIR:-${repo_root}/target/database-static-pages/hy-sql-bi-traffic-area-live-${stamp}}"
artifact_dir="${work_dir}/artifact"

mkdir -p "$work_dir"

post_json() {
  local url="$1"
  local body="$2"
  local output="$3"
  local status
  status="$(
    curl -sS -m 60 \
      -H 'Content-Type: application/json' \
      -o "$output" \
      -w '%{http_code}' \
      -X POST "$url" \
      -d "$body"
  )"
  if [[ "$status" -lt 200 || "$status" -ge 300 ]]; then
    echo "Request failed: ${url} HTTP ${status}" >&2
    cat "$output" >&2
    exit 1
  fi
}

post_json_file() {
  local url="$1"
  local body_file="$2"
  local output="$3"
  local status
  status="$(
    curl -sS -m 60 \
      -H 'Content-Type: application/json' \
      -o "$output" \
      -w '%{http_code}' \
      -X POST "$url" \
      -d @"$body_file"
  )"
  if [[ "$status" -lt 200 || "$status" -ge 300 ]]; then
    echo "Request failed: ${url} HTTP ${status}" >&2
    cat "$output" >&2
    exit 1
  fi
}

node - "$work_dir" "$preferred_table" <<'NODE'
const fs = require('fs');
const path = require('path');

const workDir = process.argv[2];
const preferredTable = process.argv[3];
fs.writeFileSync(
  path.join(workDir, 'apply-profile-request.json'),
  JSON.stringify(
    {
      sample_limit: 100,
      database_source: { tables: [] },
      tables: [preferredTable],
    },
    null,
    2
  )
);
NODE

post_json_file \
  "${api_base}/v1/external/sources/${source_id}/database/apply-profile" \
  "${work_dir}/apply-profile-request.json" \
  "${work_dir}/apply-profile.json"

post_json \
  "${api_base}/v1/external/sources/${source_id}/database/profile" \
  '{"sample_limit":100,"database_source":{}}' \
  "${work_dir}/profile.json"

node - "$work_dir" "$preferred_table" <<'NODE'
const fs = require('fs');
const path = require('path');

const workDir = process.argv[2];
const preferredTable = process.argv[3];
const profileResponse = JSON.parse(fs.readFileSync(path.join(workDir, 'profile.json'), 'utf8'));
const profile = profileResponse.profile || {};
const tables = profile.tables || [];

const scoreTable = (table) =>
  (table.name === preferredTable ? 100 : 0) +
  ((table.metrics || []).length ? 10 : 0) +
  ((table.dimensions || []).length ? 6 : 0) +
  ((table.time_dimensions || []).length ? 4 : 0);
const table = [...tables].sort((a, b) => scoreTable(b) - scoreTable(a))[0] || {};

const uniq = (items) => [...new Set((items || []).filter(Boolean))];
const includes = (items, value) => (items || []).some((item) => item === value);
const pickBy = (items, preferred, patterns) => {
  const list = uniq(items);
  if (includes(list, preferred)) return preferred;
  let best = null;
  let bestScore = Number.POSITIVE_INFINITY;
  for (const item of list) {
    for (let index = 0; index < patterns.length; index += 1) {
      if (patterns[index].test(item) && index < bestScore) {
        best = item;
        bestScore = index;
      }
    }
  }
  return best || list[0] || null;
};

const mapping = table.suggested_mapping || {};
const dimensions = uniq([
  ...(table.dimensions || []),
  ...(table.entity_columns || []),
  mapping.title_column,
]);
const metrics = uniq(table.metrics || []);
const timeDimensions = uniq(table.time_dimensions || []);
const rankDimension = pickBy(dimensions, 'areaname', [
  /^areaname$/i,
  /^area_name$/i,
  /area.*name/i,
  /region.*name/i,
  /city.*name/i,
  /name/i,
  /区域名/,
  /地区名/,
  /城市名/,
  /area/i,
  /region/i,
  /city/i,
  /区域/,
  /地区/,
  /城市/,
]);
const metric = pickBy(metrics, 'up', [/traffic/i, /count/i, /^up$/i, /^down$/i, /amount/i, /value/i, /流量/, /数量/, /总量/, /金额/, /值/]);
const timeDimension = pickBy(timeDimensions, 'stat_date', [/date/i, /time/i, /day/i, /month/i, /日期/, /时间/, /月份/, /天/]);
const aggregation = metric ? 'sum' : 'count';
const tableName = table.name || preferredTable;

const aggregateRequest = (dimensionsForRequest, limit) => ({
  table: tableName,
  dimensions: dimensionsForRequest.filter(Boolean),
  metric,
  aggregation,
  limit,
  database_source: {},
});

const plan = {
  database: profile.database,
  table: tableName,
  preferredTable,
  rankDimension,
  metric,
  aggregation,
  timeDimension,
  dimensions,
  metrics,
  timeDimensions,
  sourceProfile: {
    tableCount: tables.length,
    selectedTableColumnCount: table.column_count || 0,
  },
};

fs.writeFileSync(path.join(workDir, 'aggregate-plan.json'), JSON.stringify(plan, null, 2));
fs.writeFileSync(path.join(workDir, 'area-aggregate-request.json'), JSON.stringify(aggregateRequest(rankDimension ? [rankDimension] : [], 10), null, 2));
fs.writeFileSync(path.join(workDir, 'trend-aggregate-request.json'), JSON.stringify(aggregateRequest(timeDimension ? [timeDimension] : rankDimension ? [rankDimension] : [], timeDimension ? 30 : 10), null, 2));
NODE

post_json_file \
  "${api_base}/v1/external/sources/${source_id}/database/aggregate" \
  "${work_dir}/area-aggregate-request.json" \
  "${work_dir}/area-ranking.json"

post_json_file \
  "${api_base}/v1/external/sources/${source_id}/database/aggregate" \
  "${work_dir}/trend-aggregate-request.json" \
  "${work_dir}/date-trend.json"

node - "$work_dir" "$source_id" "$stamp" <<'NODE'
const fs = require('fs');
const path = require('path');

const workDir = process.argv[2];
const sourceId = process.argv[3];
const stamp = process.argv[4];
const readJson = (name) => JSON.parse(fs.readFileSync(path.join(workDir, name), 'utf8'));
const profileResponse = readJson('profile.json');
const areaResponse = readJson('area-ranking.json');
const trendResponse = readJson('date-trend.json');
const aggregatePlan = readJson('aggregate-plan.json');

const profile = profileResponse.profile || {};
const table =
  (profile.tables || []).find((item) => item.name === aggregatePlan.table) ||
  (profile.tables || [])[0] ||
  {};
const labelFromRow = (row, dimension, fallback) =>
  String(
    (dimension && row[dimension] != null ? row[dimension] : undefined) ??
      row.dimension ??
      row.label ??
      fallback
  );
const areaRows = ((areaResponse.result || {}).rows || []).map((row) => ({
  label: labelFromRow(row, aggregatePlan.rankDimension, '总计'),
  value: Number(row.value ?? 0),
  kind: 'evidence_value',
}));
const trendRows = ((trendResponse.result || {}).rows || []).map((row) => ({
  label: labelFromRow(row, aggregatePlan.timeDimension || aggregatePlan.rankDimension, '总计'),
  value: Number(row.value ?? 0),
  kind: 'evidence_value',
}));
const topArea = areaRows[0]?.label || '暂无';
const totalTraffic = areaRows.reduce((sum, row) => sum + (Number.isFinite(row.value) ? row.value : 0), 0);

const request = {
  draft_id: `db-bi-traffic-area-live-${stamp}`,
  assistant_run_id: `db-report-live-${stamp}`,
  title: '区域流量经营分析报表 - 真实库数据版',
  draft_payload: {
    styleDirection: 'client-delivery',
    modelSummary:
      `基于 ${profile.database || '数据库'}.${aggregatePlan.table} 的真实数据库 profile 与 aggregate 结果生成。V3 将数据库 source 作为数据集理解入口，先识别字段语义，再按维度、指标和时间轴生成经营报表。`,
    visualSpec: {
      palette: {
        background: '#f7faf8',
        surface: '#ffffff',
        text: '#17211d',
        muted: '#5f6f68',
        accent: '#0f766e',
        chart: '#2563eb',
      },
    },
    renderSpec: {
      componentModel: 'dom-text-svg-chart',
      responsive: true,
      finalHtmlSource: 'static-page-renderer-v1',
    },
    previewContract: {
      status: 'confirmed',
      source: 'database-live-profile-aggregate',
      assetKey: 'previews/hy-sql-bi-traffic-area-live.png',
      draftFingerprint: `hy-sql-bi-traffic-area-live-${stamp}`,
      confirmedAt: new Date().toISOString(),
    },
    mobileOrder: ['business-overview', 'area-ranking', 'traffic-trend', 'qa-playbook'],
    modules: [
      {
        id: 'business-overview',
        title: '经营概览',
        content:
          '从真实库表识别区域、流量、日期等经营分析口径。该页适合回答总量、排名、趋势和报表输出类问题。',
        dataBinding: {
          label: '真实库语义画像',
          sourceId,
          fieldPath: 'database.profile.overview',
          evidenceIds: ['profile-bi_traffic_area'],
        },
        visualization: {
          type: 'kpi-cards',
          data: [
            { label: '字段数', value: String(table.column_count ?? 0) },
            { label: '维度数', value: String((table.dimensions || []).length) },
            { label: '指标数', value: String((table.metrics || []).length) },
            { label: 'Top 对象', value: topArea },
          ],
        },
        layout: { x: 0, y: 0, w: 4, h: 3 },
      },
      {
        id: 'area-ranking',
        title: '区域流量排行',
        content:
          `按 ${aggregatePlan.rankDimension || '全表'} 聚合 ${aggregatePlan.metric || '记录数'}，展示核心对象贡献。可继续扩展为 TopN、筛选和异常对象追踪。`,
        dataBinding: {
          label: '维度排行',
          sourceId,
          fieldPath: `aggregate.${aggregatePlan.rankDimension || 'all'}.${aggregatePlan.aggregation}_${aggregatePlan.metric || 'records'}`,
          evidenceIds: ['aggregate-area-ranking'],
        },
        visualization: {
          type: 'bar-chart',
          label: '维度排行',
          chartRuntime: 'echarts',
          chartOptions: {
            title: { text: '维度排行' },
            xAxis: { type: 'category' },
            yAxis: { type: 'value' },
            series: [{ type: 'bar', data: areaRows.map((row) => row.value) }],
          },
        },
        layout: { x: 4, y: 0, w: 4, h: 3 },
      },
      {
        id: 'traffic-trend',
        title: '日期趋势',
        content:
          `按 ${aggregatePlan.timeDimension || aggregatePlan.rankDimension || '全表'} 聚合 ${aggregatePlan.metric || '记录数'}，观察整体变化。后续可加入同比、环比和异常点解释。`,
        dataBinding: {
          label: '日期趋势',
          sourceId,
          fieldPath: `aggregate.${aggregatePlan.timeDimension || aggregatePlan.rankDimension || 'all'}.${aggregatePlan.aggregation}_${aggregatePlan.metric || 'records'}`,
          evidenceIds: ['aggregate-date-trend'],
        },
        visualization: {
          type: 'line-chart',
          label: '总流量趋势',
        },
        layout: { x: 8, y: 0, w: 4, h: 3 },
      },
      {
        id: 'qa-playbook',
        title: '问答与报表建议',
        content:
          '数据库接入后，模型应优先理解业务对象、指标口径、维度枚举和时间粒度，再按用户要求输出富文本、MD 表格、JSON 或静态 HTML 报表。',
        dataBinding: {
          label: '报表行动建议',
          sourceId,
          fieldPath: 'report.playbook',
          evidenceIds: ['database-report-suggestion'],
        },
        visualization: {
          type: 'kpi-cards',
          data: [
            { label: '总流量', value: String(Math.round(totalTraffic)) },
            { label: '区域项', value: String(areaRows.length) },
            { label: '趋势点', value: String(trendRows.length) },
            { label: '输出', value: 'HTML' },
          ],
        },
        layout: { x: 0, y: 3, w: 12, h: 3 },
      },
    ],
    dataSnapshot: {
      source: 'hy-sql-bi-traffic-area-live-profile-aggregate',
      semanticProfile: {
        database: profile.database,
        table: table.name,
        dimensions: table.dimensions || [],
        metrics: table.metrics || [],
        timeDimensions: table.time_dimensions || [],
        selectedRankDimension: aggregatePlan.rankDimension,
        selectedMetric: aggregatePlan.metric,
        selectedAggregation: aggregatePlan.aggregation,
        selectedTimeDimension: aggregatePlan.timeDimension,
        suggestedQuestions: table.suggested_questions || [],
      },
      module_bindings: [
        {
          moduleId: 'business-overview',
          sampleData: [
            { label: '字段数', value: Number(table.column_count || 0), kind: 'module_data' },
            { label: '维度数', value: (table.dimensions || []).length, kind: 'module_data' },
            { label: '指标数', value: (table.metrics || []).length, kind: 'module_data' },
            { label: '时间字段数', value: (table.time_dimensions || []).length, kind: 'module_data' },
          ],
          dataQuality: 'module_data',
          bindingQuality: { status: 'confirmed', chartDataFit: 'ready', reason: 'live_database_profile' },
        },
        {
          moduleId: 'area-ranking',
          sampleData: areaRows,
          dataQuality: 'evidence_value',
          bindingQuality: { status: 'confirmed', chartDataFit: 'ready', reason: 'live_database_aggregate' },
        },
        {
          moduleId: 'traffic-trend',
          sampleData: trendRows,
          dataQuality: 'evidence_value',
          bindingQuality: { status: 'confirmed', chartDataFit: 'ready', reason: 'live_database_aggregate' },
        },
        {
          moduleId: 'qa-playbook',
          sampleData: [
            { label: '可问排序', value: 1, kind: 'module_data' },
            { label: '可问趋势', value: 1, kind: 'module_data' },
            { label: '可问异常', value: 1, kind: 'module_data' },
            { label: '可生成报表', value: 1, kind: 'module_data' },
          ],
          dataQuality: 'module_data',
          bindingQuality: { status: 'confirmed', chartDataFit: 'ready', reason: 'report_playbook_ready' },
        },
      ],
    },
  },
  selected_scope: {
    mode: 'database_source_live',
    sourceId,
    database: profile.database,
    tables: [table.name || aggregatePlan.table],
  },
  visibility_snapshot: {
    policy: 'database_source_live_profile_aggregate',
    liveData: true,
  },
  preview_asset_key: 'previews/hy-sql-bi-traffic-area-live.png',
  image_job_id: `db-report-live-image-${stamp}`,
};

fs.writeFileSync(path.join(workDir, 'database-static-page-request.json'), JSON.stringify(request, null, 2));
NODE

cargo run -p static-page-renderer --example render_static_page_request -- \
  "${work_dir}/database-static-page-request.json" \
  "$artifact_dir"

node "${repo_root}/tools/validate-static-page-export-artifact.mjs" --artifact "$artifact_dir"

echo "Live database report artifact: ${artifact_dir}"
