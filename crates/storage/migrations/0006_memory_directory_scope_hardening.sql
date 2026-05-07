alter table memory_directories
    add column if not exists owner_user_id uuid references users (id) on delete set null;

alter table memory_directories
    add column if not exists source_document_ids uuid[] not null default '{}'::uuid[];

create index if not exists memory_directories_owner_dataset_idx
    on memory_directories (tenant_id, owner_user_id, dataset_id, version_no desc, created_at desc)
    where owner_user_id is not null;

create index if not exists memory_directories_dataset_version_created_idx
    on memory_directories (tenant_id, dataset_id, version_no desc, created_at desc);
