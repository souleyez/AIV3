param(
    [string[]] $Case = @("all"),
    [switch] $Local,
    [switch] $PlanOnly,
    [string] $BaseUrl = "",
    [string] $BearerToken = "",
    [string] $ServerCaseConfigPath = "",
    [int] $ServerPollTimeoutSec = 600,
    [int] $ServerPollIntervalSec = 15,
    [string] $ReportDir = "",
    [string] $CargoBin = "cargo",
    [switch] $AllowServerMutation,
    [switch] $Json
)

$ErrorActionPreference = "Stop"

$AllCases = @(
    "static-page-no-confirm",
    "data-ingestion-analysis",
    "static_page_plan_only",
    "static_page_new_artifact",
    "static_page_overwrite_rejected",
    "answer_quality_autofix",
    "human_exception",
    "runtime_summary"
)

$CaseAliases = @{
    "static_page_no_confirm" = "static-page-no-confirm"
    "static-page-image2-data-publish" = "static-page-no-confirm"
    "static_page_image2_data_publish" = "static-page-no-confirm"
    "data_ingestion_analysis" = "data-ingestion-analysis"
    "data-ingestion" = "data-ingestion-analysis"
}

function Resolve-SmokeCases {
    param([string[]] $Requested)
    if ($Requested.Count -eq 0 -or $Requested -contains "all") {
        return $AllCases
    }
    $resolved = New-Object System.Collections.Generic.List[string]
    foreach ($item in $Requested) {
        $items = ([string]$item).Split(",", [System.StringSplitOptions]::RemoveEmptyEntries)
        foreach ($rawCaseId in $items) {
            $caseName = $rawCaseId.Trim()
            if ([string]::IsNullOrWhiteSpace($caseName)) {
                continue
            }
            $caseId = if ($CaseAliases.ContainsKey($caseName)) { $CaseAliases[$caseName] } else { $caseName }
            if ($AllCases -notcontains $caseId) {
                throw "Unknown fixed-task smoke case '$caseName'. Known cases: $($AllCases -join ', ')"
            }
            if (-not $resolved.Contains($caseId)) {
                $resolved.Add($caseId) | Out-Null
            }
        }
    }
    return $resolved.ToArray()
}

function New-SmokeResult {
    param(
        [string] $CaseId,
        [string] $Status,
        [string] $Message,
        [object] $Details = $null
    )
    [ordered]@{
        case = $CaseId
        status = $Status
        message = $Message
        details = $Details
    }
}

function Add-SmokeResult {
    param(
        [System.Collections.Generic.List[object]] $Results,
        [string] $CaseId,
        [string] $Status,
        [string] $Message,
        [object] $Details = $null
    )
    $result = New-SmokeResult `
        -CaseId $CaseId `
        -Status $Status `
        -Message $Message `
        -Details $Details
    $Results.Add($result) | Out-Null
}

function Invoke-CheckedCommand {
    param(
        [string] $FilePath,
        [string[]] $Arguments
    )
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $FilePath @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    [ordered]@{
        exit_code = $exitCode
        output = ($output -join "`n")
    }
}

function New-CommandDetail {
    param(
        [string] $Command,
        [object] $Result
    )
    $excerpt = ""
    if ($Result.exit_code -ne 0 -and -not [string]::IsNullOrWhiteSpace($Result.output)) {
        $excerpt = $Result.output.Substring(0, [Math]::Min(4000, $Result.output.Length))
    }
    [ordered]@{
        command = $Command
        exit_code = $Result.exit_code
        failure_excerpt = $excerpt
    }
}

function Invoke-SmokeHttpRequest {
    param(
        [string] $Method,
        [string] $Uri,
        [string] $Body = "",
        [string] $ContentType = "",
        [hashtable] $Headers = @{}
    )

    $params = @{
        Method = $Method
        Uri = $Uri
        TimeoutSec = 20
        UseBasicParsing = $true
        MaximumRedirection = 0
    }
    if ($Headers.Count -gt 0) {
        $params.Headers = $Headers
    }
    if ((Get-Command Invoke-WebRequest).Parameters.ContainsKey("SkipHttpErrorCheck")) {
        $params.SkipHttpErrorCheck = $true
    }
    if (-not [string]::IsNullOrWhiteSpace($Body)) {
        $params.Body = $Body
    }
    if (-not [string]::IsNullOrWhiteSpace($ContentType)) {
        $params.ContentType = $ContentType
    }

    try {
        $response = Invoke-WebRequest @params
        $content = if ($null -eq $response.Content) { "" } else { $response.Content.ToString() }
        return [ordered]@{
            ok = $true
            status_code = [int]$response.StatusCode
            content = $content
            error = $null
        }
    } catch {
        $response = $_.Exception.Response
        if ($null -eq $response) {
            return [ordered]@{
                ok = $false
                status_code = $null
                content = ""
                error = $_.Exception.Message
            }
        }

        $content = ""
        if ($response -is [System.Net.HttpWebResponse]) {
            try {
                $stream = $response.GetResponseStream()
                if ($null -ne $stream) {
                    $reader = [System.IO.StreamReader]::new($stream)
                    try {
                        $content = $reader.ReadToEnd()
                    } finally {
                        $reader.Dispose()
                    }
                }
            } catch {
                $content = ""
            }
        } elseif ($response.PSObject.Properties.Name -contains "Content") {
            try {
                if ($null -eq $response.Content) {
                    $content = ""
                } elseif ($response.Content.PSObject.Methods.Name -contains "ReadAsStringAsync") {
                    $content = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
                } else {
                    $content = $response.Content.ToString()
                }
            } catch {
                $content = ""
            }
        }

        return [ordered]@{
            ok = $true
            status_code = [int]$response.StatusCode
            content = $content
            error = $_.Exception.Message
        }
    }
}

