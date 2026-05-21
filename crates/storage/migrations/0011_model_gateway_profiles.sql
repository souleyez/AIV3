create table if not exists model_gateway_profiles (
    id uuid primary key,
    tenant_id uuid not null references tenants (id) on delete cascade,
    profile_id text not null,
    display_name text not null,
    lane text not null,
    provider_id text not null,
    model_id text not null,
    base_url text,
    api_path text,
    wire_api text not null default 'openai-compatible',
    auth_mode text not null default 'env_key',
    auth_env_key_name text,
    recommended_preset text,
    max_concurrency integer,
    rpm_limit integer,
    tpm_limit integer,
    timeout_ms integer,
    priority integer not null default 100,
    enabled boolean not null default true,
    capabilities jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, profile_id)
);

create index if not exists model_gateway_profiles_lane_enabled_idx
    on model_gateway_profiles (tenant_id, lane, enabled, priority desc, profile_id);

create table if not exists model_gateway_profile_events (
    id uuid primary key,
    tenant_id uuid not null references tenants (id) on delete cascade,
    profile_id text not null,
    lane text not null,
    event_type text not null,
    latency_ms integer,
    input_tokens integer,
    output_tokens integer,
    error_kind text,
    created_at timestamptz not null default now()
);

create index if not exists model_gateway_profile_events_profile_created_idx
    on model_gateway_profile_events (tenant_id, profile_id, created_at desc);

create index if not exists model_gateway_profile_events_lane_created_idx
    on model_gateway_profile_events (tenant_id, lane, created_at desc);
