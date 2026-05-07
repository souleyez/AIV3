alter table users
    add column if not exists email_normalized text,
    add column if not exists email_verified_at timestamptz,
    add column if not exists auth_state text not null default 'active',
    add column if not exists primary_secret_fingerprint text,
    add column if not exists last_login_at timestamptz,
    add column if not exists metadata jsonb not null default '{}'::jsonb;

update users
set email_normalized = lower(trim(email))
where email_normalized is null;

create unique index if not exists users_tenant_email_normalized_idx
    on users (tenant_id, email_normalized)
    where email_normalized is not null;

create table if not exists user_sessions (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    user_id uuid not null references users (id) on delete cascade,
    device_fingerprint text not null,
    session_token_hash text not null unique,
    auth_method text not null,
    created_at timestamptz not null default now(),
    last_seen_at timestamptz not null default now(),
    expires_at timestamptz not null,
    revoked_at timestamptz
);

create index if not exists user_sessions_user_active_idx
    on user_sessions (tenant_id, user_id, expires_at desc)
    where revoked_at is null;

create table if not exists email_verification_challenges (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    email_normalized text not null,
    purpose text not null,
    code_hash text not null,
    attempt_count integer not null default 0,
    max_attempts integer not null default 5,
    expires_at timestamptz not null,
    consumed_at timestamptz,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create index if not exists email_verification_challenges_active_idx
    on email_verification_challenges (tenant_id, email_normalized, purpose, created_at desc)
    where consumed_at is null;

create index if not exists email_verification_challenges_expires_idx
    on email_verification_challenges (expires_at);

create table if not exists auth_audit_events (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    user_id uuid references users (id) on delete set null,
    session_id uuid references user_sessions (id) on delete set null,
    email_normalized text,
    event_name text not null,
    outcome text not null,
    ip_hash text,
    device_fingerprint text,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create index if not exists auth_audit_events_user_created_idx
    on auth_audit_events (tenant_id, user_id, created_at desc)
    where user_id is not null;

create index if not exists auth_audit_events_email_created_idx
    on auth_audit_events (tenant_id, email_normalized, created_at desc)
    where email_normalized is not null;

create index if not exists auth_audit_events_event_created_idx
    on auth_audit_events (tenant_id, event_name, created_at desc);

alter table datasets
    add column if not exists owner_user_id uuid references users (id) on delete set null;

alter table documents
    add column if not exists owner_user_id uuid references users (id) on delete set null;

alter table assistant_runs
    add column if not exists user_id uuid references users (id) on delete set null;

alter table conversation_memory_items
    add column if not exists user_id uuid references users (id) on delete set null;

alter table report_plans
    add column if not exists owner_user_id uuid references users (id) on delete set null;

alter table static_page_drafts
    add column if not exists owner_user_id uuid references users (id) on delete set null;

alter table static_page_render_outputs
    add column if not exists owner_user_id uuid references users (id) on delete set null;

create index if not exists datasets_owner_user_idx
    on datasets (tenant_id, owner_user_id)
    where owner_user_id is not null;

create index if not exists assistant_runs_user_thread_idx
    on assistant_runs (tenant_id, user_id, local_thread_id, created_at desc)
    where user_id is not null;

create index if not exists conversation_memory_items_user_thread_idx
    on conversation_memory_items (tenant_id, user_id, local_thread_id, updated_at desc)
    where user_id is not null;