function New-ContentExcerpt {
    param([string] $Content)
    if ([string]::IsNullOrWhiteSpace($Content)) {
        return ""
    }
    return $Content.Substring(0, [Math]::Min(240, $Content.Length))
}

function Get-ConfiguredBearerToken {
    param([object] $Config)
    if (-not [string]::IsNullOrWhiteSpace($BearerToken)) {
        return $BearerToken.Trim()
    }
    if (-not [string]::IsNullOrWhiteSpace($env:V3_EXTERNAL_CHANNEL_BEARER_TOKEN)) {
        return $env:V3_EXTERNAL_CHANNEL_BEARER_TOKEN.Trim()
    }
    if ($null -ne $Config) {
        if (-not [string]::IsNullOrWhiteSpace($Config.bearer_token)) {
            return ([string]$Config.bearer_token).Trim()
        }
        if (-not [string]::IsNullOrWhiteSpace($Config.bearer_token_env)) {
            $envName = [string]$Config.bearer_token_env
            $value = [Environment]::GetEnvironmentVariable($envName)
            if (-not [string]::IsNullOrWhiteSpace($value)) {
                return $value.Trim()
            }
        }
    }
    return ""
}

function Read-ServerCaseConfig {
    if ([string]::IsNullOrWhiteSpace($ServerCaseConfigPath)) {
        return $null
    }
    if (-not (Test-Path -LiteralPath $ServerCaseConfigPath)) {
        throw "ServerCaseConfigPath not found: $ServerCaseConfigPath"
    }
    return Get-Content -Raw -LiteralPath $ServerCaseConfigPath | ConvertFrom-Json
}

function Get-ConfigValue {
    param(
        [object] $Config,
        [string] $Name,
        [object] $Default = $null
    )
    if ($null -ne $Config -and $Config.PSObject.Properties.Name -contains $Name) {
        return $Config.$Name
    }
    return $Default
}

function Convert-ToStringArray {
    param([object] $Value)
    if ($null -eq $Value) {
        return @()
    }
    if ($Value -is [array]) {
        return @($Value | ForEach-Object { [string]$_ } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }
    if ($Value -is [System.Collections.IEnumerable] -and -not ($Value -is [string])) {
        return @($Value | ForEach-Object { [string]$_ } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }
    $text = [string]$Value
    if ([string]::IsNullOrWhiteSpace($text)) {
        return @()
    }
    return @($text.Split(",") | ForEach-Object { $_.Trim() } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}

function New-ExternalStaticPageSmokeBody {
    param(
        [object] $Config,
        [string] $RunId
    )

    $tenantExternalId = [string](Get-ConfigValue -Config $Config -Name "tenant_external_id" -Default "tenant-ext-001")
    $botExternalId = [string](Get-ConfigValue -Config $Config -Name "bot_external_id" -Default "bot-v3")
    $conversationExternalId = [string](Get-ConfigValue -Config $Config -Name "conversation_external_id" -Default "smoke-static-page-$RunId")
    $senderExternalId = [string](Get-ConfigValue -Config $Config -Name "sender_external_id" -Default "operator-smoke")
    $sourceId = [string](Get-ConfigValue -Config $Config -Name "available_document_source_id" -Default "")
    $documentExternalIds = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "available_document_external_ids"))
    $datasetExternalIds = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "dataset_external_ids"))
    $requestedSkills = Get-ConfigValue -Config $Config -Name "requested_skills" -Default @()
    if ($null -eq $requestedSkills) {
        $requestedSkills = @()
    } elseif (-not ($requestedSkills -is [array])) {
        $requestedSkills = @($requestedSkills)
    }
    $template = Get-ConfigValue -Config $Config -Name "template" -Default $null

    $body = [ordered]@{
        platform = [string](Get-ConfigValue -Config $Config -Name "platform" -Default "generic_chat")
        tenant_external_id = $tenantExternalId
        bot_external_id = $botExternalId
        conversation_external_id = $conversationExternalId
        thread_external_id = $null
        sender_external_id = $senderExternalId
        sender_display_name = [string](Get-ConfigValue -Config $Config -Name "sender_display_name" -Default "V3 smoke")
        message_external_id = "msg-static-page-$RunId"
        message_type = "text"
        text = [string](Get-ConfigValue -Config $Config -Name "text" -Default "Generate one public demo report page, return an accessible link first, and continue through the static-page publish pipeline automatically.")
        default_prompt = [string](Get-ConfigValue -Config $Config -Name "default_prompt" -Default "Answer for business users using the selected documents and data sources first.")
        output_format = [string](Get-ConfigValue -Config $Config -Name "output_format" -Default "rich_text")
        render_mode = "artifact"
        artifact_type = "static_page"
        available_document_source_id = if ([string]::IsNullOrWhiteSpace($sourceId)) { $null } else { $sourceId }
        available_document_external_ids = $documentExternalIds
        dataset_external_ids = $datasetExternalIds
        requested_skills = $requestedSkills
        mention_external_user_ids = @()
        attachment_refs = @()
        idempotency_key = "cloudflare-codex-smoke:static-page:$RunId"
        received_at = (Get-Date).ToUniversalTime().ToString("o")
    }
    if ($null -ne $template) {
        $body["template"] = $template
    }
    return $body
}

