# ADR-0003: Upgrade the primary state store to PostgreSQL 18.4

## Status

Implemented on 2026-07-10

## Context

DataMax V3 uses PostgreSQL as its primary transactional state store. Before the
cutover, the production host ran PostgreSQL 17.9, while the local compose file
still referenced PostgreSQL 16 and the README referenced 17.9. PostgreSQL 18.4
was selected as the supported stable target release.

The production database is currently small enough for a reviewed maintenance
window: the database is about 279 MB and the PostgreSQL 17 data directory is
about 407 MB. The only non-default extension is `pgcrypto`. The user confirmed
that the system is not currently in use and does not require a blue-green
upgrade.

## Decision

Production was upgraded from PostgreSQL 17.9 to PostgreSQL 18.4 in a cold
maintenance window after application release commit
`f9463861ced34022fb98ed959bccda015c8b7800` was pushed and its GitHub Actions run
completed successfully.

The implemented upgrade:

- stopped all AIV3 services before the final backup;
- created logical dumps for `ai_data_platform_v3`, `aidp_client`, and
  `home_platform`, plus a filesystem archive of the PostgreSQL 17 data
  directory;
- installed PostgreSQL 18.4 and `postgresql18-contrib` from signed PGDG RPMs;
- initialized a clean PostgreSQL 18 cluster with data checksums enabled;
- restored roles and all three user databases with versioned PostgreSQL 18
  tools, then proved exact per-table row-count parity;
- updated 15 AIV3 systemd dependencies and the Home database backup unit from
  `postgresql-17.service` to `postgresql-18.service`;
- kept the PostgreSQL 17 data directory and verified backups until a separately
  approved cleanup.

No application schema change, feature enablement, backfill, or production data
cleanup is allowed in the database upgrade window.

## Consequences

### Positive

- Moves the primary state store to the latest supported stable major release.
- Enables data checksums on the new cluster.
- Removes local development version drift.
- Keeps a filesystem-level rollback source until cleanup is approved.

### Negative

- Requires a maintenance window and a full service stop.
- Fifteen AIV3 service units and one database backup unit had to be updated
  together.
- Logical restore must preserve roles, ownership, extensions, grants, indexes,
  and full-text-search behavior.

### Neutral

- SQLx 0.9 remains the application database driver.
- Connection limits remain unchanged until production measurements justify a
  separate tuning decision.

## Alternatives Considered

### Blue-green logical replication

Rejected for this upgrade because the system is not in active use and the user
explicitly does not require blue-green operation.

### `pg_upgrade --link` or `--swap`

Rejected because both modes weaken the simplicity of the rollback boundary.

### Stay on PostgreSQL 17

Rejected as the final state. Updating to 17.10 remains a valid fallback if the
PostgreSQL 18 restore rehearsal fails.

## References

- https://www.postgresql.org/support/versioning/
- https://www.postgresql.org/docs/18/upgrading.html
- https://www.postgresql.org/docs/18/release-18.html
- https://github.com/docker-library/docs/blob/master/postgres/README.md
