create table if not exists dataset_semantic_snapshots (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    schema_version text not null,
    generation_version text not null,
    source_fingerprint text not null,
    status text not null,
    manifest jsonb not null default '{}'::jsonb,
    source_document_count bigint not null default 0,
    source_asset_count bigint not null default 0,
    source_record_count bigint not null default 0,
    node_count integer not null default 0,
    edge_count integer not null default 0,
    failure_code text,
    generated_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (status in ('building', 'ready', 'failed', 'superseded')),
    check (jsonb_typeof(manifest) = 'object'),
    check (source_document_count >= 0),
    check (source_asset_count >= 0),
    check (source_record_count >= 0),
    check (node_count >= 0),
    check (edge_count >= 0),
    unique (tenant_id, dataset_id, generation_version, source_fingerprint)
);

create index if not exists dataset_semantic_snapshots_latest_ready_idx
    on dataset_semantic_snapshots (tenant_id, dataset_id, generated_at desc)
    where status = 'ready';

create table if not exists semantic_dictionary_entries (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    source_kind text not null,
    source_system_key text not null default '*',
    source_object_key text not null default '*',
    raw_field_key text not null,
    display_name text not null,
    description text,
    semantic_role text not null default 'unknown',
    value_type text not null default 'unknown',
    status text not null default 'suggested',
    confidence double precision not null default 0,
    created_by_user_id uuid references users (id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (status in ('suggested', 'confirmed', 'rejected')),
    check (confidence >= 0 and confidence <= 1),
    check (length(btrim(source_kind)) > 0),
    check (length(btrim(source_system_key)) > 0),
    check (length(btrim(source_object_key)) > 0),
    check (length(btrim(raw_field_key)) > 0),
    check (length(btrim(display_name)) > 0),
    unique (
        tenant_id,
        source_kind,
        source_system_key,
        source_object_key,
        raw_field_key
    )
);

create index if not exists semantic_dictionary_entries_resolve_idx
    on semantic_dictionary_entries (
        tenant_id,
        source_kind,
        raw_field_key,
        source_system_key,
        source_object_key,
        status
    );
