#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEFAULT_OUTPUT = 'target/html-artifacts/third-party-handoff-document.json';

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith('--')) {
      continue;
    }
    parsed[arg.slice(2)] = argv[index + 1] || true;
    if (argv[index + 1] && !argv[index + 1].startsWith('--')) {
      index += 1;
    }
  }
  return parsed;
}

function ensureParentDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function thirdPartyHandoffHtmlArtifactManifest({ generatedAt = new Date().toISOString(), head = '' } = {}) {
  return {
    kind: 'html_artifact',
    version: 1,
    id: 'html-artifact-third-party-handoff-document',
    title: 'V3 纯第三方模式对接文档',
    source_type: 'external_integration',
    template_id: 'third_party_handoff_document',
    owner_scope: {
      type: 'external_integration_handoff',
      id: 'pure-third-party',
    },
    data_refs: [
      {
        kind: 'source_document',
        id: 'docs/pure-third-party-integration-guide.zh-CN.md',
        label: '纯第三方模式 Markdown 源文档',
      },
      {
        kind: 'review_document',
        id: 'docs/pure-third-party-integration-guide.zh-CN.html',
        label: '纯第三方模式 HTML 阅读版',
      },
      {
        kind: 'api_document',
        id: 'docs/third-party-integration-api.zh-CN.md',
        label: '第三方标准接口中文说明',
      },
      {
        kind: 'handoff_manifest',
        id: 'handoff/third-party-handoff.sample.json',
        label: '第三方沙箱交接清单样例',
      },
    ],
    provenance: {
      producer: 'v3-handoff-package-builder',
      reason: 'third-party handoff review artifact',
      source_run_id: head || null,
    },
    interaction_mode: 'read_only',
    created_at: generatedAt,
    payload: {
      handoff: {
        status: 'review_ready',
        defaultDomain: 'v3.elepcloud.com',
        version: 'v0.1',
        audience: '第三方 IT 对接团队',
        sourceDocument: 'docs/pure-third-party-integration-guide.zh-CN.md',
        reviewDocument: 'docs/pure-third-party-integration-guide.zh-CN.html',
      },
      summary: '第三方可自建页面、文档库、用户权限、产物与业务接口，V3 负责权限供料、模型回复、动作确认和审计观测。',
      integrationModes: [
        {
          title: '飞书/企微标准接口',
          detail: 'V3 接官方消息、事件、卡片、文件和回调接口，企业资料仍按权限进入 V3 对话链路。',
          status: 'standard_channel',
        },
        {
          title: '纯第三方模式',
          detail: '第三方托管聊天页面、文档库、用户目录、产物系统和业务动作 endpoint，V3 通过标准事件与回调协作。',
          status: 'customer_hosted',
        },
        {
          title: '混合模式',
          detail: '聊天面可在飞书、企微或第三方页面，知识、权限和动作来自第三方服务器，V3 只消费本轮授权范围。',
          status: 'supported',
        },
      ],
      endpoints: [
        {
          method: 'POST',
          path: '/v1/external/channels/{connection_id}/events',
          description: '接收第三方聊天页面或渠道转发的标准消息事件。',
          auth: 'channel signing',
        },
        {
          method: 'POST',
          path: '/v1/external/channels/{connection_id}/confirmations',
          description: '接收用户确认后的事务处理请求，V3 侧继续做动作校验和审计。',
          auth: 'channel signing',
        },
        {
          method: 'POST',
          path: '/v1/external/channels/{connection_id}/actions/{action_id}/result',
          description: '接收第三方异步动作执行结果回调，并写入脱敏状态摘要。',
          auth: 'result callback signing',
        },
        {
          method: 'GET',
          path: '/v1/external/integrations',
          description: 'V3 观测页读取外部集成状态、渠道、动作和搜索证据摘要。',
          auth: 'operator session',
        },
        {
          method: 'GET',
          path: '/v1/external/integrations/{id}/audit',
          description: '按消息、同步、动作、回调、搜索证据等类型查看脱敏审计时间线。',
          auth: 'operator session',
        },
      ],
      deliveryArtifacts: [
        {
          title: 'HTML 阅读版',
          path: 'docs/pure-third-party-integration-guide.zh-CN.html',
          description: '适合业务和技术评审先读，不作为唯一源数据。',
        },
        {
          title: 'Markdown 源文档',
          path: 'docs/pure-third-party-integration-guide.zh-CN.md',
          description: '第三方纯模式的可编辑源文档。',
        },
        {
          title: '接口说明',
          path: 'docs/third-party-integration-api.zh-CN.md',
          description: '飞书、企微和纯第三方接口的完整中文说明。',
        },
        {
          title: '沙箱清单样例',
          path: 'handoff/third-party-handoff.sample.json',
          description: '第三方填写沙箱文档、用户、权限、回调和联系人信息的结构样例。',
        },
        {
          title: '安全 HTML artifact manifest',
          path: 'html-artifacts/third-party-handoff-document.json',
          description: 'V3 受信模板可直接渲染的第三方交接摘要。',
        },
      ],
      validationCommands: [
        {
          title: '交接清单校验',
          command: 'npm run validate:handoff',
          when: '填写第三方沙箱清单后',
        },
        {
          title: '包完整性校验',
          command: 'npm run validate:package',
          when: '收到或解压交接包后',
        },
        {
          title: '归档校验',
          command: 'npm run validate:archive',
          when: '包目录与归档文件同级存在时',
        },
        {
          title: '交付物校验',
          command: 'npm run validate:delivery',
          when: '正式发送或接收交付物前',
        },
        {
          title: '一键聚合校验',
          command: 'npm run validate:all',
          when: '需要生成统一交接 gate 时',
        },
        {
          title: '最终证据校验',
          command: 'npm run validate:evidence',
          when: '需要归档最终交接证据时',
        },
      ],
      reviewChecklist: [
        {
          title: '权限样例',
          detail: '用户、部门、组和 ACL 样例需覆盖可见、不可见和跨等级访问场景。',
          owner: 'third_party',
        },
        {
          title: '文档接口',
          detail: '列清索引增量、片段读取、原文读取、权限过滤和不可见返回的行为。',
          owner: 'third_party',
        },
        {
          title: '聊天通道',
          detail: '确认消息事件、会话 ID、用户映射、确认交互和重试幂等字段稳定。',
          owner: 'joint',
        },
        {
          title: '事务处理',
          detail: '动作执行需先确认，再由第三方返回状态、结构化摘要和可审计 request id。',
          owner: 'joint',
        },
        {
          title: '观测面板',
          detail: 'V3 主观测页默认使用 v3.elepcloud.com，对外集成面板不直接跳回主站。',
          owner: 'v3',
        },
      ],
      safetyRules: [
        {
          title: '不搬迁整库',
          detail: 'V3 按需读取、增量索引和权限过滤，不要求第三方一次性复制全部文档。',
          level: 'required',
        },
        {
          title: '权限证据优先',
          detail: '未拿到 V3 可见权限或供料证据时，模型需说明当前不可见或未供料。',
          level: 'required',
        },
        {
          title: '不提交真实凭证值',
          detail: '交接清单只写引用、交付方式或脱敏占位，真实凭证通过双方确认的安全通道交付。',
          level: 'required',
        },
        {
          title: '默认可回答通用问题',
          detail: 'V3 自我认知不限制模型能力；超出 V3 资料范围的问题可以按在线水准回答并标明依据边界。',
          level: 'required',
        },
      ],
    },
  };
}

function writeThirdPartyHandoffHtmlArtifact({ outputPath, generatedAt, head = '' }) {
  const manifest = thirdPartyHandoffHtmlArtifactManifest({ generatedAt, head });
  ensureParentDir(outputPath);
  fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const outputPath = path.resolve(repoRoot, args.out || DEFAULT_OUTPUT);
  writeThirdPartyHandoffHtmlArtifact({
    outputPath,
    generatedAt: args.generatedAt || new Date().toISOString(),
    head: args.head || '',
  });
  console.log(JSON.stringify({ outputPath }, null, 2));
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
const modulePath = fileURLToPath(import.meta.url);
if (invokedPath === modulePath) {
  await main();
}

export {
  thirdPartyHandoffHtmlArtifactManifest,
  writeThirdPartyHandoffHtmlArtifact,
};