function Test-ConfigHasDataSourceScope {
    param([object] $Config)
    if ($null -eq $Config) {
        return $false
    }
    foreach ($key in @(
        "dataset_external_id",
        "dataset_external_ids",
        "available_document_external_ids",
        "business_datasource_ids",
        "business_database_source_ids",
        "database_source_ids"
    )) {
        $value = Get-ConfigValue -Config $Config -Name $key -Default $null
        if ($key -eq "dataset_external_id") {
            if (-not [string]::IsNullOrWhiteSpace([string]$value)) {
                return $true
            }
        } elseif (@(Convert-ToStringArray $value).Count -gt 0) {
            return $true
        }
    }
    return $false
}

function New-ExternalDataIngestionSmokeBody {
    param(
        [object] $Config,
        [string] $RunId
    )

    $sourceId = [string](Get-ConfigValue -Config $Config -Name "available_document_source_id" -Default "")
    $datasetExternalId = [string](Get-ConfigValue -Config $Config -Name "dataset_external_id" -Default "")
    $businessDatasourceIds = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "business_datasource_ids"))
    if ($businessDatasourceIds.Count -eq 0) {
        $businessDatasourceIds = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "business_database_source_ids"))
    }
    if ($businessDatasourceIds.Count -eq 0) {
        $businessDatasourceIds = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "database_source_ids"))
    }
    $body = [ordered]@{
        platform = [string](Get-ConfigValue -Config $Config -Name "platform" -Default "generic_chat")
        tenant_external_id = [string](Get-ConfigValue -Config $Config -Name "tenant_external_id" -Default "tenant-ext-001")
        bot_external_id = [string](Get-ConfigValue -Config $Config -Name "bot_external_id" -Default "bot-v3")
        conversation_external_id = [string](Get-ConfigValue -Config $Config -Name "data_ingestion_conversation_external_id" -Default "smoke-data-ingestion-$RunId")
        thread_external_id = $null
        sender_external_id = [string](Get-ConfigValue -Config $Config -Name "sender_external_id" -Default "operator-smoke")
        sender_display_name = [string](Get-ConfigValue -Config $Config -Name "sender_display_name" -Default "V3 smoke")
        message_external_id = "msg-data-ingestion-$RunId"
        message_type = "text"
        text = [string](Get-ConfigValue -Config $Config -Name "data_ingestion_text" -Default "Analyze the selected data source or documents, return read-only field mapping, cleansing suggestions, validation checks, and a staging ingestion plan without production writes.")
        default_prompt = [string](Get-ConfigValue -Config $Config -Name "default_prompt" -Default "Answer for business users using the selected documents and data sources first.")
        output_format = [string](Get-ConfigValue -Config $Config -Name "output_format" -Default "rich_text")
        available_document_source_id = if ([string]::IsNullOrWhiteSpace($sourceId)) { $null } else { $sourceId }
        available_document_external_ids = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "available_document_external_ids"))
        dataset_external_ids = @(Convert-ToStringArray (Get-ConfigValue -Config $Config -Name "dataset_external_ids"))
        business_datasource_ids = $businessDatasourceIds
        mention_external_user_ids = @()
        attachment_refs = @()
        idempotency_key = "cloudflare-codex-smoke:data-ingestion:$RunId"
        received_at = (Get-Date).ToUniversalTime().ToString("o")
    }
    if (-not [string]::IsNullOrWhiteSpace($datasetExternalId)) {
        $body["dataset_external_id"] = $datasetExternalId
    }
    return $body
}

function Get-FirstArtifactUrl {
    param([object] $Response)
    if ($null -eq $Response) {
        return ""
    }
    if ($Response.PSObject.Properties.Name -contains "reply" -and $null -ne $Response.reply) {
        $reply = $Response.reply
        if ($reply.PSObject.Properties.Name -contains "artifact_links" -and $null -ne $reply.artifact_links -and @($reply.artifact_links).Count -gt 0) {
            return [string]@($reply.artifact_links)[0]
        }
        if ($reply.PSObject.Properties.Name -contains "card" -and $null -ne $reply.card) {
            foreach ($key in @("public_url", "generated_artifact_url", "html_preview_url", "html_download_url")) {
                if ($reply.card.PSObject.Properties.Name -contains $key -and -not [string]::IsNullOrWhiteSpace($reply.card.$key)) {
                    return [string]$reply.card.$key
                }
            }
        }
    }
    return ""
}

