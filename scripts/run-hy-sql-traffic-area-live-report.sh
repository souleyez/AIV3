#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_id="${HY_SQL_TRAFFIC_SOURCE_ID:-hy-sql-traffic-area}"
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

post_json \
  "${api_base}/v1/external/sources/${source_id}/database/profile" \
  '{"sample_limit":100,"database_source":{}}' \
  "${work_dir}/profile.json"

post_json \
  "${api_base}/v1/external/sources/${source_id}/database/aggregate" \
  '{"table":"bi_traffic_area","dimensions":["area_name"],"metric":"traffic_count","aggregation":"sum","limit":10,"database_source":{}}' \
  "${work_dir}/area-ranking.json"

post_json \
  "${api_base}/v1/external/sources/${source_id}/database/aggregate" \
  '{"table":"bi_traffic_area","dimensions":["stat_date"],"metric":"traffic_count","aggregation":"sum","limit":30,"database_source":{}}' \
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

const profile = profileResponse.profile || {};
const table =
  (profile.tables || []).find((item) => item.name === 'bi_traffic_area') ||
  (profile.tables || [])[0] ||
  {};
const areaRows = ((areaResponse.result || {}).rows || []).map((row) => ({
  label: String(row.area_name ?? row.areaName ?? row.dimension ?? '未命名区域'),
  value: Number(row.value ?? 0),
  kind: 'evidence_value',
}));
const trendRows = ((trendResponse.result || {}).rows || []).map((row) => ({
  label: String(row.stat_date ?? row.statDate ?? row.dimension ?? '未命名日期'),
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
      '基于 hy_sql.bi_traffic_area 的真实数据库 profile 与 aggregate 结果生成。V3 将数据库 source 作为数据集理解入口，先识别字段语义，再按区域与日期生成经营报表。',
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
            { label: 'Top 区域', value: topArea },
          ],
        },
        layout: { x: 0, y: 0, w: 4, h: 3 },
      },
      {
        id: 'area-ranking',
        title: '区域流量排行',
        content:
          '按 area_name 聚合 traffic_count，展示区域经营贡献。可继续扩展为 TopN、区域筛选和异常区域追踪。',
        dataBinding: {
          label: '区域总流量',
          sourceId,
          fieldPath: 'aggregate.area_name.sum_traffic_count',
          evidenceIds: ['aggregate-area-ranking'],
        },
        visualization: {
          type: 'bar-chart',
          label: '区域流量排行',
          chartRuntime: 'echarts',
          chartOptions: {
            title: { text: '区域流量排行' },
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
          '按 stat_date 聚合 traffic_count，观察整体流量走势。后续可加入同比、环比、节假日和异常点解释。',
        dataBinding: {
          label: '日期趋势',
          sourceId,
          fieldPath: 'aggregate.stat_date.sum_traffic_count',
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
    tables: [table.name || 'bi_traffic_area'],
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
