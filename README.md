# AI Data Platform V3

Greenfield Rust rebuild workspace for AI Data Platform V3.

## Scope of this bootstrap

- Rust workspace and crate boundaries aligned with the V3 architecture docs
- Node workspace placeholders for `apps/web` and `apps/docs-site`
- Initial domain model, API contracts, scope model, workflow engine skeleton
- PostgreSQL initial schema migration for the system-of-record tables
- Placeholder app, worker, gateway, analytics, and report crates

## Architecture references

- `docs/architecture/v3-rust-architecture.md`
- `docs/architecture/v3-repository-and-module-layout.md`
- `docs/adr/README.md`

## Current status

This repository contains compile-oriented skeletons. Runtime integrations, storage adapters, and UI implementation are intentionally left as the next layer of work.
