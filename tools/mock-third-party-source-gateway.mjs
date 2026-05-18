import http from 'node:http';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export const MOCK_USERS = [
  {
    user_external_id: 'user-10001',
    display_name: '采购专员',
    status: 'active',
    department_external_ids: ['dept-procurement'],
    group_external_ids: ['group-procurement'],
    role_external_ids: ['role-requester'],
  },
  {
    user_external_id: 'user-20001',
    display_name: '财务观察员',
    status: 'active',
    department_external_ids: ['dept-finance'],
    group_external_ids: ['group-finance'],
    role_external_ids: ['role-viewer'],
  },
  {
    user_external_id: 'user-denied',
    display_name: '受限用户',
    status: 'active',
    department_external_ids: ['dept-procurement'],
    group_external_ids: ['group-procurement'],
    role_external_ids: ['role-requester'],
  },
];

export const MOCK_DOCUMENTS = [
  {
    document_external_id: 'doc-procurement-001',
    title: '采购审批制度',
    document_type: 'markdown',
    revision: 'rev-20260518-001',
    updated_at: '2026-05-18T09:20:00Z',
    deleted: false,
    content_hash: 'sha256:procurement-001',
    body: '# 采购审批制度\n\n超过 10 万元的采购需要部门负责人审批，并记录供应商风险。',
    acl: {
      allow: [{ subject_type: 'group', subject_external_id: 'group-procurement', level: 'read' }],
      deny: [],
    },
  },
  {
    document_external_id: 'doc-finance-001',
    title: '供应商付款制度',
    document_type: 'markdown',
    revision: 'rev-20260518-001',
    updated_at: '2026-05-18T09:22:00Z',
    deleted: false,
    content_hash: 'sha256:finance-001',
    body: '# 供应商付款制度\n\n付款申请必须匹配验收单、合同和发票。',
    acl: {
      allow: [{ subject_type: 'group', subject_external_id: 'group-finance', level: 'read' }],
      deny: [],
    },
  },
  {
    document_external_id: 'doc-public-001',
    title: '通用合规问答',
    document_type: 'markdown',
    revision: 'rev-20260518-001',
    updated_at: '2026-05-18T09:24:00Z',
    deleted: false,
    content_hash: 'sha256:public-001',
    body: '# 通用合规问答\n\n所有员工都应遵守供应商准入、审批留痕和资料保密要求。',
    acl: {
      allow: [{ subject_type: 'tenant', subject_external_id: 'tenant-ext-001', level: 'read' }],
      deny: [],
    },
  },
  {
    document_external_id: 'doc-restricted-001',
    title: '受限采购例外清单',
    document_type: 'markdown',
    revision: 'rev-20260518-001',
    updated_at: '2026-05-18T09:26:00Z',
    deleted: false,
    content_hash: 'sha256:restricted-001',
    body: '# 受限采购例外清单\n\n特殊供应商例外审批只能由采购负责人和风控负责人查看。',
    acl: {
      allow: [{ subject_type: 'group', subject_external_id: 'group-procurement', level: 'read' }],
      deny: [{ subject_type: 'user', subject_external_id: 'user-denied', reason: 'restricted' }],
    },
  },
  {
    document_external_id: 'doc-revoked-001',
    title: '已撤销旧制度',
    document_type: 'markdown',
    revision: 'rev-20260518-001',
    updated_at: '2026-05-18T09:28:00Z',
    deleted: false,
    content_hash: 'sha256:revoked-001',
    body: '# 已撤销旧制度\n\n该制度已经撤销，不应进入任何用户的回答证据。',
    acl: {
      allow: [],
      deny: [{ subject_type: 'tenant', subject_external_id: 'tenant-ext-001', reason: 'revoked' }],
    },
  },
];

const DEPARTMENTS = [
  { department_external_id: 'dept-procurement', name: '采购部' },
  { department_external_id: 'dept-finance', name: '财务部' },
];

const GROUPS = [
  { group_external_id: 'group-procurement', name: '采购制度读者' },
  { group_external_id: 'group-finance', name: '财务制度读者' },
];

const ROLES = [
  { role_external_id: 'role-requester', name: '申请人' },
  { role_external_id: 'role-viewer', name: '观察员' },
];