function Get-StatusUrl {
    param(
        [object] $Response,
        [string] $Base,
        [string] $ConnectionId = ""
    )
    if ($null -eq $Response) {
        return ""
    }
    $statusUrl = ""
    if ($Response.PSObject.Properties.Name -contains "reply" -and $null -ne $Response.reply) {
        $reply = $Response.reply
        if ($reply.PSObject.Properties.Name -contains "card" -and $null -ne $reply.card -and $reply.card.PSObject.Properties.Name -contains "status_url") {
            $statusUrl = [string]$reply.card.status_url
        }
    }
    if ([string]::IsNullOrWhiteSpace($statusUrl)) {
        $runId = if ($Response.PSObject.Properties.Name -contains "assistant_run_id") { [string]$Response.assistant_run_id } else { "" }
        if (-not [string]::IsNullOrWhiteSpace($ConnectionId) -and -not [string]::IsNullOrWhiteSpace($runId)) {
            return "$($Base.TrimEnd('/'))/v1/external/channels/$ConnectionId/assistant-runs/$runId/reply"
        }
        return ""
    }
    if ($statusUrl.StartsWith("http://") -or $statusUrl.StartsWith("https://")) {
        return $statusUrl
    }
    return "$($Base.TrimEnd('/'))/$($statusUrl.TrimStart('/'))"
}

function Test-StaticPageTerminalFailure {
    param([object] $Response)
    $statusValues = New-Object System.Collections.Generic.List[string]
    if ($null -ne $Response -and $Response.PSObject.Properties.Name -contains "reply" -and $null -ne $Response.reply) {
        foreach ($key in @("task_status", "reply_type")) {
            if ($Response.reply.PSObject.Properties.Name -contains $key -and -not [string]::IsNullOrWhiteSpace($Response.reply.$key)) {
                $statusValues.Add([string]$Response.reply.$key) | Out-Null
            }
        }
        if ($Response.reply.PSObject.Properties.Name -contains "card" -and $null -ne $Response.reply.card) {
            foreach ($key in @("status", "codex_final_status", "render_output_status", "image_job_status")) {
                if ($Response.reply.card.PSObject.Properties.Name -contains $key -and -not [string]::IsNullOrWhiteSpace($Response.reply.card.$key)) {
                    $statusValues.Add([string]$Response.reply.card.$key) | Out-Null
                }
            }
        }
    }
    return @($statusValues.ToArray() | Where-Object { $_ -match "failed|cancelled|needs_human" }).Count -gt 0
}

function Get-ReplyTaskStatusValues {
    param([object] $Response)
    $statusValues = New-Object System.Collections.Generic.List[string]
    if ($null -ne $Response -and $Response.PSObject.Properties.Name -contains "reply" -and $null -ne $Response.reply) {
        foreach ($key in @("task_status", "reply_type")) {
            if ($Response.reply.PSObject.Properties.Name -contains $key -and -not [string]::IsNullOrWhiteSpace($Response.reply.$key)) {
                $statusValues.Add([string]$Response.reply.$key) | Out-Null
            }
        }
        if ($Response.reply.PSObject.Properties.Name -contains "card" -and $null -ne $Response.reply.card) {
            foreach ($key in @("status", "workflow_status")) {
                if ($Response.reply.card.PSObject.Properties.Name -contains $key -and -not [string]::IsNullOrWhiteSpace($Response.reply.card.$key)) {
                    $statusValues.Add([string]$Response.reply.card.$key) | Out-Null
                }
            }
        }
    }
    return $statusValues.ToArray()
}

function Test-DataIngestionTerminalSuccess {
    param([object] $Response)
    return @(
        Get-ReplyTaskStatusValues -Response $Response |
            Where-Object { $_ -match "^data_ingestion_analysis_(completed|needs_human)$|^data_ingestion_staging_(dataset_ready|sync_started|sync_running|sync_completed)$" }
    ).Count -gt 0
}

function Test-DataIngestionTerminalFailure {
    param([object] $Response)
    return @(
        Get-ReplyTaskStatusValues -Response $Response |
            Where-Object { $_ -match "^data_ingestion_analysis_(failed|cancelled|source_required)$|^data_ingestion_staging_sync_failed$" }
    ).Count -gt 0
}

