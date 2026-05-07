alter table dataset_outputs
    add column if not exists owner_user_id uuid references users (id) on delete set null;

alter table chat_sessions
    add column if not exists user_id uuid references users (id) on delete set null;

create index if not exists dataset_outputs_owner_user_idx
    on dataset_outputs (tenant_id, owner_user_id, created_at desc)
    where owner_user_id is not null;

create index if not exists chat_sessions_user_dataset_idx
    on chat_sessions (tenant_id, user_id, dataset_id, created_at desc)
    where user_id is not null;
