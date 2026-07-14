create unique index if not exists dataset_semantic_snapshots_tenant_dataset_id_uidx
    on dataset_semantic_snapshots (tenant_id, dataset_id, id);

create table if not exists dataset_semantic_link_snapshots (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    left_dataset_id uuid not null references datasets (id) on delete cascade,
    right_dataset_id uuid not null references datasets (id) on delete cascade,
    left_snapshot_id uuid not null,
    right_snapshot_id uuid not null,
    schema_version text not null,
    generation_version text not null,
    source_fingerprint text not null,
    status text not null,
    manifest jsonb not null default '{}'::jsonb,
    node_count integer not null default 0,
    edge_count integer not null default 0,
    failure_code text,
    generated_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (left_dataset_id < right_dataset_id),
    check (left_snapshot_id <> right_snapshot_id),
    check (status in ('building', 'ready', 'failed', 'superseded')),
    check (jsonb_typeof(manifest) = 'object'),
    check (node_count >= 0),
    check (edge_count >= 0),
    foreign key (tenant_id, left_dataset_id, left_snapshot_id)
        references dataset_semantic_snapshots (tenant_id, dataset_id, id) on delete cascade,
    foreign key (tenant_id, right_dataset_id, right_snapshot_id)
        references dataset_semantic_snapshots (tenant_id, dataset_id, id) on delete cascade,
    unique (tenant_id, left_snapshot_id, right_snapshot_id, generation_version)
);

create index if not exists dataset_semantic_link_snapshots_latest_ready_idx
    on dataset_semantic_link_snapshots (
        tenant_id,
        left_dataset_id,
        right_dataset_id,
        generated_at desc
    )
    where status = 'ready';

create table if not exists dataset_semantic_link_runs (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants (id) on delete cascade,
    left_dataset_id uuid not null references datasets (id) on delete cascade,
    right_dataset_id uuid not null references datasets (id) on delete cascade,
    left_snapshot_id uuid not null,
    right_snapshot_id uuid not null,
    generation_version text not null,
    source_fingerprint text not null,
    status text not null default 'pending',
    priority integer not null default 100,
    attempt_count integer not null default 0,
    max_attempts integer not null default 3,
    available_at timestamptz not null default now(),
    claimed_at timestamptz,
    finished_at timestamptz,
    failure_code text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (left_dataset_id < right_dataset_id),
    check (left_snapshot_id <> right_snapshot_id),
    check (status in ('pending', 'running', 'succeeded', 'retry_wait', 'dead_letter')),
    check (attempt_count >= 0),
    check (max_attempts > 0),
    foreign key (tenant_id, left_dataset_id, left_snapshot_id)
        references dataset_semantic_snapshots (tenant_id, dataset_id, id) on delete cascade,
    foreign key (tenant_id, right_dataset_id, right_snapshot_id)
        references dataset_semantic_snapshots (tenant_id, dataset_id, id) on delete cascade,
    unique (tenant_id, left_snapshot_id, right_snapshot_id, generation_version)
);

create index if not exists dataset_semantic_link_runs_claim_idx
    on dataset_semantic_link_runs (
        tenant_id,
        status,
        priority asc,
        available_at asc,
        created_at asc
    );