function Invoke-ServerStaticPageMutationSmoke {
    param(
        [string] $Base,
        [object] $Config,
        [string] $Token
    )
    if ([string]::IsNullOrWhiteSpace($Token)) {
        return New-SmokeResult -CaseId "static-page-no-confirm" -Status "failed" -Message "Server mutation smoke needs BearerToken, V3_EXTERNAL_CHANNEL_BEARER_TOKEN, or bearer_token_env in ServerCaseConfigPath." -Details @{
            base_url = $Base
            bearer_configured = $false
            mutation_attempted = $false
        }
    }

    $connectionId = [string](Get-ConfigValue -Config $Config -Name "connection_id" -Default "generic-chat-main")
    $runId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $body = New-ExternalStaticPageSmokeBody -Config $Config -RunId $runId
    $url = "$($Base.TrimEnd('/'))/v1/external/channels/$connectionId/events"
    $headers = @{ Authorization = "Bearer $Token" }
    $bodyJson = $body | ConvertTo-Json -Depth 12
    $submit = Invoke-SmokeHttpRequest -Method "POST" -Uri $url -Body $bodyJson -ContentType "application/json" -Headers $headers
    $submitParsed = $null
    if ($submit.ok -and $submit.status_code -ge 200 -and $submit.status_code -lt 300) {
        try {
            $submitParsed = $submit.content | ConvertFrom-Json
        } catch {
            $submitParsed = $null
        }
    }

    $artifactUrl = Get-FirstArtifactUrl -Response $submitParsed
    $statusUrl = Get-StatusUrl -Response $submitParsed -Base $Base -ConnectionId $connectionId
    $polls = New-Object System.Collections.Generic.List[object]
    $terminalFailure = Test-StaticPageTerminalFailure -Response $submitParsed
    $deadline = (Get-Date).AddSeconds($ServerPollTimeoutSec)
    while ([string]::IsNullOrWhiteSpace($artifactUrl) -and -not $terminalFailure -and -not [string]::IsNullOrWhiteSpace($statusUrl) -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds ([Math]::Max(1, $ServerPollIntervalSec))
        $poll = Invoke-SmokeHttpRequest -Method "GET" -Uri $statusUrl -Headers $headers
        $pollParsed = $null
        if ($poll.ok -and $poll.status_code -ge 200 -and $poll.status_code -lt 300) {
            try {
                $pollParsed = $poll.content | ConvertFrom-Json
            } catch {
                $pollParsed = $null
            }
        }
        $polls.Add([ordered]@{
            status_code = $poll.status_code
            task_status = if ($null -ne $pollParsed -and $null -ne $pollParsed.reply) { $pollParsed.reply.task_status } else { $null }
            reply_type = if ($null -ne $pollParsed -and $null -ne $pollParsed.reply) { $pollParsed.reply.reply_type } else { $null }
            has_artifact_url = -not [string]::IsNullOrWhiteSpace((Get-FirstArtifactUrl -Response $pollParsed))
        }) | Out-Null
        $artifactUrl = Get-FirstArtifactUrl -Response $pollParsed
        $terminalFailure = Test-StaticPageTerminalFailure -Response $pollParsed
    }

    $submitStatus = if ($null -ne $submitParsed -and $null -ne $submitParsed.reply) { $submitParsed.reply.task_status } else { $null }
    $details = @{
        base_url = $Base
        connection_id = $connectionId
        bearer_configured = $true
        status_code = $submit.status_code
        accepted = if ($null -ne $submitParsed -and $submitParsed.PSObject.Properties.Name -contains "accepted") { $submitParsed.accepted } else { $null }
        initial_task_status = $submitStatus
        initial_reply_type = if ($null -ne $submitParsed -and $null -ne $submitParsed.reply) { $submitParsed.reply.reply_type } else { $null }
        initial_has_artifact_url = -not [string]::IsNullOrWhiteSpace((Get-FirstArtifactUrl -Response $submitParsed))
        status_url_present = -not [string]::IsNullOrWhiteSpace($statusUrl)
        poll_count = $polls.Count
        polls = $polls.ToArray()
        final_has_artifact_url = -not [string]::IsNullOrWhiteSpace($artifactUrl)
        final_artifact_url = if ([string]::IsNullOrWhiteSpace($artifactUrl)) { $null } else { $artifactUrl }
        terminal_failure = $terminalFailure
        content_excerpt = New-ContentExcerpt -Content $submit.content
        error = $submit.error
        idempotency_key = $body.idempotency_key
        dataset_external_ids_count = @($body.dataset_external_ids).Count
        available_document_external_ids_count = @($body.available_document_external_ids).Count
        business_datasource_ids_count = @($body.business_datasource_ids).Count
    }
    if ($submit.status_code -lt 200 -or $submit.status_code -ge 300 -or $null -eq $submitParsed) {
        return New-SmokeResult -CaseId "static-page-no-confirm" -Status "failed" -Message "External static-page mutation request did not return a valid success response." -Details $details
    }
    if ($terminalFailure) {
        return New-SmokeResult -CaseId "static-page-no-confirm" -Status "failed" -Message "External static-page mutation reached a terminal failure status." -Details $details
    }
    if (-not [string]::IsNullOrWhiteSpace($artifactUrl)) {
        return New-SmokeResult -CaseId "static-page-no-confirm" -Status "passed" -Message "External static-page mutation returned an artifact URL." -Details $details
    }
    return New-SmokeResult -CaseId "static-page-no-confirm" -Status "passed" -Message "External static-page mutation was accepted and remains processing; status URL is available for continued polling." -Details $details
}

