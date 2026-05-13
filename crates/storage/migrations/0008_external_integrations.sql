create table if not exists external_channel_connections (
    id text not null,
    tenant_id uuid not null references tenants (id) on delete cascade,
    platform text not null,
    connection_key text not null,
    display_name text not null,
    config_redacted jsonb not null default '{}'::jsonb,
    status text not null default 'enabled',
    health_status text not null default 'unknown',
    last_event_at timestamptz,
    last_success_at timestamptz,
    last_failure_at timestamptz,
    disabled_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, id),
    unique (tenant_id, connection_key)
);

create table if not exists external_source_connections (
    id text not null,
    tenant_id uuid not null references tenants (id) on delete cascade,
    connector_kind text not null,
    source_key text not null,
    display_name text not null,
    base_url_redacted text not null,
    config_redacted jsonb not null default '{}'::jsonb,
    sync_mode text not null,
    permission_mode text not null,
    health_status text not null default 'unknown',
    last_sync_at timestamptz,
    last_success_at timestamptz,
    last_failure_at timestamptz,
    disabled_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, id),
    unique (tenant_id, source_key)
);

create table if not exists external_principals (
    tenant_id uuid not null references tenants (id) on delete cascade,
    platform text not null,
    external_user_id text not null,
    v3_user_id uuid references users (id) on delete set null,
    trust_level text not null,
    is_disabled boolean not null default false,
    external_department_ids jsonb not null default '[]'::jsonb,
    external_group_ids jsonb not null default '[]'::jsonb,
    external_role_ids jsonb not null default '[]'::jsonb,
    profile_redacted jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, platform, external_user_id)
);

create table if not exists external_permission_snapshots (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    source_id text not null,
    document_external_id text not null,
    revision_external_id text,
    acl_snapshot jsonb not null,
    acl_hash text,
    captured_at timestamptz not null default now(),
    created_at timestamptz not null default now(),
    foreign key (tenant_id, source_id)
        references external_source_connections (tenant_id, id)
        on delete cascade
);

create table if not exists external_message_events (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    channel_connection_id text not null,
    assistant_run_id uuid references assistant_runs (id) on delete set null,
    direction text not null,
    platform text not null,
    conversation_external_id text not null,
    message_external_id text not null,
    idempotency_key text not null,
    payload_summary jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    foreign key (tenant_id, channel_connection_id)
        references external_channel_connections (tenant_id, id)
        on delete cascade,
    unique (tenant_id, idempotency_key)
);

create table if not exists external_action_runs (
    id text not null,
    tenant_id uuid not null references tenants (id) on delete cascade,
    assistant_run_id uuid references assistant_runs (id) on delete set null,
    requester_summary jsonb not null,
    risk_level text not null,
    target_system text not null,
    action_type text not null,
    arguments_redacted jsonb not null default '{}'::jsonb,
    confirmation_state text not null default 'not_required',
    external_request_id text,
    result_summary jsonb not null default '{}'::jsonb,
    failure_kind text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, id)
);

create table if not exists external_sync_runs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    source_id text not null,
    sync_kind text not null,
    status text not null,
    checkpoint jsonb not null default '{}'::jsonb,
    counts jsonb not null default '{}'::jsonb,
    failure_kind text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, source_id)
        references external_source_connections (tenant_id, id)
        on delete cascade
);

create index if not exists external_channel_connections_status_idx
    on external_channel_connections (tenant_id, status, health_status, updated_at desc);

create index if not exists external_source_connections_status_idx
    on external_source_connections (tenant_id, connector_kind, status, health_status, updated_at desc);

create index if not exists external_principals_v3_user_idx
    on external_principals (tenant_id, v3_user_id)
    where v3_user_id is not null;

create index if not exists external_permission_snapshots_document_idx
    on external_permission_snapshots (tenant_id, source_id, document_external_id, captured_at desc);

create unique index if not exists external_permission_snapshots_revision_key
    on external_permission_snapshots (
        tenant_id,
        source_id,
        document_external_id,
        coalesce(revision_external_id, '')
    );

create index if not exists external_message_events_run_idx
    on external_message_events (tenant_id, assistant_run_id, created_at desc)
    where assistant_run_id is not null;

create index if not exists external_message_events_conversation_idx
    on external_message_events (tenant_id, platform, conversation_external_id, created_at desc);

create index if not exists external_action_runs_status_idx
    on external_action_runs (tenant_id, confirmation_state, risk_level, updated_at desc);

create index if not exists external_sync_runs_source_idx
    on external_sync_runs (tenant_id, source_id, created_at desc);
