create table if not exists asset_retrieval_evidences (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    asset_id uuid not null references asset_items (id) on delete cascade,
    asset_profile_id uuid not null references asset_profiles (id) on delete cascade,
    profile_kind text not null,
    profile_version text not null,
    materialized_text text not null,
    safe_metadata jsonb not null default '{}'::jsonb,
    content_hash text not null,
    search_terms text[] not null default '{}'::text[],
    search_tsv tsvector generated always as (
        to_tsvector('simple'::regconfig, materialized_text)
    ) stored,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (length(btrim(materialized_text)) > 0),
    check (jsonb_typeof(safe_metadata) = 'object'),
    check (content_hash ~ '^[0-9a-f]{64}$'),
    unique (tenant_id, dataset_id, asset_id, profile_kind, profile_version, content_hash)
);

create index if not exists asset_retrieval_evidences_scope_profile_idx
    on asset_retrieval_evidences (
        tenant_id,
        dataset_id,
        profile_kind,
        created_at desc
    );

create index if not exists asset_retrieval_evidences_asset_profile_idx
    on asset_retrieval_evidences (
        tenant_id,
        asset_id,
        asset_profile_id,
        created_at desc
    );

create index if not exists asset_retrieval_evidences_search_terms_gin_idx
    on asset_retrieval_evidences using gin (search_terms);

create index if not exists asset_retrieval_evidences_search_tsv_gin_idx
    on asset_retrieval_evidences using gin (search_tsv);

create index if not exists asset_retrieval_evidences_content_hash_idx
    on asset_retrieval_evidences (
        tenant_id,
        asset_id,
        content_hash,
        created_at desc
    );
