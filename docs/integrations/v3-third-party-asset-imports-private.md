# DataMax V3 Third-Party Asset Imports — PRIVATE PILOT

This operator-only document is not part of the public integration contract. Do not copy its route, request fields, or examples into public schemas, generated guides, Web assets, or customer documentation before an independent contract review.

## Boundary

- Route: `POST /v1/external/channels/{connection_id}/asset-imports`
- Authentication: the selected external channel must have an active inbound bearer token.
- Release controls: `EXTERNAL_ASSET_IMPORT_ENABLED=false` and an empty `EXTERNAL_ASSET_IMPORT_CONNECTION_ALLOWLIST` are the safe defaults.
- Pilot opening: set the feature flag true and allowlist exactly one reviewed connection.
- Tenant: the request has no tenant field. The API uses the server-side tenant of the authenticated connection.
- Source: `source_external_id` must be the connection default or one of its configured allowed sources.
- Dataset/library: every requested external dataset must belong to the connection system user and already be attached to the selected private asset library. The library metadata must bind `external_asset_import.connection_id` and `external_asset_import.source_external_id` to the reviewed pilot scope.
- Visibility: public third-party schemas, URLs, and required fields remain unchanged.

## Private request

```json
{
  "request_id": "operator-generated-idempotency-key",
  "source_external_id": "reviewed-source-id",
  "asset_library_external_id": "reviewed-private-library-id",
  "asset_collection_external_id": "optional-private-collection-id",
  "dataset_external_ids": ["reviewed-dataset-id"],
  "assets": [
    {
      "external_id": "customer-stable-asset-id",
      "title": "Pilot image",
      "object_key": "operator-reviewed-object-reference",
      "content_type": "image/png",
      "profile_payload": {},
      "metadata": {}
    }
  ],
  "packages": [],
  "metadata": {}
}
```

Single assets, batches, and reviewed ZIP object references use the existing DataMax import validation. A replacement uses a new `request_id` with the same asset `external_id`; a repeat of the same request uses the same `request_id` and identical payload. Reusing a request ID with different content is rejected.

The private response contains only `request_id`, an opaque `task_ref`, aggregate `parse_status`, and safe asset summaries. It does not return tenant IDs, internal asset/dataset/library IDs, object references, source URLs, provider payloads, bearer material, or database locators.

## Pilot sequence

1. Prove the public contract guard, ordinary third-party chat/static-page smoke, and feature-off denial.
2. Record a backup and the exact application SHA. Enable the flag for one connection and restart only platform-api.
3. Run single, batch, ZIP, replacement, same-request replay, and a different-connection denial.
4. Record only safe aggregates and opaque refs. Do not put credentials, object references, external IDs, or customer content in shared receipts.
5. Restore `EXTERNAL_ASSET_IMPORT_ENABLED=false`, clear the allowlist, restart platform-api, revoke any one-time session/token, rerun regressions, and create a no-delete cleanup manifest.

## Stop conditions

Stop immediately for public contract drift, cross-connection visibility, a non-private library, source/dataset/library scope mismatch, request-id conflict that writes data, internal-field leakage, or a failed feature-off rollback. Test and pilot objects are retained until a separately approved cleanup.
