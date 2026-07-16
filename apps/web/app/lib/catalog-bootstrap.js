const SECONDARY_CATALOG_ENDPOINTS = [
  '/api/v3/report-plans',
  '/api/v3/published-reports',
  '/api/v3/documents',
];

export async function loadCatalogInStages(fetchJson, onDatasetsReady) {
  const datasetCatalogPromise = fetchJson('/api/v3/datasets');
  const secondaryCatalogPromise = Promise.all(
    SECONDARY_CATALOG_ENDPOINTS.map((endpoint) => fetchJson(endpoint)),
  ).then(
    ([planItems, reportItems, documentItems]) => ({
      ok: true,
      planItems,
      reportItems,
      documentItems,
    }),
    (error) => ({ ok: false, error }),
  );
  const datasetItems = await datasetCatalogPromise;
  onDatasetsReady(datasetItems);

  const secondaryCatalog = await secondaryCatalogPromise;
  if (!secondaryCatalog.ok) {
    throw secondaryCatalog.error;
  }

  return {
    planItems: secondaryCatalog.planItems,
    reportItems: secondaryCatalog.reportItems,
    documentItems: secondaryCatalog.documentItems,
  };
}
