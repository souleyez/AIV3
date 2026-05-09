create table if not exists html_artifacts (
    id text not null,
    tenant_id uuid not null references tenants (id) on delete cascade,
    owner_user_id uuid references users (id) on delete set null,
    assistant_run_id uuid references assistant_runs (id) on delete cascade,
    local_thread_id text,
    source_type text not null,
    template_id text not null,
    interaction_mode text not null,
    manifest jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, id)
);

create index if not exists html_artifacts_run_created_idx
    on html_artifacts (tenant_id, assistant_run_id, created_at desc)
    where assistant_run_id is not null;

create index if not exists html_artifacts_thread_created_idx
    on html_artifacts (tenant_id, local_thread_id, created_at desc)
    where local_thread_id is not null;

create index if not exists html_artifacts_owner_thread_idx
    on html_artifacts (tenant_id, owner_user_id, local_thread_id, created_at desc)
    where owner_user_id is not null;