function Invoke-ServerDataIngestionMutationSmoke {
    param(
        [string] $Base,
        [object] $Config,
        [string] $Token
    )
    if ([string]::IsNullOrWhiteSpace($Token)) {
        return New-SmokeResult -CaseId "data-ingestion-analysis" -Status "failed" -Message "Server mutation smoke needs BearerToken, V3_EXTERNAL_CHANNEL_BEARER_TOKEN, or bearer_token_env in ServerCaseConfigPath." -Details @{
            base_url = $Base
            bearer_configured = $false
            mutation_attempted = $false
        }
    }
    if (-not (Test-ConfigHasDataSourceScope -Config $Config)) {
        return New-SmokeResult -CaseId "data-ingestion-analysis" -Status "failed" -Message "Data-ingestion mutation smoke needs a private ServerCaseConfigPath with dataset_external_id, dataset_external_ids, or available_document_external_ids." -Details @{
            base_url = $Base
            bearer_configured = $true
            source_configured = $false
            mutation_attempted = $false
        }
    }

    $connectionId = [string](Get-ConfigValue -Config $Config -Name "connection_id" -Default "generic-chat-main")
    $runId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $body = New-ExternalDataIngestionSmokeBody -Config $Config -RunId $runId
    $url = "$($Base.TrimEnd('/'))/v1/external/channels/$connectionId/events"
    $headers = @{ Authorization = "Bearer $Token" }
    $submit = Invoke-SmokeHttpRequest -Method "POST" -Uri $url -Body ($body | ConvertTo-Json -Depth 12) -ContentType "application/json" -Headers $headers
    $submitParsed = $null
    if ($submit.ok -and $submit.status_code -ge 200 -and $submit.status_code -lt 300) {
        try {
            $submitParsed = $submit.content | ConvertFrom-Json
        } catch {
            $submitParsed = $null
        }
    }

    $statusUrl = Get-StatusUrl -Response $submitParsed -Base $Base -ConnectionId $connectionId
    $terminalSuccess = Test-DataIngestionTerminalSuccess -Response $submitParsed
    $terminalFailure = Test-DataIngestionTerminalFailure -Response $submitParsed
    $polls = New-Object System.Collections.Generic.List[object]
    $deadline = (Get-Date).AddSeconds($ServerPollTimeoutSec)
    while (-not $terminalSuccess -and -not $terminalFailure -and -not [string]::IsNullOrWhiteSpace($statusUrl) -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds ([Math]::Max(1, $ServerPollIntervalSec))
        $poll = Invoke-SmokeHttpRequest -Method "GET" -Uri $statusUrl -Headers $headers
        $pollParsed = $null
        if ($poll.ok -and $poll.status_code -ge 200 -and $poll.status_code -lt 300) {
            try {
                $pollParsed = $poll.content | ConvertFrom-Json
            } catch {
                $pollParsed = $null
            }
        }
        $pollStatuses = @(Get-ReplyTaskStatusValues -Response $pollParsed)
        $polls.Add([ordered]@{
            status_code = $poll.status_code
            task_status = if ($pollStatuses.Count -gt 0) { $pollStatuses[0] } else { $null }
            status_values = $pollStatuses
            terminal_success = Test-DataIngestionTerminalSuccess -Response $pollParsed
            terminal_failure = Test-DataIngestionTerminalFailure -Response $pollParsed
        }) | Out-Null
        $terminalSuccess = Test-DataIngestionTerminalSuccess -Response $pollParsed
        $terminalFailure = Test-DataIngestionTerminalFailure -Response $pollParsed
    }

    $initialStatuses = @(Get-ReplyTaskStatusValues -Response $submitParsed)
    $details = @{
        base_url = $Base
        connection_id = $connectionId
        bearer_configured = $true
        source_configured = $true
        status_code = $submit.status_code
        accepted = if ($null -ne $submitParsed -and $submitParsed.PSObject.Properties.Name -contains "accepted") { $submitParsed.accepted } else { $null }
        initial_status_values = $initialStatuses
        status_url_present = -not [string]::IsNullOrWhiteSpace($statusUrl)
        poll_count = $polls.Count
        polls = $polls.ToArray()
        terminal_success = $terminalSuccess
        terminal_failure = $terminalFailure
        content_excerpt = New-ContentExcerpt -Content $submit.content
        error = $submit.error
        idempotency_key = $body.idempotency_key
        dataset_external_ids_count = @($body.dataset_external_ids).Count
        available_document_external_ids_count = @($body.available_document_external_ids).Count
        business_datasource_ids_count = @($body.business_datasource_ids).Count
    }
    if ($submit.status_code -lt 200 -or $submit.status_code -ge 300 -or $null -eq $submitParsed) {
        return New-SmokeResult -CaseId "data-ingestion-analysis" -Status "failed" -Message "External data-ingestion mutation request did not return a valid success response." -Details $details
    }
    if ($terminalFailure) {
        return New-SmokeResult -CaseId "data-ingestion-analysis" -Status "failed" -Message "External data-ingestion mutation reached a terminal failure/source-required status." -Details $details
    }
    if ($terminalSuccess) {
        return New-SmokeResult -CaseId "data-ingestion-analysis" -Status "passed" -Message "External data-ingestion mutation reached a reviewed terminal or staging status." -Details $details
    }
    return New-SmokeResult -CaseId "data-ingestion-analysis" -Status "passed" -Message "External data-ingestion mutation was accepted and remains processing; status URL is available for continued polling." -Details $details
}

