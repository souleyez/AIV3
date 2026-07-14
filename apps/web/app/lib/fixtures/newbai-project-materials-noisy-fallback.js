// This fixture preserves only the noise shapes observed in the Newbai project
// fallback payload. Names, identifiers, amounts, dates, and paths are synthetic.

const documentTitles = [
  '示例经营分析场景',
  'technical_report_alpha',
  '示例经营指标',
  '示例合同预警',
  '示例固定提成规则',
  '示例项目目录',
  '示例低活跃清单一',
  '示例低活跃清单二',
  '示例项目附件',
];

const documents = documentTitles.map((title, index) => {
  const archive = index === documentTitles.length - 1;
  return {
    id: `sanitized-document-${index + 1}`,
    dataset_id: 'fixture-newbai-project-materials',
    dataset_ids: ['fixture-newbai-project-materials'],
    title: `${title}.${archive ? 'zip' : 'xlsx'}`,
    content_type: archive
      ? 'application/zip'
      : 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    lifecycle: 'indexed',
    parse_status: 'completed',
    parse_quality_status: 'ok',
  };
});

export const newbaiProjectMaterialsNoisyFallback = Object.freeze({
  dataset: {
    id: 'fixture-newbai-project-materials',
    key: 'fixture-newbai-project-materials',
    title: '新百项目资料（脱敏基线）',
    document_count: 9,
    estimated_word_count: 55919,
    parse_status_summary: 'completed:9,indexed:9',
    content_type_summary: 'spreadsheet:8,other:1',
    document_title_hints: documentTitles,
    material_hints: [],
    noun_term_hints: [
      '2026',
      '1001\t示例门店\t200001\t123456.78\t2026-01-01',
      ') + 1 as days,',
      '/*示例日均值*/',
      'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      'paragraph_aware_noun_terms_v1',
      '00000000-1111-4222-8333-444444444444',
      'C:\\internal\\example\\source.xlsx',
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      '示例知识点',
      '经营指标',
      '合同预警',
      '低活跃品牌',
      '销售情况',
      '示例字段',
      '数据理解',
    ],
    section_title_hints: [
      '2026',
      '001\t示例字段\t9999',
      'select date_add(day, 1, dt) from sample_table',
      '/*SQL 示例注释*/',
      'application/zip',
      'sheet_profile_v1',
      '结构摘要',
      '经营结构',
      '预警结构',
      '数据来源',
    ],
    document_understanding_strategies: ['paragraph_aware_noun_terms_v1'],
  },
  documents,
  understanding: {
    schema_version: '1.0.0',
    generation_version: 'semantic_profile_v3',
    status: 'empty',
    dataset: {
      id: 'fixture-newbai-project-materials',
      title: '新百项目资料（脱敏基线）',
    },
    objects: [],
    fields: [],
    relations: [],
  },
});

