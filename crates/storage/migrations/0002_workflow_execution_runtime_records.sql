alter table llm_invocations
drop constraint if exists llm_invocations_check;

alter table llm_invocations
add constraint llm_invocations_check
check (
    (source_kind = 'dataset_output' and dataset_output_id is not null and chat_message_id is null)
    or
    (source_kind = 'chat_message' and chat_message_id is not null and dataset_output_id is null)
    or
    (source_kind = 'workflow_execution' and dataset_output_id is null and chat_message_id is null)
);

alter table tool_executions
drop constraint if exists tool_executions_check;

alter table tool_executions
add constraint tool_executions_check
check (
    (source_kind = 'dataset_output' and dataset_output_id is not null and chat_message_id is null)
    or
    (source_kind = 'chat_message' and chat_message_id is not null and dataset_output_id is null)
    or
    (source_kind = 'workflow_execution' and dataset_output_id is null and chat_message_id is null)
);
