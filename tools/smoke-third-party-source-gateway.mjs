import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export async function runThirdPartySourceGatewaySmoke({
  baseUrl,
  token,
  expectedMinUsers = 3,
  expectedMinDocuments = 5,
  fetchImpl = globalThis.fetch,
} = {}) {
  if (!baseUrl) throw new Error('baseUrl is required');
  if (!token) throw new Error('token is required');
  if (typeof fetchImpl !== 'function') throw new Error('fetch implementation is required');

  const checks = [];
  const errors = [];
  const check = (key, label, passed, detail = {}) => {
    checks.push({ key, label, passed: Boolean(passed), ...detail });
    if (!passed) errors.push({ code: key, message: label, detail });
  };

  const health = await getJson(fetchImpl, baseUrl, '/health', null);
  check('health_ok', 'Source gateway health endpoint is reachable', health.status === 200 && health.body?.ok === true, { status: health.status });

  const users = await getJson(fetchImpl, baseUrl, '/users', token);
  const userItems = Array.isArray(users.body?.items) ? users.body.items : [];
  check('users_minimum', 'Source gateway returns enough users', users.status === 200 && userItems.length >= expectedMinUsers, {
    status: users.status,
    count: userItems.length,
  });

  const documents = await getJson(fetchImpl, baseUrl, '/documents?limit=100', token);
  const documentItems = Array.isArray(documents.body?.items) ? documents.body.items : [];
  check('documents_minimum', 'Source gateway returns enough documents', documents.status === 200 && documentItems.length >= expectedMinDocuments, {
    status: documents.status,
    count: documentItems.length,
  });

  const firstDocument = documentItems[0];
  let content = null;
  let acl = null;
  if (firstDocument?.document_external_id) {
    const encodedId = encodeURIComponent(firstDocument.document_external_id);
    const revision = encodeURIComponent(firstDocument.revision || '');
    content = await getJson(fetchImpl, baseUrl, `/documents/${encodedId}/content?revision=${revision}`, token);
    acl = await getJson(fetchImpl, baseUrl, `/documents/${encodedId}/acl?revision=${revision}`, token);
  }
  check('content_fetch', 'Document content endpoint returns body for listed document', content?.status === 200 && typeof content.body?.body === 'string' && content.body.body.trim().length > 0, {
    status: content?.status ?? null,
    document_external_id: firstDocument?.document_external_id || null,
  });
  check('acl_fetch', 'Document ACL endpoint returns allow/deny arrays for listed document', acl?.status === 200 && Array.isArray(acl.body?.allow) && Array.isArray(acl.body?.deny), {
    status: acl?.status ?? null,
    document_external_id: firstDocument?.document_external_id || null,
  });

  const unauthorized = await getJson(fetchImpl, baseUrl, '/documents?limit=1', 'wrong-token');
  check('auth_rejects_wrong_token', 'Source gateway rejects wrong Bearer token', unauthorized.status === 401, { status: unauthorized.status });

  const ready = checks.every((item) => item.passed);
  return {
    report_type: 'third_party_source_gateway_smoke',
    generated_at: new Date().toISOString(),
    base_url: redactBaseUrl(baseUrl),
    ready,
    checks,
    errors,
    summary: {
      user_count: userItems.length,
      document_count: documentItems.length,
      sampled_document_external_id: firstDocument?.document_external_id || null,
      sampled_revision: firstDocument?.revision || null,
    },
  };
}

async function getJson(fetchImpl, baseUrl, pathname, token) {
  const headers = {};
  if (token) headers.Authorization = `Bearer ${token}`;
  const response = await fetchImpl(new URL(pathname, ensureTrailingSlash(baseUrl)), { headers });
  let body = null;
  try {
    body = await response.json();
  } catch {
    body = null;
  }
  return { status: response.status, body };
}

function ensureTrailingSlash(value) {
  return value.endsWith('/') ? value : `${value}/`;
}

function redactBaseUrl(rawUrl) {
  try {
    const url = new URL(rawUrl);
    return `${url.protocol}//${url.hostname}${url.port ? `:${url.port}` : ''}`;
  } catch {
    return '[invalid-url]';
  }
}

export function renderSourceSmokeMarkdown(report) {
  const lines = [
    '# Third-party Source Gateway Smoke',
    '',
    `Ready: \`${report.ready ? 'yes' : 'no'}\``,
    `Base URL: \`${report.base_url}\``,
    `Users: \`${report.summary.user_count}\``,
    `Documents: \`${report.summary.document_count}\``,
    '',
    '## Checks',
    '',
    '| Check | Passed |',
    '| --- | --- |',
    ...report.checks.map((check) => `| ${check.label} | ${check.passed ? 'yes' : 'no'} |`),
  ];
  if (report.errors.length > 0) {
    lines.push('', '## Errors', '');
    for (const item of report.errors) {
      lines.push(`- \`${item.code}\` ${item.message}`);
    }
  }
  return `${lines.join('\n')}\n`;
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--base-url') args.baseUrl = argv[++index];
    else if (arg === '--token') args.token = argv[++index];
    else if (arg === '--token-env') args.tokenEnv = argv[++index];
    else if (arg === '--out') args.out = argv[++index];
    else if (arg === '--markdown') args.markdown = argv[++index];
    else if (arg === '--help' || arg === '-h') args.help = true;
  }
  return args;
}

function printHelp() {
  console.log('Usage: node tools/smoke-third-party-source-gateway.mjs --base-url <url> (--token <token> | --token-env ENV_NAME) [--out report.json] [--markdown report.md]');
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const token = args.token || (args.tokenEnv ? process.env[args.tokenEnv] : null) || process.env.THIRD_PARTY_SOURCE_TOKEN;
  if (args.help || !args.baseUrl || !token) {
    printHelp();
    process.exit(args.help ? 0 : 2);
  }
  const report = await runThirdPartySourceGatewaySmoke({ baseUrl: args.baseUrl, token });
  const json = `${JSON.stringify(report, null, 2)}\n`;
  if (args.out) fs.writeFileSync(path.resolve(args.out), json);
  if (args.markdown) fs.writeFileSync(path.resolve(args.markdown), renderSourceSmokeMarkdown(report));
  if (!args.out) process.stdout.write(json);
  process.exit(report.ready ? 0 : 1);
}

const isCli = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  main().catch((error) => {
    console.error(error?.stack || error);
    process.exit(1);
  });
}
