create table if not exists v3_client_config_packages (
    id uuid primary key default gen_random_uuid(),
    package_id text not null unique,
    tenant_id uuid not null references tenants(id) on delete cascade,
    owner_user_id uuid references users(id) on delete set null,
    tenant_ref text not null,
    user_ref text not null,
    client_id text not null,
    package_payload jsonb not null,
    expires_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_v3_client_config_packages_tenant_created
    on v3_client_config_packages (tenant_id, created_at desc);

create index if not exists idx_v3_client_config_packages_client
    on v3_client_config_packages (tenant_id, client_id, created_at desc);

create table if not exists v3_client_artifacts (
    id uuid primary key default gen_random_uuid(),
    artifact_id text not null unique,
    tenant_id uuid not null references tenants(id) on delete cascade,
    owner_user_id uuid references users(id) on delete set null,
    tenant_ref text not null,
    user_ref text not null,
    client_id text not null,
    task_id text not null,
    title text not null,
    artifact_type text not null,
    status text not null default 'received',
    manifest jsonb not null,
    dataset_ids text[] not null default array[]::text[],
    asset_library_ids text[] not null default array[]::text[],
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_v3_client_artifacts_tenant_created
    on v3_client_artifacts (tenant_id, created_at desc);

create index if not exists idx_v3_client_artifacts_task
    on v3_client_artifacts (tenant_id, task_id, created_at desc);

create index if not exists idx_v3_client_artifacts_dataset_ids
    on v3_client_artifacts using gin (dataset_ids);

create index if not exists idx_v3_client_artifacts_asset_library_ids
    on v3_client_artifacts using gin (asset_library_ids);

create table if not exists v3_client_artifact_files (
    id uuid primary key default gen_random_uuid(),
    artifact_id uuid not null references v3_client_artifacts(id) on delete cascade,
    file_index integer not null,
    filename text not null,
    content_type text not null,
    role text not null,
    size_bytes bigint not null,
    sha256 text not null,
    storage_kind text not null default 'database',
    object_locator text,
    bytes bytea,
    created_at timestamptz not null default now(),
    constraint v3_client_artifact_files_storage_kind_chk
        check (storage_kind in ('database', 'filesystem')),
    constraint v3_client_artifact_files_storage_payload_chk
        check (
            (storage_kind = 'database' and bytes is not null and object_locator is null)
            or (storage_kind = 'filesystem' and object_locator is not null and bytes is null)
        ),
    unique (artifact_id, file_index)
);

create index if not exists idx_v3_client_artifact_files_artifact
    on v3_client_artifact_files (artifact_id, file_index);

alter table if exists v3_client_artifact_files
    add column if not exists storage_kind text not null default 'database';

alter table if exists v3_client_artifact_files
    add column if not exists object_locator text;

alter table if exists v3_client_artifact_files
    alter column bytes drop not null;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'v3_client_artifact_files_storage_kind_chk'
    ) then
        alter table v3_client_artifact_files
            add constraint v3_client_artifact_files_storage_kind_chk
            check (storage_kind in ('database', 'filesystem'));
    end if;
    if not exists (
        select 1
        from pg_constraint
        where conname = 'v3_client_artifact_files_storage_payload_chk'
    ) then
        alter table v3_client_artifact_files
            add constraint v3_client_artifact_files_storage_payload_chk
            check (
                (storage_kind = 'database' and bytes is not null and object_locator is null)
                or (storage_kind = 'filesystem' and object_locator is not null and bytes is null)
            );
    end if;
end
$$;
