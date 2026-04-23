import { proxyPlatformApiRequest } from '../../../lib/platform-api';

export const dynamic = 'force-dynamic';

async function readPathSegments(context) {
  const params = await context.params;
  return params?.path ?? [];
}

export async function GET(request, context) {
  return proxyPlatformApiRequest(request, await readPathSegments(context));
}

export async function POST(request, context) {
  return proxyPlatformApiRequest(request, await readPathSegments(context));
}

export async function PUT(request, context) {
  return proxyPlatformApiRequest(request, await readPathSegments(context));
}

export async function PATCH(request, context) {
  return proxyPlatformApiRequest(request, await readPathSegments(context));
}

export async function DELETE(request, context) {
  return proxyPlatformApiRequest(request, await readPathSegments(context));
}
