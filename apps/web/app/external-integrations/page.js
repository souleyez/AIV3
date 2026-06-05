import ExternalIntegrationsPageClient from './ExternalIntegrationsPageClient';

export const metadata = {
  title: 'DataMax 外部集成观测',
  description: 'DataMax 第三方接口和外部集成观测面板。',
};

export default function ExternalIntegrationsPage() {
  return <ExternalIntegrationsPageClient />;
}