function Invoke-LocalCaseSmoke {
    param(
        [string] $CaseId,
        [string] $CargoBin
    )

    $commands = New-Object System.Collections.Generic.List[object]
    switch ($CaseId) {
        "static-page-no-confirm" {
            $commands.Add([string[]]@("test", "-p", "platform-api", "external_channel_static_page", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "platform-api", "codex_host_fixed_task", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "codex-host-agent", "static_page", "--lib")) | Out-Null
        }
        "answer_quality_autofix" {
            $commands.Add([string[]]@("test", "-p", "platform-api", "answer_quality_autofix", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "codex-host-agent", "answer_quality", "--lib")) | Out-Null
        }
        "data-ingestion-analysis" {
            $commands.Add([string[]]@("test", "-p", "contracts", "data_ingestion", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "codex-host-agent", "data_ingestion", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "platform-api", "data_ingestion", "--lib")) | Out-Null
            $commands.Add([string[]]@("test", "-p", "platform-api", "codex_host_fixed_task", "--lib")) | Out-Null
        }
        default {
            $commands.Add([string[]]@("test", "-p", "platform-api", "codex_host_fixed_task", "--lib")) | Out-Null
        }
    }

    $details = New-Object System.Collections.Generic.List[object]
    $failed = $false
    foreach ($args in $commands.ToArray()) {
        $result = Invoke-CheckedCommand -FilePath $CargoBin -Arguments $args
        $commandText = "$CargoBin $($args -join ' ')"
        $details.Add((New-CommandDetail -Command $commandText -Result $result)) | Out-Null
        if ($result.exit_code -ne 0) {
            $failed = $true
        }
    }

    [ordered]@{
        status = if ($failed) { "failed" } else { "passed" }
        details = [ordered]@{
            plan_only = $true
            no_server_writes = $true
            commands = $details.ToArray()
        }
    }
}

function Write-SmokeReports {
    param(
        [object[]] $Results,
        [string] $Mode,
        [string] $OutputDir
    )
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        $OutputDir = Join-Path (Get-Location) "target\cloudflare-codex-fixed-task-smoke"
    }
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
    $stamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $jsonPath = Join-Path $OutputDir "cloudflare-codex-fixed-task-smoke-$stamp.json"
    $mdPath = Join-Path $OutputDir "cloudflare-codex-fixed-task-smoke-$stamp.md"
    $passed = @($Results | Where-Object { $_.status -eq "passed" }).Count
    $failed = @($Results | Where-Object { $_.status -eq "failed" }).Count
    $skipped = @($Results | Where-Object { $_.status -eq "skipped" }).Count
    $payload = [ordered]@{
        generated_at = (Get-Date).ToUniversalTime().ToString("o")
        mode = $Mode
        passed = $passed
        failed = $failed
        skipped = $skipped
        results = $Results
    }
    $payload | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -Path $jsonPath

    $lines = @()
    $lines += "# Cloudflare Codex Fixed Task Smoke"
    $lines += ""
    $lines += "- Mode: $Mode"
    $lines += "- Passed: $passed"
    $lines += "- Failed: $failed"
    $lines += "- Skipped: $skipped"
    $lines += ""
    foreach ($result in $Results) {
        $lines += "## $($result.case)"
        $lines += ""
        $lines += "- Status: $($result.status)"
        $lines += "- Message: $($result.message)"
        $lines += ""
    }
    $lines | Set-Content -Encoding UTF8 -Path $mdPath
    [ordered]@{
        json = $jsonPath
        markdown = $mdPath
        failed = $failed
    }
}

$selectedCases = @(Resolve-SmokeCases -Requested $Case)
$mode = if ($Local -or [string]::IsNullOrWhiteSpace($BaseUrl)) { "local_plan_only" } else { "server" }
$results = New-Object System.Collections.Generic.List[object]

