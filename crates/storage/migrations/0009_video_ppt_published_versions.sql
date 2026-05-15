create table if not exists published_video_ppt_packages (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    assistant_run_id uuid not null references assistant_runs (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    package_key text not null,
    current_version_id uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, assistant_run_id, document_id, package_key)
);

create table if not exists published_video_ppt_versions (
    id uuid primary key default gen_random_uuid(),
    package_id uuid not null references published_video_ppt_packages (id) on delete cascade,
    version_no integer not null,
    version_fingerprint text not null,
    lifecycle_state text not null,
    artifact_manifest jsonb not null,
    created_at timestamptz not null default now(),
    unique (package_id, version_no),
    unique (package_id, version_fingerprint)
);

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'published_video_ppt_packages_current_version_fk'
    ) then
        alter table published_video_ppt_packages
            add constraint published_video_ppt_packages_current_version_fk
            foreign key (current_version_id)
            references published_video_ppt_versions (id)
            on delete set null;
    end if;
end $$;

create index if not exists published_video_ppt_packages_document_updated_idx
    on published_video_ppt_packages (tenant_id, document_id, updated_at desc);

create index if not exists published_video_ppt_packages_run_updated_idx
    on published_video_ppt_packages (tenant_id, assistant_run_id, updated_at desc);

create index if not exists published_video_ppt_versions_package_created_idx
    on published_video_ppt_versions (package_id, version_no desc, created_at desc);
