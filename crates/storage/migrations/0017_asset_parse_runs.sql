create table if not exists asset_parse_runs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    asset_id uuid not null references asset_items (id) on delete cascade,
    parser_name text not null,
    parser_version text not null default 'v1',
    status text not null default 'pending',
    started_at timestamptz,
    finished_at timestamptz,
    error_code text,
    error_message text,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, asset_id, parser_name, parser_version)
);

create index if not exists asset_parse_runs_asset_idx
    on asset_parse_runs (tenant_id, asset_id, created_at desc);

create index if not exists asset_parse_runs_status_idx
    on asset_parse_runs (tenant_id, status, updated_at desc);

create index if not exists asset_parse_runs_metadata_gin_idx
    on asset_parse_runs using gin (metadata);
