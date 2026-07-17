create schema if not exists mall_sz02;

-- Composite keys keep every analytics row inside the same tenant and dataset as
-- its platform catalog records. The leading id remains the platform primary key.
create unique index if not exists datasets_tenant_id_id_uidx
    on public.datasets (tenant_id, id);

create unique index if not exists documents_tenant_dataset_id_uidx
    on public.documents (tenant_id, dataset_id, id);

create unique index if not exists documents_tenant_id_id_uidx
    on public.documents (tenant_id, id);

create table if not exists mall_sz02.ingest_batch (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references public.tenants (id) on delete cascade,
    dataset_id uuid not null,
    mall_code text not null,
    batch_key text not null,
    source_kind text not null,
    status text not null default 'pending',
    file_manifest jsonb not null default '{"files":[]}'::jsonb,
    combined_content_sha256 text not null,
    source_file_count integer not null default 0,
    source_row_count bigint not null default 0,
    loaded_row_count bigint not null default 0,
    rejected_row_count bigint not null default 0,
    business_date_from date,
    business_date_to date,
    quality_flags jsonb not null default '[]'::jsonb,
    failure_message text,
    started_at timestamptz,
    completed_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, dataset_id)
        references public.datasets (tenant_id, id) on delete cascade,
    unique (tenant_id, dataset_id, mall_code, id),
    unique (tenant_id, dataset_id, mall_code, batch_key),
    unique (tenant_id, dataset_id, mall_code, combined_content_sha256),
    check (mall_code = upper(btrim(mall_code)) and length(mall_code) > 0),
    check (length(btrim(batch_key)) > 0),
    check (source_kind in ('aibee_traffic_hourly', 'aibee_profile_aggregate')),
    check (status in ('pending', 'validating', 'loading', 'ready', 'failed', 'superseded')),
    check (jsonb_typeof(file_manifest) = 'object'),
    check (
        not (file_manifest ? 'files')
        or jsonb_typeof(file_manifest -> 'files') = 'array'
    ),
    check (combined_content_sha256 ~ '^[0-9a-f]{64}$'),
    check (status <> 'ready' or completed_at is not null),
    check (source_file_count >= 0),
    check (source_row_count >= 0),
    check (loaded_row_count >= 0),
    check (rejected_row_count >= 0),
    check (
        (business_date_from is null and business_date_to is null)
        or (
            business_date_from is not null
            and business_date_to is not null
            and business_date_from <= business_date_to
        )
    ),
    check (jsonb_typeof(quality_flags) = 'array'),
    check (completed_at is null or started_at is null or completed_at >= started_at)
);

create index if not exists ingest_batch_dataset_status_created_idx
    on mall_sz02.ingest_batch (
        tenant_id,
        dataset_id,
        mall_code,
        status,
        created_at desc
    );

create index if not exists ingest_batch_business_date_idx
    on mall_sz02.ingest_batch (
        tenant_id,
        dataset_id,
        mall_code,
        business_date_from,
        business_date_to
    );

create table if not exists mall_sz02.traffic_point_dim (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references public.tenants (id) on delete cascade,
    dataset_id uuid not null,
    batch_id uuid not null,
    source_document_id uuid not null,
    source_row_number bigint not null,
    mall_code text not null,
    source_inventory_mall_id text,
    entity_type smallint not null,
    entity_type_name text,
    entity_name text not null,
    source_entity_id text not null,
    aibee_entity_id text not null,
    floor_code text,
    floor_name text,
    entity_status smallint,
    area numeric,
    l1_retail_format text,
    l2_retail_format text,
    inventory_present boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, dataset_id)
        references public.datasets (tenant_id, id) on delete cascade,
    foreign key (tenant_id, dataset_id, mall_code, batch_id)
        references mall_sz02.ingest_batch (tenant_id, dataset_id, mall_code, id) on delete cascade,
    foreign key (tenant_id, source_document_id)
        references public.documents (tenant_id, id) on delete restrict,
    unique (tenant_id, dataset_id, mall_code, entity_type, aibee_entity_id),
    check (source_row_number > 0),
    check (mall_code = upper(btrim(mall_code)) and length(mall_code) > 0),
    check (
        source_inventory_mall_id is null
        or length(btrim(source_inventory_mall_id)) > 0
    ),
    check (entity_type in (20, 40, 60, 70, 80)),
    check (length(btrim(entity_name)) > 0),
    check (length(btrim(source_entity_id)) > 0),
    check (length(btrim(aibee_entity_id)) > 0),
    check (area is null or area >= 0)
);

