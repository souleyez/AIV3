import { proxyPlatformApiRequest } from '../../../lib/platform-api';

export const dynamic = 'force-dynamic';

export async function GET(request, { params }) {
  return proxyPlatformApiRequest(request, params.path);
}

export async function POST(request, { params }) {
  return proxyPlatformApiRequest(request, params.path);
}

export async function PUT(request, { params }) {
  return proxyPlatformApiRequest(request, params.path);
}

export async function PATCH(request, { params }) {
  return proxyPlatformApiRequest(request, params.path);
}

export async function DELETE(request, { params }) {
  return proxyPlatformApiRequest(request, params.path);
}
