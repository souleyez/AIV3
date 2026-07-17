import test from 'node:test';
import assert from 'node:assert/strict';

import {
  databaseSourceProfileRequestBody,
  databaseSourceSyncRequestBody,
  databaseSourceOptionsFromIntegrations,
  normalizeDatabaseProfile,
  normalizeDatabaseSchema,
} from './database-source.js';

test('databaseSourceOptionsFromIntegrations keeps only configured database sources', () => {
  const options = databaseSourceOptionsFromIntegrations({
    integrations: [
      {
        integration_id: 'db-main',
        integration_kind: 'source',
        display_name: 'HY SQL',
        provider: 'mysql',
        config_summary: {
          database_source: {
            kind: 'mysql',
            database: 'hy_sql',
            connection_env: 'THIRD_PARTY_HY_SQL_DATABASE_URL',
            table_count: 1,
            tables: ['bi_traffic_area'],
            default_dataset_id: 'dataset-1',
          },
        },
      },
      {
        integration_id: 'chat-main',
        integration_kind: 'channel',
        display_name: 'Chat',
        config_summary: {},
      },
    ],
  });

  assert.equal(options.length, 1);
  assert.equal(options[0].id, 'db-main');
  assert.equal(options[0].source.database, 'hy_sql');
  assert.deepEqual(options[0].source.tables, ['bi_traffic_area']);
  assert.equal(options[0].source.defaultDatasetId, 'dataset-1');
});

test('normalizeDatabaseSchema accepts server schema payload shape', () => {
  const schema = normalizeDatabaseSchema({
    schema: {
      database: 'hy_sql',
      tables: [
        {
          name: 'bi_traffic_area',
          table_type: 'BASE TABLE',
          comment: '小时客流明细',
          approximate_row_count: 12,
          primary_key_columns: ['city'],
          indexes: [{ name: 'PRIMARY', columns: ['city'], is_unique: true }],
          columns: [
            {
              name: 'city',
              ordinal_position: 1,
              data_type: 'varchar',
              column_type: 'varchar(64)',
              is_nullable: false,
              comment: '城市',
            },
            { name: 'count', ordinal_position: 2, data_type: 'int', is_nullable: true },
          ],
        },
      ],
    },
  });

  assert.equal(schema.database, 'hy_sql');
  assert.equal(schema.tableCount, 1);
  assert.equal(schema.tables[0].columnCount, 2);
  assert.equal(schema.tables[0].columns[0].nullable, false);
  assert.equal(schema.tables[0].columns[0].primaryKey, true);
  assert.equal(schema.tables[0].columns[0].comment, '城市');
  assert.equal(schema.tables[0].comment, '小时客流明细');
});

test('normalizeDatabaseProfile compacts semantic counts for UI', () => {
  const profile = normalizeDatabaseProfile({
    profile: {
      database: 'hy_sql',
      metric_count: 3,
      dimension_count: 4,
      tables: [
        {
          name: 'bi_traffic_area',
          metrics: ['traffic_in', 'traffic_out'],
          dimensions: ['point_id', 'area_name', 'floor_name'],
          time_dimensions: ['hour'],
          suggested_mapping: { confidence: 86 },
          columns: [{
            name: 'traffic_in',
            data_type: 'bigint',
            column_type: 'bigint unsigned',
            semantic_role: 'metric',
            role_confidence: 92,
            nullable: false,
            sample_values: ['18', '27'],
            distinct_sample_count: 2,
            null_sample_count: 0,
          }],
        },
      ],
    },
  });

  assert.equal(profile.database, 'hy_sql');
  assert.equal(profile.metricCount, 3);
  assert.equal(profile.dimensionCount, 4);
  assert.equal(profile.tables[0].mappingConfidence, 86);
  assert.equal(profile.tables[0].metricCount, 2);
  assert.equal(profile.tables[0].columns[0].semanticRole, 'metric');
});

test('databaseSourceSyncRequestBody supports dataset id and external dataset binding', () => {
  assert.deepEqual(databaseSourceSyncRequestBody('full', 'dataset-1'), {
    sync_kind: 'full',
    connector_context: {},
    dataset_id: 'dataset-1',
  });

  assert.deepEqual(databaseSourceSyncRequestBody('incremental', {
    datasetExternalId: 'workspace-db-main',
    datasetTitle: '工作区数据库',
  }), {
    sync_kind: 'incremental',
    connector_context: {},
    dataset_external_id: 'workspace-db-main',
    dataset_title: '工作区数据库',
  });
});

test('databaseSourceProfileRequestBody keeps apply-profile payload bounded', () => {
  assert.deepEqual(databaseSourceProfileRequestBody(), {
    sample_limit: 100,
    database_source: {},
  });

  assert.deepEqual(databaseSourceProfileRequestBody({
    sampleLimit: 25.8,
    tables: [' bi_traffic_area ', '', 'bi_store'],
    dryRun: true,
  }), {
    sample_limit: 25,
    tables: ['bi_traffic_area', 'bi_store'],
    dry_run: true,
    database_source: {},
  });

  assert.deepEqual(databaseSourceProfileRequestBody({ sampleLimit: -1 }), {
    sample_limit: 100,
    database_source: {},
  });
});
