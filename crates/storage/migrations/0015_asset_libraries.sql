create table if not exists enterprise_asset_libraries (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    external_id text,
    name text not null,
    domain text not null default 'general',
    description text,
    visibility text not null default 'private',
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, external_id)
);

create index if not exists enterprise_asset_libraries_tenant_domain_idx
    on enterprise_asset_libraries (tenant_id, domain, created_at desc);

create index if not exists enterprise_asset_libraries_metadata_gin_idx
    on enterprise_asset_libraries using gin (metadata);

create table if not exists asset_library_dataset_memberships (
    tenant_id uuid not null references tenants (id) on delete cascade,
    asset_library_id uuid not null references enterprise_asset_libraries (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    role text not null default 'member',
    priority integer not null default 100,
    created_at timestamptz not null default now(),
    primary key (tenant_id, asset_library_id, dataset_id)
);

create index if not exists asset_library_dataset_memberships_dataset_idx
    on asset_library_dataset_memberships (tenant_id, dataset_id, asset_library_id);

create index if not exists asset_library_dataset_memberships_priority_idx
    on asset_library_dataset_memberships (tenant_id, asset_library_id, priority asc, created_at asc);

create table if not exists asset_collections (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    asset_library_id uuid not null references enterprise_asset_libraries (id) on delete cascade,
    parent_collection_id uuid references asset_collections (id) on delete set null,
    external_id text,
    name text not null,
    collection_type text not null default 'general',
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, asset_library_id, external_id)
);

create index if not exists asset_collections_parent_idx
    on asset_collections (tenant_id, asset_library_id, parent_collection_id);

create index if not exists asset_collections_type_idx
    on asset_collections (tenant_id, asset_library_id, collection_type);

create table if not exists asset_items (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    asset_library_id uuid references enterprise_asset_libraries (id) on delete cascade,
    collection_id uuid references asset_collections (id) on delete set null,
    external_id text,
    title text not null,
    asset_kind text not null default 'document',
    source_kind text not null default 'document',
    source_id text,
    content_type text,
    object_key text,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, asset_library_id, external_id)
);

create index if not exists asset_items_library_kind_idx
    on asset_items (tenant_id, asset_library_id, asset_kind, created_at desc);

create index if not exists asset_items_collection_idx
    on asset_items (tenant_id, collection_id, created_at desc)
    where collection_id is not null;

create index if not exists asset_items_source_idx
    on asset_items (tenant_id, source_kind, source_id)
    where source_id is not null;

create unique index if not exists asset_items_source_unique_idx
    on asset_items (tenant_id, source_kind, source_id)
    where source_id is not null;

create index if not exists asset_items_metadata_gin_idx
    on asset_items using gin (metadata);

create table if not exists dataset_asset_memberships (
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    asset_id uuid not null references asset_items (id) on delete cascade,
    membership_kind text not null default 'curated',
    expires_at timestamptz,
    created_at timestamptz not null default now(),
    primary key (tenant_id, dataset_id, asset_id)
);

create index if not exists dataset_asset_memberships_asset_idx
    on dataset_asset_memberships (tenant_id, asset_id, dataset_id);

create index if not exists dataset_asset_memberships_expiry_idx
    on dataset_asset_memberships (tenant_id, expires_at)
    where expires_at is not null;

create table if not exists asset_profiles (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    asset_id uuid not null references asset_items (id) on delete cascade,
    profile_kind text not null,
    profile_version text not null default 'v1',
    attributes jsonb not null default '{}'::jsonb,
    embedding_status text not null default 'not_requested',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, asset_id, profile_kind, profile_version)
);

create index if not exists asset_profiles_kind_idx
    on asset_profiles (tenant_id, profile_kind, created_at desc);

create index if not exists asset_profiles_attributes_gin_idx
    on asset_profiles using gin (attributes);