export function createMockThirdPartySourceGateway({ token = 'mock-source-token', baseUrl = '' } = {}) {
  const requestLog = [];
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || '/', baseUrl || `http://${req.headers.host || '127.0.0.1'}`);
    requestLog.push({ method: req.method, pathname: url.pathname, query: Object.fromEntries(url.searchParams.entries()) });

    if (req.method !== 'GET') {
      return writeJson(res, 405, { error: 'method_not_allowed' });
    }
    if (url.pathname === '/health') {
      return writeJson(res, 200, { ok: true, service: 'mock-third-party-source-gateway' });
    }
    if (!isAuthorized(req, token)) {
      return writeJson(res, 401, { error: 'source_auth_failed' });
    }

    if (url.pathname === '/users') return writeJson(res, 200, { items: MOCK_USERS, next_cursor: null });
    if (url.pathname === '/departments') return writeJson(res, 200, { items: DEPARTMENTS, next_cursor: null });
    if (url.pathname === '/groups') return writeJson(res, 200, { items: GROUPS, next_cursor: null });
    if (url.pathname === '/roles') return writeJson(res, 200, { items: ROLES, next_cursor: null });
    if (url.pathname === '/documents') return writeJson(res, 200, documentsResponse(url));
    if (url.pathname === '/parameter-card') return writeJson(res, 200, parameterCardResponse(baseUrl || `http://${req.headers.host}`));

    const membershipMatch = url.pathname.match(/^\/users\/([^/]+)\/memberships$/);
    if (membershipMatch) {
      const user = MOCK_USERS.find((candidate) => candidate.user_external_id === decodeURIComponent(membershipMatch[1]));
      if (!user) return writeJson(res, 404, { error: 'user_not_found' });
      return writeJson(res, 200, {
        user_external_id: user.user_external_id,
        department_external_ids: user.department_external_ids,
        group_external_ids: user.group_external_ids,
        role_external_ids: user.role_external_ids,
        status: user.status,
      });
    }

    const contentMatch = url.pathname.match(/^\/documents\/([^/]+)\/content$/);
    if (contentMatch) {
      const document = findDocument(contentMatch[1]);
      if (!document) return writeJson(res, 404, { error: 'document_not_found' });
      if (!revisionMatches(document, url.searchParams.get('revision'))) {
        return writeJson(res, 409, { error: 'revision_mismatch', expected_revision: document.revision });
      }
      return writeJson(res, 200, {
        document_external_id: document.document_external_id,
        revision: document.revision,
        content_type: 'text/markdown',
        title: document.title,
        body: document.body,
        content_hash: document.content_hash,
      });
    }

    const aclMatch = url.pathname.match(/^\/documents\/([^/]+)\/acl$/);
    if (aclMatch) {
      const document = findDocument(aclMatch[1]);
      if (!document) return writeJson(res, 404, { error: 'document_not_found' });
      if (!revisionMatches(document, url.searchParams.get('revision'))) {
        return writeJson(res, 409, { error: 'revision_mismatch', expected_revision: document.revision });
      }
      return writeJson(res, 200, {
        document_external_id: document.document_external_id,
        revision: document.revision,
        acl_hash: `sha256:acl-${document.document_external_id}`,
        captured_at: '2026-05-18T09:30:00Z',
        allow: document.acl.allow,
        deny: document.acl.deny,
      });
    }

    return writeJson(res, 404, { error: 'not_found' });
  });
  server.requestLog = requestLog;
  return server;
}

export async function startMockThirdPartySourceGateway({ host = '127.0.0.1', port = 43181, token = 'mock-source-token' } = {}) {
  const server = createMockThirdPartySourceGateway({ token });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, () => {
      server.off('error', reject);
      resolve();
    });
  });
  const address = server.address();
  const baseUrl = `http://${address.address}:${address.port}`;
  return { server, baseUrl, token };
}

function documentsResponse(url) {
  const limit = Math.max(1, Math.min(Number(url.searchParams.get('limit') || MOCK_DOCUMENTS.length), 100));
  const cursor = Number(url.searchParams.get('cursor') || 0);
  const items = MOCK_DOCUMENTS.slice(cursor, cursor + limit).map((document) => ({
    document_external_id: document.document_external_id,
    title: document.title,
    document_type: document.document_type,
    revision: document.revision,
    updated_at: document.updated_at,
    deleted: document.deleted,
    content_hash: document.content_hash,
    content_url: `/documents/${document.document_external_id}/content`,
    acl_url: `/documents/${document.document_external_id}/acl`,
  }));
  const nextCursor = cursor + limit < MOCK_DOCUMENTS.length ? String(cursor + limit) : null;
  return { items, next_cursor: nextCursor };
}

