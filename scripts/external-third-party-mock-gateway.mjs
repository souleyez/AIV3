#!/usr/bin/env node
import crypto from 'node:crypto';
import http from 'node:http';

const host = process.env.EXTERNAL_THIRD_PARTY_MOCK_HOST || '127.0.0.1';
const port = Number(process.env.EXTERNAL_THIRD_PARTY_MOCK_PORT || 43180);
const bearerToken = process.env.EXTERNAL_THIRD_PARTY_MOCK_BEARER_TOKEN || 'dispatch-token';
const signingSecret = process.env.EXTERNAL_THIRD_PARTY_MOCK_SIGNING_SECRET || 'dispatch-secret';
const requestId = process.env.EXTERNAL_THIRD_PARTY_MOCK_REQUEST_ID || 'gateway-req-001';
const requests = [];

function sha256Hex(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function hmacHex(secret, payload) {
  return crypto.createHmac('sha256', secret).update(payload).digest('hex');
}

function sendJson(response, status, payload) {
  const body = JSON.stringify(payload);
  response.writeHead(status, {
    'content-type': 'application/json',
    'content-length': Buffer.byteLength(body),
    connection: 'close',
  });
  response.end(body);
}

function requestBody(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on('data', (chunk) => chunks.push(chunk));
    request.on('error', reject);
    request.on('end', () => resolve(Buffer.concat(chunks)));
  });
}

async function handleDispatch(request, response) {
  const body = await requestBody(request);
  const bodyText = body.toString('utf8');
  const bodyHash = sha256Hex(body);
  const timestamp = String(request.headers['x-v3-timestamp'] || '');
  const nonce = String(request.headers['x-v3-nonce'] || '');
  const contentHash = String(request.headers['x-v3-content-sha256'] || '');
  const signature = String(request.headers['x-v3-signature'] || '');
  const authorization = String(request.headers.authorization || '');
  const pathWithQuery = request.url || '/';
  const canonical = `${request.method}\n${pathWithQuery}\n${timestamp}\n${nonce}\n${bodyHash}`;
  const expectedSignature = `sha256=${hmacHex(signingSecret, canonical)}`;
  const bearerValid = authorization === `Bearer ${bearerToken}`;
  const bodyHashValid = contentHash === bodyHash;
  const signatureValid = signature === expectedSignature;
  const methodValid = request.method === 'POST';
  let payload = {};

  try {
    payload = JSON.parse(bodyText || '{}');
  } catch {
    payload = {};
  }

  const record = {
    method: request.method,
    path: pathWithQuery,
    action_id: payload.action_id || null,
    action_type: payload.action_type || null,
    raw_arguments_included: payload.raw_arguments_included === true,
    requester_sender_present: Boolean(payload.requester_summary?.sender_external_id),
    bearer_valid: bearerValid,
    signature_valid: signatureValid,
    body_hash_valid: bodyHashValid,
    contains_forbidden_text:
      bodyText.includes('raw prompt secret') ||
      bodyText.includes('callback-token-should-not-leak') ||
      bodyText.includes('third-party-secret'),
  };
  requests.push(record);

  if (!methodValid || !bearerValid || !bodyHashValid || !signatureValid || record.contains_forbidden_text) {
    sendJson(response, 401, {
      status: 'rejected',
      bearer_valid: bearerValid,
      signature_valid: signatureValid,
      body_hash_valid: bodyHashValid,
    });
    return;
  }

  sendJson(response, 200, {
    external_request_id: requestId,
    status: 'accepted',
    message: 'accepted by external mock gateway',
    token: 'third-party-secret',
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === 'GET' && request.url === '/__mock/health') {
      sendJson(response, 200, { status: 'ok', request_count: requests.length });
      return;
    }
    if (request.method === 'GET' && request.url === '/__mock/requests') {
      sendJson(response, 200, { requests });
      return;
    }
    if (request.url?.startsWith('/third-party/actions')) {
      await handleDispatch(request, response);
      return;
    }
    sendJson(response, 404, { status: 'not_found' });
  } catch (error) {
    sendJson(response, 500, { status: 'error', message: String(error?.message || error) });
  }
});

server.listen(port, host, () => {
  console.log(`external-third-party-mock-gateway listening on http://${host}:${port}`);
});

process.on('SIGTERM', () => server.close(() => process.exit(0)));
process.on('SIGINT', () => server.close(() => process.exit(0)));
