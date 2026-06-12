import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  promptRejectsStaticPageOutput,
  promptRequestsAssistantContinue,
  promptRequestsCodexForward,
  promptRequestsStaticPage,
  promptRequestsStaticPageEdit,
} from './home-chat-intents.js';

describe('home chat intent helpers', () => {
  it('detects assistant continuation prompts without requiring a static page request', () => {
    assert.equal(promptRequestsAssistantContinue('继续吧'), true);
    assert.equal(promptRequestsAssistantContinue('按计划继续'), true);
    assert.equal(promptRequestsAssistantContinue('今天销售怎么样'), false);
  });

  it('detects static page edit prompts used with an active draft', () => {
    assert.equal(promptRequestsStaticPageEdit('把标题改短，图表换成柱状图'), true);
    assert.equal(promptRequestsStaticPageEdit('移动风险模块，精简一点'), true);
    assert.equal(promptRequestsStaticPageEdit('邓工是谁'), false);
  });

  it('detects CC forwarding prefixes only at the prompt start', () => {
    assert.equal(promptRequestsCodexForward('cc 帮我接入数据库API'), true);
    assert.equal(promptRequestsCodexForward(' CC：检查数据源 '), true);
    assert.equal(promptRequestsCodexForward('请解释 cc 模式是什么'), false);
    assert.equal(promptRequestsStaticPage('cc 生成一个经营分析报表'), false);
  });

  it('honors explicit static page output rejection while allowing existing template links', () => {
    assert.equal(promptRejectsStaticPageOutput('不要生成新的静态页，先回答问题'), true);
    assert.equal(promptRejectsStaticPageOutput('报表链接也不要给'), true);
    assert.equal(promptRejectsStaticPageOutput('把之前模板链接发我'), false);
    assert.equal(promptRequestsStaticPage('不要生成新的报表，先解释口径'), false);
  });

  it('triggers static page/report flow for explicit creation and business report requests', () => {
    assert.equal(promptRequestsStaticPage('基于新百数据生成一个经营分析报表'), true);
    assert.equal(promptRequestsStaticPage('看看经营健康度和风险门店'), true);
    assert.equal(promptRequestsStaticPage('列出本月销售缺口和需要助推的门店'), true);
    assert.equal(promptRequestsStaticPage('做一份多维数据分析看板'), true);
  });

  it('keeps known explanatory questions from triggering report generation', () => {
    assert.equal(promptRequestsStaticPage('取高是什么意思？'), false);
    assert.equal(promptRequestsStaticPage('风险识别系统有哪些项目经历？'), false);
    assert.equal(promptRequestsStaticPage('销售缺口怎么计算'), false);
    assert.equal(promptRequestsStaticPage('经营健康度是什么含义'), false);
  });

  it('requires action context for broad data-analysis phrases', () => {
    assert.equal(promptRequestsStaticPage('经营分析数据里有哪些指标'), false);
    assert.equal(promptRequestsStaticPage('请按经营分析数据整理一份完整报表'), true);
  });
});
