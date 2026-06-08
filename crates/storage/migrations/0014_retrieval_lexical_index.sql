alter table retrieval_evidences
    add column if not exists search_text text,
    add column if not exists search_terms jsonb not null default '[]'::jsonb,
    add column if not exists search_tsv tsvector,
    add column if not exists search_language text,
    add column if not exists indexed_content_hash text,
    add column if not exists indexed_at timestamptz;

-- Existing rows are backfilled so postgres_lexical can recall older chunks
-- immediately after the backend is enabled. Review row count and migration
-- lock window before applying this to a production-sized database.
update retrieval_evidences
set search_text = coalesce(
        nullif(evidence_manifest #>> '{lexical,search_text}', ''),
        concat_ws(E'\n', summary, content_excerpt, source_locator, payload_filter_key)
    ),
    search_terms = case
        when jsonb_typeof(evidence_manifest #> '{lexical,search_terms}') = 'array'
            then evidence_manifest #> '{lexical,search_terms}'
        else '[]'::jsonb
    end,
    search_tsv = to_tsvector(
        'simple',
        coalesce(
            nullif(evidence_manifest #>> '{lexical,search_text}', ''),
            concat_ws(E'\n', summary, content_excerpt, source_locator, payload_filter_key)
        )
    ),
    search_language = coalesce(nullif(evidence_manifest #>> '{lexical,language}', ''), 'simple'),
    indexed_content_hash = coalesce(
        nullif(evidence_manifest #>> '{lexical,indexed_content_hash}', ''),
        md5(concat_ws(E'\n', summary, content_excerpt, source_locator, payload_filter_key))
    ),
    indexed_at = coalesce(
        (nullif(evidence_manifest #>> '{lexical,indexed_at}', ''))::timestamptz,
        created_at
    )
where search_text is null
   or search_tsv is null
   or indexed_content_hash is null
   or indexed_at is null;

create index if not exists retrieval_evidences_scope_chunk_idx
    on retrieval_evidences (tenant_id, dataset_id, document_id, document_chunk_id, created_at desc);

create index if not exists retrieval_evidences_search_terms_gin_idx
    on retrieval_evidences using gin (search_terms);

create index if not exists retrieval_evidences_search_tsv_gin_idx
    on retrieval_evidences using gin (search_tsv);

create index if not exists retrieval_evidences_indexed_content_hash_idx
    on retrieval_evidences (tenant_id, document_chunk_id, indexed_content_hash, indexed_at desc);
