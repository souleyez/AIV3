create table if not exists dataset_document_memberships (
    tenant_id uuid not null references tenants (id) on delete cascade,
    dataset_id uuid not null references datasets (id) on delete cascade,
    document_id uuid not null references documents (id) on delete cascade,
    membership_kind text not null default 'curated',
    source text not null default 'manual',
    expires_at timestamptz,
    created_at timestamptz not null default now(),
    primary key (tenant_id, dataset_id, document_id)
);

create index if not exists dataset_document_memberships_document_idx
    on dataset_document_memberships (tenant_id, document_id);

create index if not exists dataset_document_memberships_expiry_idx
    on dataset_document_memberships (tenant_id, expires_at)
    where expires_at is not null;
