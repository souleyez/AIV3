import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildKnowledgeGraphLenses,
  KNOWLEDGE_GRAPH_LENS_PROFILES,
  resolveKnowledgeGraphLensProfile,
} from './knowledge-graph-lenses.js';

function node(id, name, overrides = {}) {
  return {
    id,
    name,
    kind: 'field',
    entityType: 'field',
    datasetRefs: ['resume-a'],
    businessScore: 50,
    ...overrides,
  };
}

function resumeModel() {
  return {
    title: '候选人简历库',
    nodes: [
      node('field:quality', '待解释字段', { kind: 'unresolved' }),
      node('field:location', '工作地点', { semanticRole: 'location' }),
      node('field:time', '最近年份', { semanticRole: 'date' }),
      node('field:school', '毕业院校'),
      node('field:degree', '最高学历'),
      node('field:skill', '技能能力'),
      node('field:project', '项目经历'),
      node('field:position', '岗位职责'),
      node('field:company', '任职公司'),
      node('field:candidate', '候选人姓名', {
        examples: ['张三', '13800138000', 'person@example.com'],
        rawLabel: '张三',
        technicalName: 'PERSON_NAME',
      }),
    ],
    links: [
      {
        id: 'edge:confirmed',
        source: 'field:candidate',
        target: 'field:company',
        type: 'confirmed',
      },
      {
        id: 'edge:observed',
        source: 'field:company',
        target: 'field:position',
        evidenceClass: 'observed',
      },
    ],
  };
}

test('built-in profiles expose stable generic and resume lens keys', () => {
  assert.deepEqual(
    KNOWLEDGE_GRAPH_LENS_PROFILES.generic.map((lens) => lens.key),
    ['entity', 'organization', 'time', 'location', 'metric', 'classification', 'quality'],
  );
  assert.deepEqual(
    KNOWLEDGE_GRAPH_LENS_PROFILES.resume.map((lens) => lens.key),
    ['resume-structure', 'candidate', 'experience', 'project', 'capability', 'education', 'time', 'location', 'quality'],
  );
});

test('auto profile detects resume semantics while an explicit generic profile wins', () => {
  const model = resumeModel();
  assert.equal(resolveKnowledgeGraphLensProfile(model), 'resume');
  assert.equal(resolveKnowledgeGraphLensProfile(model, 'generic'), 'generic');
  assert.throws(
    () => resolveKnowledgeGraphLensProfile(model, 'unknown'),
    /unknown knowledge graph lens profile/,
  );
});

test('resume lenses cover existing semantic nodes without carrying raw personal values', () => {
  const result = buildKnowledgeGraphLenses(resumeModel());

  assert.equal(result.profile, 'resume');
  assert.deepEqual(result.facets.map((facet) => facet.key), [
    'candidate',
    'experience',
    'project',
    'capability',
    'education',
    'time',
    'location',
    'quality',
  ]);
  assert.deepEqual(result.privacy, {
    usesRawExamples: false,
    identityMerging: false,
    sharedSkillSemantics: 'concept_only',
  });
  const serialized = JSON.stringify(result);
  assert.doesNotMatch(serialized, /张三|13800138000|person@example\.com|PERSON_NAME/);
  assert.doesNotMatch(serialized, /examples|rawLabel|technicalName/);
  const inputIds = new Set(resumeModel().nodes.map((item) => item.id));
  result.facets.flatMap((facet) => facet.nodeIds)
    .forEach((id) => assert.equal(inputIds.has(id), true));
});

test('resume fallback exposes only safe structure nodes while semantic snapshot is pending', () => {
  const result = buildKnowledgeGraphLenses({
    title: '简历',
    nodes: [
      node('document:person-a', '1777027532662-张某', {
        kind: 'document',
        entityType: 'document',
      }),
      node('structure:architecture', '认证解决方案架构师专家', {
        kind: 'section',
        entityType: 'section',
      }),
      node('structure:project', '中国移动家庭印相业务', {
        kind: 'section',
        entityType: 'section',
      }),
    ],
    links: [],
  });

  assert.equal(result.profile, 'resume');
  assert.deepEqual(result.facets.map((facet) => facet.key), ['resume-structure']);
  assert.deepEqual([...result.facets[0].nodeIds].sort(), [
    'structure:architecture',
    'structure:project',
  ].sort());
  assert.doesNotMatch(JSON.stringify(result), /张某|document:person-a/);
});

test('same candidate field labels remain separate nodes and never become an identity merge', () => {
  const model = {
    title: '简历字段图谱',
    nodes: [
      node('field:candidate:a', '候选人姓名'),
      node('field:candidate:b', '候选人姓名', { datasetRefs: ['resume-b'] }),
    ],
    links: [{
      id: 'edge:similarity',
      source: 'field:candidate:a',
      target: 'field:candidate:b',
      type: 'inferred',
      relationSemantics: 'similarity',
    }],
  };

  const result = buildKnowledgeGraphLenses(model);
  const candidate = result.facets.find((facet) => facet.key === 'candidate');
  assert.deepEqual(candidate.nodeIds, ['field:candidate:a', 'field:candidate:b']);
  assert.equal(candidate.nodeCount, 2);
  assert.equal(candidate.evidenceCounts.inferred, 1);
  assert.equal(result.privacy.identityMerging, false);
});

test('shared skills are admitted only as concepts and never projected as identity', () => {
  const model = {
    title: '候选人能力简历',
    nodes: [
      node('shared:concept:skill', '技能能力', {
        kind: 'concept',
        entityType: 'concept',
        datasetRefs: ['resume-a', 'resume-b'],
        shared: true,
      }),
      node('shared:field:skill', '技能字段', {
        datasetRefs: ['resume-a', 'resume-b'],
        shared: true,
      }),
    ],
    links: [],
  };

  const result = buildKnowledgeGraphLenses(model);
  const capability = result.facets.find((facet) => facet.key === 'capability');
  assert.deepEqual(capability.nodeIds, ['shared:concept:skill']);
  assert.equal(capability.shared, true);
  assert.equal(result.privacy.sharedSkillSemantics, 'concept_only');
});

test('lens projection is deterministic when backend nodes and links arrive in reverse order', () => {
  const model = resumeModel();
  const forward = buildKnowledgeGraphLenses(model);
  const reversed = buildKnowledgeGraphLenses({
    ...model,
    nodes: [...model.nodes].reverse(),
    links: [...model.links].reverse(),
  });

  assert.deepEqual(forward, reversed);
});

test('generic profile uses semantic roles without inventing nodes', () => {
  const model = {
    title: '经营数据集',
    nodes: [
      node('object:order', '订单对象', { kind: 'object', entityType: 'object' }),
      node('field:date', '业务日期', { semanticRole: 'date' }),
      node('field:amount', '销售金额', { semanticRole: 'amount' }),
      node('field:status', '订单状态', { semanticRole: 'status' }),
      node('field:unsafe-email', 'person@example.com'),
      node('field:unsafe-phone', '13800138000'),
    ],
    links: [],
  };

  const result = buildKnowledgeGraphLenses(model, { profile: 'generic' });
  assert.deepEqual(result.facets.map((facet) => facet.key), [
    'entity',
    'time',
    'metric',
    'classification',
  ]);
  assert.equal(result.unmatchedNodeCount, 0);
  assert.doesNotMatch(JSON.stringify(result), /person@example\.com|13800138000/);
});
