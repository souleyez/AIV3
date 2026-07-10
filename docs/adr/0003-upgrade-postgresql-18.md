# ADR-0003: Upgrade the primary state store to PostgreSQL 18.4

## Status

Accepted

## Context

DataMax V3 uses PostgreSQL as its primary transactional state store. The
production host currently runs PostgreSQL 17.9, while the local compose file
still referenced PostgreSQL 16 and the README referenced 17.9. PostgreSQL 18.4
is the current supported stable release.

The production database is currently small enough for a reviewed maintenance
window: the database is about 279 MB and the PostgreSQL 17 data directory is
about 407 MB. The only non-default extension is `pgcrypto`. The user confirmed
that the system is not currently in use and does not require a blue-green
upgrade.

## Decision

Upgrade production from PostgreSQL 17.9 to PostgreSQL 18.4 in a cold maintenance
window after the current application release is committed and pushed.

The upgrade will:

- stop all AIV3 services before the final backup;
- create both logical and filesystem backups before changing packages;
- install PostgreSQL 18.4 and `postgresql18-contrib` from the configured PGDG
  repository;
- initialize a clean PostgreSQL 18 cluster with data checksums enabled;
- restore roles and the `ai_data_platform_v3` database with versioned
  PostgreSQL 18 tools;
- update systemd dependencies from `postgresql-17.service` to
  `postgresql-18.service`;
- keep the PostgreSQL 17 data directory and backups until a separately approved
  cleanup.

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
- Fifteen AIV3 service units currently reference `postgresql-17.service` and
  must be updated together.
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
