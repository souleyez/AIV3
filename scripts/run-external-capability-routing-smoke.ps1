param(
    [string]$BaseUrl = "https://v3.elepcloud.com",
    [Parameter(Mandatory = $true)]
    [string]$Bearer,
    [Parameter(Mandatory = $true)]
    [string]$ConnectionId,
    [string]$CasesPath = (Join-Path $PSScriptRoot "..\fixtures\external-channel-capability-routing\cases.jsonl"),
    [string[]]$CaseId = @(),
    [string]$AvailableDocumentSourceId = "third-party-source-main",
    [string[]]$DatasetExternalIds = @(),
    [string]$DefaultPrompt = "DataMax external capability routing smoke. Return customer-safe status or answer only.",
    [int]$TimeoutSeconds = 180
)

$ErrorActionPreference = "Stop"

function Read-RoutingCases {
    param([string]$Path)
    if (-not (Test-Path $Path)) {
        throw "Cases file not found: $Path"
    }
    $cases = @()
    $lineNo = 0
    foreach ($line in Get-Content -LiteralPath $Path -Encoding UTF8) {
        $lineNo++
        $trimmed = $line.Trim()
        if ($trimmed.Length -eq 0 -or $trimmed.StartsWith("#")) {
            continue
        }
        try {
            $cases += ($trimmed | ConvertFrom-Json)
        } catch {
            throw "Invalid JSONL at $Path line $lineNo`: $($_.Exception.Message)"
        }
    }
    return $cases
}

function Get-JsonProp {
    param($Object, [string]$Name)
    if ($null -eq $Object) {
        return $null
    }
    $prop = $Object.PSObject.Properties[$Name]
    if ($null -eq $prop) {
        return $null
    }
    return $prop.Value
}

function Add-IfPresent {
    param([System.Collections.Generic.List[object]]$List, $Value)
    if ($null -ne $Value) {
        [void]$List.Add($Value)
    }
}

function Add-LinkValues {
    param([System.Collections.Generic.List[object]]$List, $Value)
    if ($null -eq $Value) {
        return
    }
    foreach ($link in @($Value)) {
        if ($null -eq $link) {
            continue
        }
        $text = "$link".Trim()
        if ($text.Length -eq 0) {
            continue
        }
        if (-not $List.Contains($text)) {
            [void]$List.Add($text)
        }
    }
}

function Test-UrlFocus {
    param([string]$Url, [string]$ExpectedFocus)
    if ([string]::IsNullOrWhiteSpace($Url) -or [string]::IsNullOrWhiteSpace($ExpectedFocus)) {
        return $false
    }
    if ($Url -notmatch "[?&]focus=([^&#]+)") {
        return $false
    }
    $rawFocus = $Matches[1].Replace("+", " ")
    $decodedFocus = [System.Uri]::UnescapeDataString($rawFocus)
    return $decodedFocus -eq $ExpectedFocus
}

function Read-SseFrame {
    param([string[]]$Lines)
    $eventName = "message"
    $dataParts = New-Object System.Collections.Generic.List[string]
    foreach ($line in $Lines) {
        if ($line.StartsWith("event:")) {
            $eventName = $line.Substring(6).Trim()
        } elseif ($line.StartsWith("data:")) {
            [void]$dataParts.Add($line.Substring(5).TrimStart())
        }
    }
    return [pscustomobject]@{
        Event = $eventName
        Data = ($dataParts -join "`n")
    }
}

