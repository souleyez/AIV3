import { NextResponse } from 'next/server';
import {
  EXTERNAL_OBSERVABILITY_COOKIE,
  consumeExternalObservabilityAccessKey,
  externalObservabilityCookieOptions,
  externalObservabilityCookieValue,
} from '../../lib/external-observability-access';

function firstForwardedHeaderValue(value) {
  return String(value || '').split(',')[0].trim();
}

function externalIntegrationsOrigin(request) {
  const forwardedHost = firstForwardedHeaderValue(request.headers.get('x-forwarded-host'));
  const forwardedProto = firstForwardedHeaderValue(request.headers.get('x-forwarded-proto'));
  const host = forwardedHost || request.headers.get('host');
  if (host) {
    const proto = forwardedProto || request.nextUrl.protocol.replace(':', '') || 'https';
    return `${proto}://${host}`;
  }
  return request.url;
}

function externalIntegrationsUrl(request, search = '') {
  const url = new URL('/external-integrations', externalIntegrationsOrigin(request));
  url.search = search;
  return url;
}

function observationPanelSearch(panel, { accessError = false } = {}) {
  const normalized = String(panel || '').trim();
  const params = new URLSearchParams();
  if (normalized === 'codex_executor') {
    params.set('codex_executor', '1');
  } else {
    params.set('conversation_tests', '1');
  }
  if (accessError) {
    params.set('access_error', '1');
  }
  return `?${params.toString()}`;
}

export async function GET(request) {
  return NextResponse.redirect(externalIntegrationsUrl(request));
}

export async function POST(request) {
  const form = await request.formData();
  const accessKey = form.get('access_key');
  const panel = form.get('observation_panel');
  if (!consumeExternalObservabilityAccessKey(accessKey)) {
    return NextResponse.redirect(
      externalIntegrationsUrl(request, observationPanelSearch(panel, { accessError: true })),
      303,
    );
  }
  const response = NextResponse.redirect(externalIntegrationsUrl(request, observationPanelSearch(panel)), 303);
  response.cookies.set(
    EXTERNAL_OBSERVABILITY_COOKIE,
    externalObservabilityCookieValue(),
    externalObservabilityCookieOptions({ secure: request.nextUrl.protocol === 'https:' }),
  );
  return response;
}
