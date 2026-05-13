import ExternalIntegrationsPageClient from './ExternalIntegrationsPageClient';

export const metadata = {
  title: 'V3 外部集成观测',
  description: 'V3 第三方接口和外部集成观测面板。',
};

export default function ExternalIntegrationsPage() {
  return <ExternalIntegrationsPageClient />;
}