function Invoke-RoutingCase {
    param($Case, [string]$RunId)

    $conversationId = "capability-routing-$($Case.case_id)-$RunId"
    $messageId = "msg-$($Case.case_id)-$RunId"
    $body = [ordered]@{
        platform = "generic_chat"
        tenant_external_id = "capability-routing-smoke"
        bot_external_id = "bot-v3"
        conversation_external_id = $conversationId
        sender_external_id = "codex-smoke"
        message_external_id = $messageId
        message_type = "text"
        text = $Case.prompt
        default_prompt = $DefaultPrompt
        output_format = "rich_text"
        render_mode = "normal"
        available_document_source_id = $AvailableDocumentSourceId
        available_document_external_ids = @()
        dataset_external_ids = @($DatasetExternalIds)
        requested_skills = @()
        mention_external_user_ids = @()
        attachment_refs = @()
        idempotency_key = "capability-routing:$($Case.case_id):$RunId"
        received_at = (Get-Date).ToUniversalTime().ToString("o")
    }
    if ($Case.expected_tool -eq "static_page_artifact") {
        $body["artifact_type"] = "static_page"
    }

    $url = ($BaseUrl.TrimEnd("/") + "/v1/external/channels/$ConnectionId/events/stream")
    $jsonBody = $body | ConvertTo-Json -Depth 20 -Compress
    $client = [System.Net.Http.HttpClient]::new()
    $client.Timeout = [TimeSpan]::FromSeconds($TimeoutSeconds)
    $request = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::Post, $url)
    $request.Headers.Authorization = [System.Net.Http.Headers.AuthenticationHeaderValue]::new("Bearer", $Bearer)
    [void]$request.Headers.Accept.Add([System.Net.Http.Headers.MediaTypeWithQualityHeaderValue]::new("text/event-stream"))
    $request.Content = [System.Net.Http.StringContent]::new($jsonBody, [System.Text.Encoding]::UTF8, "application/json")

    $taskStatuses = New-Object System.Collections.Generic.List[object]
    $cardTypes = New-Object System.Collections.Generic.List[object]
    $artifactLinks = New-Object System.Collections.Generic.List[object]
    $replyTypes = New-Object System.Collections.Generic.List[object]
    $rawFrames = New-Object System.Collections.Generic.List[string]

    try {
        $response = $client.SendAsync($request, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        if (-not $response.IsSuccessStatusCode) {
            $bodyText = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
            throw "HTTP $([int]$response.StatusCode) $($response.ReasonPhrase): $bodyText"
        }
        $stream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::UTF8)
        $frameLines = New-Object System.Collections.Generic.List[string]
        while ($true) {
            try {
                if ($reader.EndOfStream) {
                    break
                }
                $line = $reader.ReadLine()
            } catch {
                if ($_.Exception.Message -like "*ResponseEnded*" -or $_.Exception.Message -like "*response ended prematurely*") {
                    break
                }
                throw
            }
            if ($null -eq $line) {
                break
            }
            if ($line -eq "") {
                if ($frameLines.Count -eq 0) {
                    continue
                }
                $frame = Read-SseFrame -Lines $frameLines.ToArray()
                $frameLines.Clear()
                if ($frame.Data.Contains("<V3_TOOL_REQUEST>")) {
                    throw "Raw V3_TOOL_REQUEST leaked in SSE frame for case $($Case.case_id)"
                }
                [void]$rawFrames.Add("$($frame.Event):$($frame.Data)")
                if ($frame.Data.Length -eq 0 -or $frame.Data -eq "[DONE]") {
                    continue
                }
                try {
                    $payload = $frame.Data | ConvertFrom-Json
                } catch {
                    continue
                }

                Add-IfPresent $taskStatuses (Get-JsonProp $payload "task_status")
                Add-IfPresent $taskStatuses (Get-JsonProp $payload "status")
                $data = Get-JsonProp $payload "data"
                Add-IfPresent $taskStatuses (Get-JsonProp $data "task_status")
                Add-IfPresent $taskStatuses (Get-JsonProp $data "status")
                $innerData = Get-JsonProp $data "data"
                Add-IfPresent $taskStatuses (Get-JsonProp $innerData "task_status")
                Add-IfPresent $taskStatuses (Get-JsonProp $innerData "status")

                $response = Get-JsonProp $data "response"
                if ($null -eq $response) {
                    $response = Get-JsonProp $innerData "response"
                }
                $reply = Get-JsonProp $response "reply"
                Add-IfPresent $replyTypes (Get-JsonProp $reply "reply_type")
                Add-IfPresent $taskStatuses (Get-JsonProp $reply "task_status")

                foreach ($card in @(
                    (Get-JsonProp $payload "card"),
                    (Get-JsonProp $data "card"),
                    (Get-JsonProp $innerData "card"),
                    (Get-JsonProp $reply "card")
                )) {
                    Add-IfPresent $cardTypes (Get-JsonProp $card "type")
                    Add-IfPresent $taskStatuses (Get-JsonProp $card "status")
                    Add-LinkValues $artifactLinks (Get-JsonProp $card "artifact_links")
                    Add-LinkValues $artifactLinks (Get-JsonProp $card "public_url")
                    Add-LinkValues $artifactLinks (Get-JsonProp $card "generated_artifact_url")
                    Add-LinkValues $artifactLinks (Get-JsonProp $card "download_url")
                    Add-LinkValues $artifactLinks (Get-JsonProp $card "html_download_url")
                }

                foreach ($links in @(
                    (Get-JsonProp $payload "artifact_links"),
                    (Get-JsonProp $data "artifact_links"),
                    (Get-JsonProp $innerData "artifact_links"),
                    (Get-JsonProp $reply "artifact_links")
                )) {
                    if ($null -eq $links) {
                        continue
                    }
                    Add-LinkValues $artifactLinks $links
                }
            } else {
                [void]$frameLines.Add($line)
            }
        }
    } finally {
        $request.Dispose()
        $client.Dispose()
    }

    $expectedCardType = Get-JsonProp $Case "expected_card_type"
    if ($null -ne $expectedCardType -and "$expectedCardType" -ne "" -and -not $cardTypes.Contains($expectedCardType)) {
        throw "Case $($Case.case_id) expected card type $expectedCardType, saw: $($cardTypes -join ', ')"
    }

    switch ($Case.expected_tool) {
        "static_page_artifact" {
            $hasStaticStatus = ($taskStatuses | Where-Object { "$_" -in @("processing", "static_page_published") -or "$_".StartsWith("static_page_") }).Count -gt 0
            if (-not $hasStaticStatus -and $artifactLinks.Count -eq 0) {
                throw "Case $($Case.case_id) expected static page progress or artifact link, statuses: $($taskStatuses -join ', ')"
            }
        }
        "data_ingestion_analysis" {
            $hasDataStatus = ($taskStatuses | Where-Object { "$_".StartsWith("data_ingestion_analysis_") -or "$_".StartsWith("data_ingestion_staging_") }).Count -gt 0
            if (-not $hasDataStatus) {
                throw "Case $($Case.case_id) expected data ingestion status, saw: $($taskStatuses -join ', ')"
            }
        }
        "collection_setup_analysis" {
            if (-not $cardTypes.Contains("v3_collection_setup_analysis")) {
                throw "Case $($Case.case_id) expected collection setup confirmation card"
            }
        }
        "integration_setup_analysis" {
            if (-not $cardTypes.Contains("v3_integration_setup_analysis")) {
                throw "Case $($Case.case_id) expected integration setup confirmation card"
            }
        }
        "message_channel_outreach" {
            if (-not $cardTypes.Contains("v3_message_channel_outreach")) {
                throw "Case $($Case.case_id) expected message outreach confirmation card"
            }
        }
        default {
            if ($null -eq $Case.expected_tool -and ($rawFrames -join "`n").Contains("<V3_TOOL_REQUEST>")) {
                throw "Case $($Case.case_id) leaked tool request"
            }
        }
    }

    $expectedFocus = Get-JsonProp $Case "expected_focus"
    $focusCheck = "not_configured"
    if ($null -ne $expectedFocus -and "$expectedFocus" -ne "") {
        if ($artifactLinks.Count -gt 0) {
            $focusMatched = ($artifactLinks | Where-Object { Test-UrlFocus -Url "$_" -ExpectedFocus "$expectedFocus" }).Count -gt 0
            if (-not $focusMatched) {
                throw "Case $($Case.case_id) expected static page focus $expectedFocus, links: $($artifactLinks -join ', ')"
            }
            $focusCheck = "matched"
        } else {
            $focusCheck = "skipped_no_artifact_link"
        }
    }

    [pscustomobject]@{
        case_id = $Case.case_id
        expected_tool = $Case.expected_tool
        expected_focus = $expectedFocus
        focus_check = $focusCheck
        statuses = @($taskStatuses.ToArray())
        card_types = @($cardTypes.ToArray())
        artifact_link_count = $artifactLinks.Count
        artifact_links = @($artifactLinks.ToArray())
        reply_types = @($replyTypes.ToArray())
    }
}

$allCases = Read-RoutingCases -Path $CasesPath
if ($CaseId.Count -gt 0) {
    $wanted = [System.Collections.Generic.HashSet[string]]::new([string[]]$CaseId)
    $allCases = @($allCases | Where-Object { $wanted.Contains($_.case_id) })
}
if ($allCases.Count -eq 0) {
    throw "No routing cases selected"
}

$runId = (New-Guid).Guid.Substring(0, 8)
$results = @()
foreach ($case in $allCases) {
    Write-Host "Running $($case.case_id) ..."
    $results += Invoke-RoutingCase -Case $case -RunId $runId
}

$results | ConvertTo-Json -Depth 10
Write-Host "External capability routing smoke passed for $($results.Count) case(s)."
