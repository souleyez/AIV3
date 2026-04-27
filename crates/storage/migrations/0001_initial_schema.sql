create extension if not exists pgcrypto;

create table if not exists tenants (
    id uuid primary key default gen_random_uuid(),
    key text not null unique,
    name text not null,
    created_at timestamptz not null default now()
);

create table if not exists users (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    email text not null,
    display_name text not null,
    roles jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now(),
    unique (tenant_id, email)
);

create table if not exists datasets (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    key text not null,
    title text not null,
    description text,
    lifecycle text not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, key)
);

create table if not exists documents (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    title text not null,
    object_key text not null,
    content_type text not null,
    lifecycle text not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists document_chunks (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    chunk_index integer not null,
    content text not null,
    token_count integer not null,
    state text not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (document_id, chunk_index)
);

create table if not exists secret_bindings (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    document_id uuid references documents (id) on delete cascade,
    scope_level text not null,
    provider_key text not null,
    cipher_text text not null,
    fingerprint text not null,
    created_at timestamptz not null default now()
);

create table if not exists secret_grants (
    id uuid primary key default gen_random_uuid(),
    secret_binding_id uuid not null references secret_bindings (id) on delete cascade,
    tenant_id uuid not null references tenants (id) on delete cascade,
    user_id uuid not null references users (id) on delete cascade,
    device_fingerprint text not null,
    state text not null,
    granted_at timestamptz not null default now(),
    expires_at timestamptz
);

create table if not exists workflow_definitions (
    kind text not null,
    version text not null,
    summary text not null,
    is_active boolean not null default false,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    primary key (kind, version)
);

create table if not exists workflow_executions (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid references datasets (id) on delete set null,
    report_plan_id uuid,
    kind text not null,
    version text not null,
    stage text not null,
    status text not null,
    attempt integer not null default 0,
    context jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (kind, version) references workflow_definitions (kind, version)
);

create table if not exists workflow_events (
    id uuid primary key default gen_random_uuid(),
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    sequence_no bigint not null,
    event_name text not null,
    payload jsonb not null,
    created_at timestamptz not null default now(),
    unique (execution_id, sequence_no)
);

create table if not exists workflow_tasks (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    queue text not null,
    task_key text not null,
    payload jsonb not null,
    status text not null,
    attempt integer not null default 0,
    max_attempts integer not null default 3,
    available_at timestamptz not null default now(),
    claimed_at timestamptz,
    finished_at timestamptz,
    error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists prompt_definitions (
    key text primary key,
    surface text not null,
    title text not null,
    created_at timestamptz not null default now()
);

create table if not exists prompt_versions (
    id uuid primary key default gen_random_uuid(),
    prompt_key text not null references prompt_definitions (key) on delete cascade,
    version text not null,
    body text not null,
    is_active boolean not null default false,
    created_at timestamptz not null default now(),
    unique (prompt_key, version)
);

create table if not exists tool_definitions (
    key text primary key,
    title text not null,
    scope_policy text not null,
    created_at timestamptz not null default now()
);

create table if not exists tool_versions (
    id uuid primary key default gen_random_uuid(),
    tool_key text not null references tool_definitions (key) on delete cascade,
    version text not null,
    input_schema jsonb not null,
    output_schema jsonb not null,
    is_active boolean not null default false,
    created_at timestamptz not null default now(),
    unique (tool_key, version)
);

create table if not exists report_plans (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    title text not null,
    objective text not null,
    status text not null,
    theme_key text not null,
    current_ast_version_id uuid,
    ast jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

alter table report_plans
    add column if not exists current_ast_version_id uuid;

create table if not exists report_plan_ast_versions (
    id uuid primary key default gen_random_uuid(),
    plan_id uuid not null references report_plans (id) on delete cascade,
    version_no integer not null,
    ast jsonb not null,
    created_at timestamptz not null default now(),
    unique (plan_id, version_no)
);

create table if not exists report_render_outputs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    plan_id uuid not null references report_plans (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    ast_version_id uuid not null references report_plan_ast_versions (id) on delete restrict,
    surface text not null,
    status text not null,
    asset_manifest jsonb not null,
    created_at timestamptz not null default now()
);

create table if not exists memory_directories (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    version_no integer not null,
    directory_nodes integer not null,
    refreshed_chunks integer not null,
    directory_manifest jsonb not null,
    created_at timestamptz not null default now(),
    unique (tenant_id, dataset_id, version_no)
);

alter table memory_directories
    add column if not exists version_no integer;

with ranked_memory_directories as (
    select id,
           row_number() over (
               partition by tenant_id, dataset_id
               order by created_at asc, id asc
           ) as next_version_no
    from memory_directories
    where version_no is null
)
update memory_directories as target
set version_no = ranked_memory_directories.next_version_no
from ranked_memory_directories
where target.id = ranked_memory_directories.id;

alter table memory_directories
    alter column version_no set not null;

create table if not exists retrieval_evidences (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    document_chunk_id uuid not null references document_chunks (id) on delete cascade,
    chunk_index integer not null,
    source_locator text not null,
    content_excerpt text not null,
    summary text not null,
    payload_filter_key text not null,
    embedding_model text not null,
    recall_score double precision not null,
    evidence_manifest jsonb not null,
    created_at timestamptz not null default now(),
    unique (execution_id, document_chunk_id)
);

create table if not exists dataset_outputs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    prompt text not null,
    output_text text not null,
    memory_directory_id uuid references memory_directories (id) on delete set null,
    output_manifest jsonb not null,
    created_at timestamptz not null default now()
);

alter table dataset_outputs
    add column if not exists retrieval_evidence_ids uuid[] not null default '{}'::uuid[];

create table if not exists assistant_runs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    local_thread_id text,
    user_prompt text not null,
    startup_briefing jsonb not null default '{}'::jsonb,
    selected_scope jsonb not null default '{}'::jsonb,
    scope_candidates jsonb not null default '[]'::jsonb,
    context_policy jsonb not null default '{}'::jsonb,
    evidence_state jsonb not null default '{}'::jsonb,
    service_lane text not null default 'ordinary_chat',
    execution_trail jsonb not null default '[]'::jsonb,
    output_artifacts jsonb not null default '[]'::jsonb,
    runtime_manifest jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists assistant_run_events (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    run_id uuid not null references assistant_runs (id) on delete cascade,
    sequence_no integer not null,
    event_name text not null,
    payload jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    unique (run_id, sequence_no)
);

create table if not exists conversation_memory_items (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    local_thread_id text not null,
    role text not null,
    item_kind text not null,
    summary text not null,
    source_message_refs jsonb not null default '[]'::jsonb,
    artifact_refs jsonb not null default '[]'::jsonb,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists chat_sessions (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    title text not null,
    latest_memory_directory_id uuid references memory_directories (id) on delete set null,
    latest_dataset_output_id uuid references dataset_outputs (id) on delete set null,
    session_manifest jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (execution_id)
);

create table if not exists chat_messages (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    session_id uuid not null references chat_sessions (id) on delete cascade,
    role text not null,
    turn_index integer not null,
    content text not null,
    message_manifest jsonb not null,
    created_at timestamptz not null default now(),
    unique (session_id, turn_index)
);

create table if not exists llm_invocations (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    source_kind text not null,
    dataset_output_id uuid references dataset_outputs (id) on delete cascade,
    chat_message_id uuid references chat_messages (id) on delete cascade,
    sequence_no integer not null,
    mode text not null,
    provider text,
    model text,
    request_id text,
    finish_reason text,
    latency_ms bigint,
    usage jsonb,
    system_prompt_key text,
    system_prompt_version text,
    tool_trace_count integer,
    created_at timestamptz not null default now(),
    check (
        (source_kind = 'dataset_output' and dataset_output_id is not null and chat_message_id is null)
        or
        (source_kind = 'chat_message' and chat_message_id is not null and dataset_output_id is null)
    ),
    unique (execution_id, source_kind, sequence_no)
);

create table if not exists tool_executions (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    execution_id uuid not null references workflow_executions (id) on delete cascade,
    source_kind text not null,
    dataset_output_id uuid references dataset_outputs (id) on delete cascade,
    chat_message_id uuid references chat_messages (id) on delete cascade,
    sequence_no integer not null,
    call_id text,
    tool_name text not null,
    tool_snapshot jsonb,
    status text not null,
    arguments jsonb,
    result jsonb,
    created_at timestamptz not null default now(),
    check (
        (source_kind = 'dataset_output' and dataset_output_id is not null and chat_message_id is null)
        or
        (source_kind = 'chat_message' and chat_message_id is not null and dataset_output_id is null)
    ),
    unique (execution_id, source_kind, sequence_no)
);

create table if not exists report_modules (
    id uuid primary key default gen_random_uuid(),
    plan_id uuid not null references report_plans (id) on delete cascade,
    sort_order integer not null,
    kind text not null,
    title text not null,
    expected_copy text,
    chart_intent text,
    data_binding_slot text,
    metadata jsonb not null default '{}'::jsonb
);

create table if not exists published_reports (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    plan_id uuid not null references report_plans (id) on delete cascade,
    slug text not null unique,
    current_version_id uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists published_report_versions (
    id uuid primary key default gen_random_uuid(),
    report_id uuid not null references published_reports (id) on delete cascade,
    version_no integer not null,
    surface text not null,
    asset_manifest jsonb not null,
    created_at timestamptz not null default now(),
    unique (report_id, version_no, surface)
);

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'workflow_executions_report_plan_fk'
    ) then
        alter table workflow_executions
            add constraint workflow_executions_report_plan_fk
            foreign key (report_plan_id) references report_plans (id) on delete set null;
    end if;
end
$$;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'published_reports_current_version_fk'
    ) then
        alter table published_reports
            add constraint published_reports_current_version_fk
            foreign key (current_version_id) references published_report_versions (id) on delete set null;
    end if;
end
$$;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'report_plans_current_ast_version_fk'
    ) then
        alter table report_plans
            add constraint report_plans_current_ast_version_fk
            foreign key (current_ast_version_id) references report_plan_ast_versions (id) on delete set null;
    end if;
end
$$;

create index if not exists idx_documents_dataset_lifecycle on documents (dataset_id, lifecycle);
create index if not exists idx_document_chunks_document_state on document_chunks (document_id, state, chunk_index);
create index if not exists idx_document_chunks_dataset_state on document_chunks (dataset_id, state, created_at desc);
create index if not exists idx_secret_bindings_dataset_scope on secret_bindings (dataset_id, scope_level);
create index if not exists idx_secret_grants_binding_user_state on secret_grants (secret_binding_id, user_id, state);
create index if not exists idx_workflow_executions_kind_status on workflow_executions (kind, status);
create index if not exists idx_workflow_events_execution_sequence on workflow_events (execution_id, sequence_no);
create index if not exists idx_workflow_tasks_execution_status on workflow_tasks (execution_id, status, available_at);
create index if not exists idx_workflow_tasks_queue_status_available on workflow_tasks (queue, status, available_at);
create index if not exists idx_report_plans_dataset_status on report_plans (dataset_id, status);
create index if not exists idx_report_plan_ast_versions_plan_version on report_plan_ast_versions (plan_id, version_no desc);
create index if not exists idx_report_render_outputs_plan_created on report_render_outputs (plan_id, created_at desc);
create index if not exists idx_report_render_outputs_execution on report_render_outputs (execution_id);
create index if not exists idx_memory_directories_dataset_created on memory_directories (dataset_id, created_at desc);
create unique index if not exists idx_memory_directories_dataset_version on memory_directories (tenant_id, dataset_id, version_no);
create index if not exists idx_memory_directories_execution on memory_directories (execution_id);
create index if not exists idx_retrieval_evidences_dataset_created on retrieval_evidences (dataset_id, created_at desc);
create index if not exists idx_retrieval_evidences_document_created on retrieval_evidences (document_id, created_at desc);
create index if not exists idx_retrieval_evidences_execution on retrieval_evidences (execution_id);
create index if not exists idx_dataset_outputs_dataset_created on dataset_outputs (dataset_id, created_at desc);
create index if not exists idx_dataset_outputs_execution on dataset_outputs (execution_id);
create index if not exists idx_assistant_runs_tenant_created on assistant_runs (tenant_id, created_at desc);
create index if not exists idx_assistant_runs_local_thread on assistant_runs (tenant_id, local_thread_id, created_at desc);
create index if not exists idx_assistant_run_events_run_sequence on assistant_run_events (run_id, sequence_no);
create index if not exists idx_conversation_memory_thread_updated on conversation_memory_items (tenant_id, local_thread_id, updated_at desc);
create index if not exists idx_chat_sessions_dataset_created on chat_sessions (dataset_id, created_at desc);
create index if not exists idx_chat_sessions_execution on chat_sessions (execution_id);
create index if not exists idx_chat_messages_session_turn on chat_messages (session_id, turn_index asc);
create index if not exists idx_llm_invocations_execution on llm_invocations (tenant_id, execution_id, source_kind, sequence_no);
create index if not exists idx_llm_invocations_dataset_output on llm_invocations (dataset_output_id, sequence_no);
create index if not exists idx_llm_invocations_chat_message on llm_invocations (chat_message_id, sequence_no);
create index if not exists idx_tool_executions_execution on tool_executions (tenant_id, execution_id, source_kind, sequence_no);
create index if not exists idx_tool_executions_dataset_output on tool_executions (dataset_output_id, sequence_no);
create index if not exists idx_tool_executions_chat_message on tool_executions (chat_message_id, sequence_no);