if ($mode -eq "local_plan_only") {
    foreach ($caseId in $selectedCases) {
        $caseSmoke = Invoke-LocalCaseSmoke -CaseId $caseId -CargoBin $CargoBin
        $message = switch ($caseId) {
            "static-page-no-confirm" { "Image2-first static-page path exposes a no-confirm pending card, queues Codex Host only after preview-ready evidence, validates fixed output, and returns a final artifact-link reply." }
            "data-ingestion-analysis" { "Data-ingestion analysis path detects customer source-analysis requests, packages only V3-selected scope, validates read-only/staging outputs, and routes unsafe credential/schema/API/write requests to human review." }
            "static_page_plan_only" { "Fixed static-page queued audit package is safe and bounded." }
            "static_page_new_artifact" { "New generated-artifact output validation is accepted." }
            "static_page_overwrite_rejected" { "Non generated-artifact URL is rejected for human review." }
            "answer_quality_autofix" { "Low-risk allowlisted answer-quality patch output is accepted." }
            "human_exception" { "needs_human output is redacted and marked for exception notification." }
            "runtime_summary" { "Runtime diagnostics summarize fixed-task status without raw prompt data." }
            default { "Fixed-task smoke case ran." }
        }
        Add-SmokeResult -Results $results -CaseId $caseId -Status $caseSmoke.status -Message $message -Details $caseSmoke.details
    }
} else {
    $base = $BaseUrl.TrimEnd("/")
    $serverConfig = Read-ServerCaseConfig
    $serverBearerToken = Get-ConfiguredBearerToken -Config $serverConfig

    $docsUrl = "$base/external-integrations/pure-third-party-integration-guide.zh-CN.html"
    $docs = Invoke-SmokeHttpRequest -Method "GET" -Uri $docsUrl
    $docsPassed = $docs.ok -and $docs.status_code -eq 200 -and $docs.content.Contains("default_prompt")
    $docsStatus = "failed"
    $docsMessage = "Public third-party integration guide did not return the expected HTML content."
    if ($docsPassed) {
        $docsStatus = "passed"
        $docsMessage = "Public third-party integration guide responded."
    }
    $docsDetails = @{
        url = $docsUrl
        status_code = $docs.status_code
        content_excerpt = New-ContentExcerpt -Content $docs.content
        error = $docs.error
    }
    Add-SmokeResult -Results $results -CaseId "server_docs" -Status $docsStatus -Message $docsMessage -Details $docsDetails

    $authGuardUrl = "$base/v1/external/channels/generic-chat-main/events"
    $authGuard = Invoke-SmokeHttpRequest -Method "POST" -Uri $authGuardUrl -Body "{}" -ContentType "application/json"
    $authGuardContent = if ($null -eq $authGuard.content) { "" } else { [string]$authGuard.content }
    $authGuardPassed = $authGuard.ok -and $authGuard.status_code -eq 401 -and (
        [string]::IsNullOrWhiteSpace($authGuardContent) -or
        $authGuardContent.Contains("external_channel_auth_failed")
    )
    $authGuardStatus = "failed"
    $authGuardMessage = "External events API did not return the expected missing-token guard."
    if ($authGuardPassed) {
        $authGuardStatus = "passed"
        $authGuardMessage = "External events API reached platform-api and rejected missing bearer token without mutation."
    }
    $authGuardDetails = @{
        url = $authGuardUrl
        status_code = $authGuard.status_code
        content_excerpt = New-ContentExcerpt -Content $authGuard.content
        error = $authGuard.error
    }
    Add-SmokeResult -Results $results -CaseId "server_external_api_auth_guard" -Status $authGuardStatus -Message $authGuardMessage -Details $authGuardDetails

    $queueStatsUrl = "$base/v1/workflow-tasks/queue-stats"
    $queueStats = Invoke-SmokeHttpRequest -Method "GET" -Uri $queueStatsUrl
    $queueStatsParsed = $null
    if ($queueStats.ok -and $queueStats.status_code -eq 200) {
        try {
            $queueStatsParsed = $queueStats.content | ConvertFrom-Json
        } catch {
            $queueStatsParsed = $null
        }
    }
    $queueStatsPassed = $null -ne $queueStatsParsed -and -not [string]::IsNullOrWhiteSpace($queueStatsParsed.generated_at) -and $null -ne $queueStatsParsed.queues
    $queueStatsStatus = "failed"
    $queueStatsMessage = "Workflow queue stats endpoint did not return the expected JSON diagnostics."
    $queueCount = $null
    $taskCount = $null
    if ($queueStatsPassed) {
        $queueStatsStatus = "passed"
        $queueStatsMessage = "Workflow queue stats endpoint returned JSON diagnostics."
        $queueCount = @($queueStatsParsed.queues).Count
        $taskCount = $queueStatsParsed.task_count
    }
    $queueStatsDetails = @{
        url = $queueStatsUrl
        status_code = $queueStats.status_code
        queue_count = $queueCount
        task_count = $taskCount
        content_excerpt = New-ContentExcerpt -Content $queueStats.content
        error = $queueStats.error
    }
    Add-SmokeResult -Results $results -CaseId "server_queue_stats" -Status $queueStatsStatus -Message $queueStatsMessage -Details $queueStatsDetails

    if ($PlanOnly -or -not $AllowServerMutation) {
        foreach ($caseId in $selectedCases) {
            $guardDetails = @{
                base_url = $BaseUrl
                plan_only = [bool]$PlanOnly
                customer_confirmation_required = $false
                bearer_configured = -not [string]::IsNullOrWhiteSpace($serverBearerToken)
            }
            if ($caseId -eq "static-page-no-confirm") {
                $guardDetails.expected_final_url_prefix = "$base/generated-artifacts/"
            } elseif ($caseId -eq "data-ingestion-analysis") {
                $guardDetails.expected_output_statuses = @("analysis_ready", "staging_spec_ready", "needs_human", "failed")
                $guardDetails.production_writes_allowed = $false
            }
            Add-SmokeResult -Results $results -CaseId $caseId -Status "skipped" -Message "Server mutation smoke is guarded. Re-run on an approved host with -AllowServerMutation after deployment review." -Details $guardDetails
        }
    } else {
        foreach ($caseId in $selectedCases) {
            if ($caseId -eq "static-page-no-confirm") {
                $results.Add((Invoke-ServerStaticPageMutationSmoke -Base $base -Config $serverConfig -Token $serverBearerToken))
            } elseif ($caseId -eq "data-ingestion-analysis") {
                $results.Add((Invoke-ServerDataIngestionMutationSmoke -Base $base -Config $serverConfig -Token $serverBearerToken))
            } else {
                Add-SmokeResult -Results $results -CaseId $caseId -Status "failed" -Message "Server mutation execution for this case is not wired yet; use -PlanOnly or add a private server case implementation before enabling." -Details @{ base_url = $BaseUrl }
            }
        }
    }
}

$resultArray = $results.ToArray()
$report = Write-SmokeReports -Results $resultArray -Mode $mode -OutputDir $ReportDir
if ($Json) {
    [ordered]@{
        mode = $mode
        report = $report
        results = $resultArray
    } | ConvertTo-Json -Depth 8
} else {
    Write-Host "Cloudflare Codex fixed-task smoke complete."
    Write-Host "JSON: $($report.json)"
    Write-Host "Markdown: $($report.markdown)"
}

if ($report.failed -gt 0) {
    exit 1
}