create index if not exists traffic_point_dim_browse_idx
    on mall_sz02.traffic_point_dim (
        tenant_id,
        dataset_id,
        mall_code,
        entity_type,
        entity_name
    );

create index if not exists traffic_point_dim_source_document_idx
    on mall_sz02.traffic_point_dim (tenant_id, dataset_id, source_document_id, source_row_number);

create table if not exists mall_sz02.traffic_hourly_fact (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references public.tenants (id) on delete cascade,
    dataset_id uuid not null,
    batch_id uuid not null,
    source_document_id uuid not null,
    source_row_number bigint not null,
    mall_code text not null,
    source_mall_id text not null,
    source_internal_mall_id text not null,
    entity_type smallint not null,
    entity_type_name text,
    entity_name text not null,
    source_entity_id text not null,
    aibee_entity_id text not null,
    business_date date not null,
    hour smallint not null,
    minute smallint not null,
    interval_code text not null default 'H',
    traffic_in bigint not null,
    traffic_out bigint not null,
    visitors bigint not null,
    average_stay numeric not null,
    visitors_metric_available boolean not null,
    average_stay_metric_available boolean not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, dataset_id)
        references public.datasets (tenant_id, id) on delete cascade,
    foreign key (tenant_id, dataset_id, mall_code, batch_id)
        references mall_sz02.ingest_batch (tenant_id, dataset_id, mall_code, id) on delete cascade,
    foreign key (tenant_id, source_document_id)
        references public.documents (tenant_id, id) on delete restrict,
    unique (
        tenant_id,
        dataset_id,
        mall_code,
        entity_type,
        aibee_entity_id,
        business_date,
        hour,
        minute,
        interval_code
    ),
    check (source_row_number > 0),
    check (mall_code = upper(btrim(mall_code)) and length(mall_code) > 0),
    check (length(btrim(source_mall_id)) > 0),
    check (length(btrim(source_internal_mall_id)) > 0),
    check (entity_type in (20, 40, 60, 70, 80)),
    check (length(btrim(entity_name)) > 0),
    check (length(btrim(source_entity_id)) > 0),
    check (length(btrim(aibee_entity_id)) > 0),
    check (hour between 0 and 23),
    check (minute = 0),
    check (interval_code = 'H'),
    check (traffic_in >= 0),
    check (traffic_out >= 0),
    check (visitors >= 0),
    check (average_stay >= 0)
);

create index if not exists traffic_hourly_fact_time_idx
    on mall_sz02.traffic_hourly_fact (
        tenant_id,
        dataset_id,
        mall_code,
        business_date,
        hour,
        minute
    );

create index if not exists traffic_hourly_fact_entity_time_idx
    on mall_sz02.traffic_hourly_fact (
        tenant_id,
        dataset_id,
        mall_code,
        entity_type,
        aibee_entity_id,
        business_date,
        hour,
        minute
    );

create index if not exists traffic_hourly_fact_source_document_idx
    on mall_sz02.traffic_hourly_fact (
        tenant_id,
        dataset_id,
        source_document_id,
        source_row_number
    );

