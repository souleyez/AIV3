import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import {
  findSensitiveConnectionLogViolations,
  scanSensitiveConnectionLogFields,
} from './check-sensitive-log-fields.mjs';

test('findSensitiveConnectionLogViolations rejects raw connection variables in logs and errors', () => {
  const source = String.raw`
    tracing::info!(%database_url, "platform ready");
    tracing::warn!(%nats_url, error = ?error, "event bus disabled");
    tracing::debug!(?database_url, "database diagnostics");
    tracing::info!(nats_url = nats_url, "raw assigned field");
    anyhow!("failed to connect fixture at {database_url}: {error}");
    tracing::warn!(url = ?database_url, "leak");
    tracing::info!(%connection_url, "leak");
    anyhow!("failed: {}", nats_url);
    tracing::info!(url = %config.database_url, "leak");
  `;

  const violations = findSensitiveConnectionLogViolations(source, 'fixture.rs');

  assert.deepEqual(
    violations.map((violation) => violation.rule),
    [
      'raw_database_url_field',
      'raw_nats_url_field',
      'raw_connection_url_debug_field',
      'raw_connection_url_named_field',
      'raw_connection_url_interpolation',
      'raw_connection_url_debug_field',
      'raw_connection_url_alias_or_member_field',
      'raw_connection_url_positional_format',
      'raw_connection_url_alias_or_member_field',
    ],
  );
  assert.deepEqual(
    violations.map((violation) => violation.line),
    [2, 3, 4, 5, 6, 7, 8, 9, 10],
  );
});

test('findSensitiveConnectionLogViolations rejects equivalent member, multiline, and named fields', () => {
  const source = String.raw`
    tracing::info!(url = %config.connection_url, "leak");
    tracing::info!(url = ?request.raw_connection_url, "leak");
    anyhow!("failed at {connection_url}");
    anyhow!(
      "failed: {}",
      nats_url
    );
    tracing::info!(database_url = %config.db_uri, "leak");
  `;

  const violations = findSensitiveConnectionLogViolations(source, 'equivalent.rs');

  assert.deepEqual(
    violations.map((violation) => violation.rule),
    [
      'raw_connection_url_alias_or_member_field',
      'raw_connection_url_alias_or_member_field',
      'raw_connection_url_interpolation',
      'raw_connection_url_positional_format',
      'raw_connection_url_named_field',
    ],
  );
  assert.deepEqual(
    violations.map((violation) => violation.line),
    [2, 3, 4, 5, 9],
  );
});

test('findSensitiveConnectionLogViolations handles Rust macro delimiters, characters, and spans', () => {
  const source = String.raw`
    tracing::info! { database_url = %config.db_uri, "leak" }
    anyhow! { "failed: {}", nats_url }
    tracing::info!(marker = ?')', database_url = %config.db_uri, "leak");
    tracing::info_span!("db", database_url = %config.db_uri);
  `;

  const violations = findSensitiveConnectionLogViolations(source, 'macro-forms.rs');

  assert.deepEqual(
    violations.map((violation) => violation.rule),
    [
      'raw_connection_url_named_field',
      'raw_connection_url_positional_format',
      'raw_connection_url_named_field',
      'raw_connection_url_named_field',
    ],
  );
  assert.deepEqual(
    violations.map((violation) => violation.line),
    [2, 3, 4, 5],
  );
});

test('findSensitiveConnectionLogViolations accepts a redacted endpoint helper', () => {
  const source = String.raw`
    let database_endpoint = observability::redact_connection_endpoint(&database_url);
    tracing::info!(%database_endpoint, "platform ready");
    let nats_endpoint = observability::redact_connection_endpoint(&nats_url);
    tracing::warn!(%nats_endpoint, error = ?error, "event bus disabled");
  `;

  assert.deepEqual(findSensitiveConnectionLogViolations(source, 'safe.rs'), []);
});

test('scanSensitiveConnectionLogFields scans only Rust source files recursively', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aiv3-sensitive-log-fields-'));
  try {
    fs.mkdirSync(path.join(root, 'nested'));
    fs.writeFileSync(
      path.join(root, 'nested', 'unsafe.rs'),
      'tracing::info!(%database_url, "unsafe");\n',
    );
    fs.writeFileSync(
      path.join(root, 'nested', 'ignored.txt'),
      'tracing::info!(%nats_url, "not rust");\n',
    );

    const violations = scanSensitiveConnectionLogFields(root);

    assert.equal(violations.length, 1);
    assert.equal(violations[0].file, 'nested/unsafe.rs');
    assert.equal(violations[0].rule, 'raw_database_url_field');
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
