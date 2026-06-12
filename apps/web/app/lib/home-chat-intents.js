export function promptRequestsAssistantContinue(prompt) {
  return /继续|接着|下一步|刚才|上面|之前|这个|那版|修改|调整|改成|换成|按计划|照这个|沿用|再来|继续吧/.test(String(prompt || ''));
}

export function promptRequestsStaticPageEdit(prompt) {
  return /继续|接着|下一步|刚才|上面|之前|这个|那版|草稿|标题|文案|内容|数据|图表|布局|模块|调整|修改|改|换|突出|减少|增加|放大|缩小|移动|排序|风格|确认|效果图|导出|老板|高层|风险|柱状图|折线图|环图|看板|精简/.test(String(prompt || ''));
}

export function promptRejectsStaticPageOutput(prompt) {
  const text = String(prompt || '');
  const compact = text.replace(/\s+/g, '');
  if (!compact) {
    return false;
  }
  const asksExistingTemplateDelivery = /(?:已有|现有|旧|原|上次|之前|模板|复用).{0,16}(?:链接|地址|页面|报表|模板|产物)|(?:链接|地址).{0,16}(?:已有|现有|模板|复用|原页面|旧页面)/.test(compact);
  if (asksExistingTemplateDelivery) {
    return false;
  }
  const negativeLead = /(?:不要|别|不用|无需|不需要|禁止|避免|先别|不要再)(?:生成|制作|创建|输出|发布|渲染|做|做成|给|提供|返回|出|产出)?(?:任何|新的|新)?(?:静态页|静态页面|页面|网页|html|HTML|可视化页|报表|看板|链接|页面链接|报表链接)/.test(compact);
  const negativeTail = /(?:静态页|静态页面|页面|网页|html|HTML|可视化页|报表|看板|链接|页面链接|报表链接)(?:也)?(?:不要|别|不用|无需|不需要|禁止|避免)(?:生成|制作|创建|输出|发布|渲染|做|做成|给|提供|返回|出|产出)?/.test(compact);
  return negativeLead || negativeTail;
}

export function promptRequestsCodexForward(prompt) {
  return /^cc(?:$|[\s:：,，.。;；-])/i.test(String(prompt || '').trimStart());
}

export function promptRequestsStaticPage(prompt) {
  const text = String(prompt || '');
  const compact = text.replace(/\s+/g, '');
  if (promptRequestsCodexForward(text)) {
    return false;
  }
  if (promptRejectsStaticPageOutput(text)) {
    return false;
  }
  const hasCreateAction = /生成|制作|创建|输出|发布|渲染|出页面|出报表|做成|做个|做一个|做一份|改成|修改|调整/.test(compact);
  if (/是什么意思|什么含义|怎么计算|如何计算|为什么|口径|有哪些问题|什么问题/.test(compact) && !hasCreateAction) {
    return false;
  }
  if (/静态页|静态页面|页面规划|一页|生成页面|落地页|效果图|网页|html|HTML|可视化页|报表|看板/.test(text)) {
    return true;
  }
  const hasDataAnalysisTopic = /经营分析|经营工作分析|数据分析|业务分析|综合分析|多维分析/.test(compact)
    && /数据|数据集|数据库|指标|经营|销售|客流|租金|门店|店铺|品牌|收入|风险|趋势|排行|排名|明细|汇总|统计|新百|新世界/.test(compact);
  const hasAnalysisReportAction = /做一下|做一做|帮我|请|输出|整理|生成|制作|汇总|全面|完整|详细|系统|多维|多角度|图表|可视化|报告|报表|看板|清单|明细/.test(compact);
  if (hasDataAnalysisTopic && hasAnalysisReportAction) {
    return true;
  }
  const hasRiskIdentificationTopic = /风险识别/.test(compact) && !/风险识别系统/.test(compact);
  const hasBusinessReportTopic = /取高|经营状况|经营情况|经营状态|经营健康度|销售缺口|销售额缺口|需要助推|需助推|助推门店|门店助推|销售统计|门店统计|品牌统计|经营统计|风险门店|风险店铺/.test(compact)
    || hasRiskIdentificationTopic;
  const hasReportAction = /看看|看一下|查看|查一下|哪些|列|列出|统计|汇总|排行|排名|最新|本月|五月|5月|取高|经营状况|经营情况|销售缺口|销售额缺口|需要助推/.test(compact);
  return hasBusinessReportTopic && hasReportAction;
}
