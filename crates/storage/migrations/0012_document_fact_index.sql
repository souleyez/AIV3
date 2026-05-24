create table if not exists document_facts (
    id uuid primary key,
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    fact_type text not null,
    name text not null,
    normalized_name text not null,
    value_text text,
    value_number double precision,
    value_date date,
    attributes jsonb not null default '{}'::jsonb,
    confidence double precision not null default 1.0,
    source_kind text not null,
    source_locator text,
    source_chunk_id uuid references document_chunks (id) on delete set null,
    parse_version text,
    created_at timestamptz not null default now()
);

create index if not exists document_facts_dataset_type_name_idx
    on document_facts (tenant_id, dataset_id, fact_type, normalized_name);

create index if not exists document_facts_document_type_idx
    on document_facts (tenant_id, dataset_id, document_id, fact_type);

create index if not exists document_facts_attributes_gin_idx
    on document_facts using gin (attributes);

create table if not exists document_fact_sources (
    id uuid primary key,
    fact_id uuid not null references document_facts (id) on delete cascade,
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    source_kind text not null,
    source_locator text,
    source_chunk_id uuid references document_chunks (id) on delete set null,
    attributes jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create index if not exists document_fact_sources_fact_idx
    on document_fact_sources (tenant_id, fact_id);

create index if not exists document_fact_sources_document_idx
    on document_fact_sources (tenant_id, dataset_id, document_id);

create table if not exists dataset_fact_snapshots (
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    snapshot_kind text not null,
    snapshot_key text not null,
    snapshot_manifest jsonb not null default '{}'::jsonb,
    source_fact_count bigint not null default 0,
    source_document_count bigint not null default 0,
    created_at timestamptz not null default now(),
    primary key (tenant_id, dataset_id, snapshot_kind, snapshot_key)
);

create index if not exists dataset_fact_snapshots_manifest_gin_idx
    on dataset_fact_snapshots using gin (snapshot_manifest);