create table if not exists mall_sz02.profile_daily_summary (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references public.tenants (id) on delete cascade,
    dataset_id uuid not null,
    batch_id uuid not null,
    source_document_id uuid not null,
    source_row_number bigint not null,
    mall_code text not null,
    business_date date not null,
    record_count bigint not null,
    unique_pid_count bigint not null,
    duplicate_record_count bigint not null,
    missing_pid_count bigint not null,
    date_mismatch_count bigint not null,
    invalid_age_count bigint not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, dataset_id)
        references public.datasets (tenant_id, id) on delete cascade,
    foreign key (tenant_id, dataset_id, mall_code, batch_id)
        references mall_sz02.ingest_batch (tenant_id, dataset_id, mall_code, id) on delete cascade,
    foreign key (tenant_id, source_document_id)
        references public.documents (tenant_id, id) on delete restrict,
    unique (tenant_id, dataset_id, mall_code, business_date),
    check (source_row_number > 0),
    check (mall_code = upper(btrim(mall_code)) and length(mall_code) > 0),
    check (record_count >= 0),
    check (unique_pid_count >= 0),
    check (duplicate_record_count >= 0),
    check (missing_pid_count >= 0),
    check (date_mismatch_count >= 0),
    check (invalid_age_count >= 0),
    check (unique_pid_count <= record_count),
    check (duplicate_record_count <= record_count),
    check (missing_pid_count <= record_count),
    check (date_mismatch_count <= record_count),
    check (invalid_age_count <= record_count)
);

create index if not exists profile_daily_summary_time_idx
    on mall_sz02.profile_daily_summary (tenant_id, dataset_id, mall_code, business_date);

create table if not exists mall_sz02.profile_distribution_daily (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references public.tenants (id) on delete cascade,
    dataset_id uuid not null,
    batch_id uuid not null,
    source_document_id uuid not null,
    source_row_number bigint not null,
    mall_code text not null,
    business_date date not null,
    dimension_key text not null,
    bucket_key text not null,
    visitor_count bigint not null,
    share numeric not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, dataset_id)
        references public.datasets (tenant_id, id) on delete cascade,
    foreign key (tenant_id, dataset_id, mall_code, batch_id)
        references mall_sz02.ingest_batch (tenant_id, dataset_id, mall_code, id) on delete cascade,
    foreign key (tenant_id, source_document_id)
        references public.documents (tenant_id, id) on delete restrict,
    unique (
        tenant_id,
        dataset_id,
        mall_code,
        business_date,
        dimension_key,
        bucket_key
    ),
    check (source_row_number > 0),
    check (mall_code = upper(btrim(mall_code)) and length(mall_code) > 0),
    check (dimension_key in ('age', 'gender', 'group_type')),
    check (length(btrim(bucket_key)) > 0),
    check (visitor_count >= 0),
    check (share >= 0 and share <= 1)
);

create index if not exists profile_distribution_daily_time_dimension_idx
    on mall_sz02.profile_distribution_daily (
        tenant_id,
        dataset_id,
        mall_code,
        business_date,
        dimension_key
    );

create table if not exists mall_sz02.profile_distribution_period (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references public.tenants (id) on delete cascade,
    dataset_id uuid not null,
    batch_id uuid not null,
    source_document_id uuid not null,
    source_row_number bigint not null,
    mall_code text not null,
    period_start_date date not null,
    period_end_date date not null,
    dimension_key text not null,
    bucket_key text not null,
    visitor_count bigint not null,
    share numeric not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    foreign key (tenant_id, dataset_id)
        references public.datasets (tenant_id, id) on delete cascade,
    foreign key (tenant_id, dataset_id, mall_code, batch_id)
        references mall_sz02.ingest_batch (tenant_id, dataset_id, mall_code, id) on delete cascade,
    foreign key (tenant_id, source_document_id)
        references public.documents (tenant_id, id) on delete restrict,
    unique (
        tenant_id,
        dataset_id,
        mall_code,
        period_start_date,
        period_end_date,
        dimension_key,
        bucket_key
    ),
    check (source_row_number > 0),
    check (mall_code = upper(btrim(mall_code)) and length(mall_code) > 0),
    check (period_start_date <= period_end_date),
    check (dimension_key in ('age', 'gender', 'group_type')),
    check (length(btrim(bucket_key)) > 0),
    check (visitor_count >= 0),
    check (share >= 0 and share <= 1)
);

create index if not exists profile_distribution_period_range_dimension_idx
    on mall_sz02.profile_distribution_period (
        tenant_id,
        dataset_id,
        mall_code,
        period_start_date,
        period_end_date,
        dimension_key
    );