function parameterCardResponse(baseUrl) {
  return {
    manifest_version: 'v3.third_party_source_parameter_card.v1',
    customer: { name: 'Mock Third Party', technical_contact: 'mock@example.com' },
    environment: { name: 'local-mock', base_url: baseUrl, allow_http_loopback: true },
    source: {
      source_id: 'src-mock-third-party-docs',
      connector_kind: 'document',
      dataset_name: 'Mock 第三方制度库',
      sync_mode: 'pull',
      permission_mode: 'source_acl_snapshot',
    },
    auth: {
      request_auth_modes: ['bearer'],
      credential_delivery: 'out_of_band',
      authorization_header: 'Authorization: Bearer <third-party-source-token>',
      credential_rotation: 'customer-managed',
    },
    endpoints: {
      health: 'GET /health',
      users: 'GET /users',
      departments: 'GET /departments',
      groups: 'GET /groups',
      roles: 'GET /roles',
      memberships: 'GET /users/{user_external_id}/memberships',
      documents: 'GET /documents?cursor={cursor}&updated_after={iso_time}&limit={limit}',
      document_content: 'GET /documents/{document_external_id}/content?revision={revision}',
      document_acl: 'GET /documents/{document_external_id}/acl?revision={revision}',
    },
    fixtures: {
      users: MOCK_USERS,
      documents: MOCK_DOCUMENTS.map((document) => ({
        document_external_id: document.document_external_id,
        title: document.title,
        revision: document.revision,
        content_hash: document.content_hash,
      })),
      access_cases: [
        {
          case_id: 'procurement-user-can-read-procurement',
          user_external_id: 'user-10001',
          document_external_id: 'doc-procurement-001',
          expected: 'allow',
        },
        {
          case_id: 'finance-user-cannot-read-procurement',
          user_external_id: 'user-20001',
          document_external_id: 'doc-procurement-001',
          expected: 'deny',
        },
        {
          case_id: 'deny-user-cannot-read-restricted',
          user_external_id: 'user-denied',
          document_external_id: 'doc-restricted-001',
          expected: 'deny',
        },
      ],
    },
    operations: {
      retry_contact: 'mock-ops@example.com',
      escalation_contact: 'mock-support@example.com',
      maintenance_window: 'local testing',
    },
  };
}

function findDocument(rawId) {
  const documentId = decodeURIComponent(rawId);
  return MOCK_DOCUMENTS.find((document) => document.document_external_id === documentId);
}

function revisionMatches(document, revision) {
  return !revision || revision === document.revision;
}

function isAuthorized(req, token) {
  return req.headers.authorization === `Bearer ${token}`;
}

function writeJson(res, statusCode, body) {
  const payload = `${JSON.stringify(body, null, 2)}\n`;
  res.writeHead(statusCode, {
    'Content-Type': 'application/json; charset=utf-8',
    'Content-Length': Buffer.byteLength(payload),
  });
  res.end(payload);
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--host') args.host = argv[++index];
    else if (arg === '--port') args.port = Number(argv[++index]);
    else if (arg === '--token') args.token = argv[++index];
    else if (arg === '--help' || arg === '-h') args.help = true;
  }
  return args;
}

function printHelp() {
  console.log('Usage: node tools/mock-third-party-source-gateway.mjs [--host 127.0.0.1] [--port 43181] [--token mock-source-token]');
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    printHelp();
    return;
  }
  const { server, baseUrl, token } = await startMockThirdPartySourceGateway({
    host: args.host || '127.0.0.1',
    port: Number.isFinite(args.port) ? args.port : 43181,
    token: args.token || process.env.MOCK_THIRD_PARTY_SOURCE_TOKEN || 'mock-source-token',
  });
  console.log(`Mock third-party source gateway listening at ${baseUrl}`);
  console.log('Use Authorization: Bearer <token>');
  const shutdown = () => server.close(() => process.exit(0));
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}

const isCli = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  main().catch((error) => {
    console.error(error?.stack || error);
    process.exit(1);
  });
}
