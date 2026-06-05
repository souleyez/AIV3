alter table documents
    add column if not exists content_sha256 text,
    add column if not exists content_size_bytes bigint,
    add column if not exists canonical_document_id uuid references documents (id) on delete set null,
    add column if not exists dedup_state text not null default 'unknown',
    add column if not exists deduped_at timestamptz;

create index if not exists documents_content_fingerprint_idx
    on documents (tenant_id, content_sha256, content_size_bytes)
    where content_sha256 is not null;

create index if not exists documents_canonical_document_idx
    on documents (tenant_id, canonical_document_id)
    where canonical_document_id is not null;

create table if not exists document_content_fingerprints (
    tenant_id uuid not null references tenants (id) on delete cascade,
    content_sha256 text not null,
    content_size_bytes bigint not null,
    canonical_document_id uuid not null references documents (id) on delete cascade,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, content_sha256)
);

create index if not exists document_content_fingerprints_canonical_idx
    on document_content_fingerprints (tenant_id, canonical_document_id);

create table if not exists document_enrichment_runs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    enrichment_kind text not null,
    parse_version text,
    input_fingerprint text not null,
    status text not null default 'pending',
    priority integer not null default 100,
    attempt_count integer not null default 0,
    max_attempts integer not null default 3,
    available_at timestamptz not null default now(),
    started_at timestamptz,
    finished_at timestamptz,
    error_message text,
    output_summary jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists document_enrichment_runs_idempotency_idx
    on document_enrichment_runs (tenant_id, document_id, enrichment_kind, input_fingerprint);

create index if not exists document_enrichment_runs_status_priority_idx
    on document_enrichment_runs (tenant_id, status, priority asc, available_at asc);

create index if not exists document_enrichment_runs_document_idx
    on document_enrichment_runs (tenant_id, document_id, created_at desc);
