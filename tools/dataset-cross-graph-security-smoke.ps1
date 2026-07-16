[CmdletBinding()]
param(
    [string] $CargoBin = "cargo",
    [string[]] $ResponsePath = @(),
    [string[]] $LogPath = @(),
    [string] $ForbiddenMarkerPath = "",
    [switch] $RequireCapturedArtifacts,
    [switch] $CompactJson
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path

function Invoke-CargoFixtureGate {
    param(
        [string] $Name,
        [string] $Filter,
        [string[]] $ExpectedTests
    )

    $previousColor = $env:CARGO_TERM_COLOR
    $env:CARGO_TERM_COLOR = "never"
    Push-Location $repoRoot
    try {
        $previousPreference = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $output = & $CargoBin test -p platform-api $Filter -- --test-threads=1 2>&1
            $exitCode = $LASTEXITCODE
        } finally {
            $ErrorActionPreference = $previousPreference
        }
    } finally {
        Pop-Location
        $env:CARGO_TERM_COLOR = $previousColor
    }

    $outputText = ($output -join "`n")
    $testCounts = @([regex]::Matches($outputText, "running\s+(\d+)\s+tests?") | ForEach-Object {
        [int] $_.Groups[1].Value
    })
    $maximumTestCount = if ($testCounts.Count) { ($testCounts | Measure-Object -Maximum).Maximum } else { 0 }
    $missingTests = @($ExpectedTests | Where-Object { -not $outputText.Contains($_) })
    $passed = $exitCode -eq 0 -and $maximumTestCount -gt 0 -and $missingTests.Count -eq 0
    if (-not $passed) {
        Write-Warning "$Name failed (exit=$exitCode, maximum_test_count=$maximumTestCount, missing_expected_tests=$($missingTests.Count))."
        @($output | Select-Object -Last 40) | ForEach-Object { Write-Warning "$_" }
    }

    [pscustomobject]@{
        name = $Name
        status = if ($passed) { "passed" } else { "failed" }
        filter = $Filter
        exit_code = $exitCode
        maximum_test_count = $maximumTestCount
        expected_test_count = $ExpectedTests.Count
        missing_expected_test_count = $missingTests.Count
    }
}

function Read-ForbiddenMarkers {
    if ([string]::IsNullOrWhiteSpace($ForbiddenMarkerPath)) {
        return @()
    }
    $resolved = Resolve-Path -LiteralPath $ForbiddenMarkerPath
    return @(Get-Content -Encoding UTF8 -LiteralPath $resolved | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

function Measure-CapturedArtifactLeakage {
    param(
        [ValidateSet("response", "log")]
        [string] $Kind,
        [string[]] $Paths,
        [string[]] $ForbiddenMarkers
    )

    if ($Paths.Count -eq 0) {
        return [pscustomobject]@{
            kind = $Kind
            status = "skipped"
            reason = "no captured $Kind artifact supplied"
            artifact_count = 0
            generic_sensitive_hit_count = $null
            forbidden_marker_hit_count = $null
        }
    }

    $genericPatterns = @(
        "\b[a-fA-F0-9]{64}\b",
        "(?:[A-Za-z]:\\|/(?:etc|home|opt|srv|tmp|var)/)",
        "(?:jdbc|mongodb|oracle|postgres(?:ql)?|redis)://",
        "(?:raw[_-]?(?:hash|hmac|value)|password\s*=|user\s*id\s*=)"
    )
    $genericHits = 0
    $markerHits = 0
    foreach ($path in $Paths) {
        $resolved = Resolve-Path -LiteralPath $path
        $rawContent = Get-Content -Raw -Encoding UTF8 -LiteralPath $resolved
        $content = if ($null -eq $rawContent) { "" } else { [string] $rawContent }
        foreach ($pattern in $genericPatterns) {
            $genericHits += [regex]::Matches($content, $pattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase).Count
        }
        foreach ($marker in $ForbiddenMarkers) {
            $offset = 0
            while (($offset = $content.IndexOf($marker, $offset, [System.StringComparison]::Ordinal)) -ge 0) {
                $markerHits += 1
                $offset += [Math]::Max(1, $marker.Length)
            }
        }
    }
    $passed = $genericHits -eq 0 -and $markerHits -eq 0
    return [pscustomobject]@{
        kind = $Kind
        status = if ($passed) { "passed" } else { "failed" }
        artifact_count = $Paths.Count
        generic_sensitive_hit_count = $genericHits
        forbidden_marker_hit_count = $markerHits
    }
}

$suites = @(
    (Invoke-CargoFixtureGate -Name "cross-semantic-contract" -Filter "cross_dataset_semantic_graph" -ExpectedTests @(
        "dataset_semantic_graph_v1_contract_uses_scoped_and_opaque_shared_ids",
        "shared_document_membership_folds_by_shared_document_id_not_membership_row",
        "exact_document_field_and_confirmed_concept_identities_fold_but_source_name_does_not",
        "equal_chinese_labels_are_inferred_only_when_explicitly_enabled_and_never_fold",
        "explicit_fk_and_reference_edges_preserve_requested_evidence_class",
        "non_confirmed_fk_never_uses_confirmed_public_wording",
        "dataset_object_field_and_concept_edges_are_structure_not_reference",
        "duplicate_shared_structure_edges_merge_provenance_without_becoming_cross_links",
        "temporal_complementarity_is_inferred_and_disclaims_record_level_linkage"
    )),
    (Invoke-CargoFixtureGate -Name "relation-evidence-cap" -Filter "semantic_relation_builder" -ExpectedTests @(
        "explicit_fk_parent_reference_and_observed_containment_keep_evidence_classes",
        "evidence_class_cap_never_upgrades_inferred_relationships"
    )),
    (Invoke-CargoFixtureGate -Name "cross-graph-api-sanitization" -Filter "dataset_semantic_graph_support" -ExpectedTests @(
        "cross_graph_gate_is_fail_closed_for_feature_tenant_and_every_dataset",
        "every_explicit_dataset_is_loaded_and_any_denial_is_one_masked_404",
        "auto_neighbors_filter_visibility_then_rank_reliable_evidence_only",
        "etag_is_deterministic_but_changes_with_scope_and_query_limits",
        "public_projection_recomputes_visible_scope_and_drops_internal_material",
        "public_projection_never_promotes_reported_single_dataset_support_to_cross_dataset",
        "joint_analysis_relation_vocabulary_preserves_structure_and_analysis_boundaries",
        "pair_missing_is_non_error_status_and_cache_response_supports_304",
        "absolute_node_and_edge_budget_stays_below_two_megabytes"
    )),
    (Invoke-CargoFixtureGate -Name "dataset-visibility-matrix" -Filter "resource_access" -ExpectedTests @(
        "dataset_visibility_preserves_public_owner_and_secret_semantics",
        "dataset_request_visibility_preserves_local_thread_scope_bypass",
        "explicit_dataset_scope_matrix_is_tenant_bound_and_masked"
    ))
)

$contractSuitePassed = @($suites | Where-Object { $_.name -eq "cross-semantic-contract" -and $_.status -eq "passed" }).Count -eq 1
$relationSuitePassed = @($suites | Where-Object { $_.name -eq "relation-evidence-cap" -and $_.status -eq "passed" }).Count -eq 1
$apiSuitePassed = @($suites | Where-Object { $_.name -eq "cross-graph-api-sanitization" -and $_.status -eq "passed" }).Count -eq 1
$visibilitySuitePassed = @($suites | Where-Object { $_.name -eq "dataset-visibility-matrix" -and $_.status -eq "passed" }).Count -eq 1

$crossSemanticCases = @(
    [pscustomobject]@{ case = "same_content_identity_folds"; status = if ($contractSuitePassed) { "passed" } else { "failed" }; evidence = "opaque exact-content contract fixture" },
    [pscustomobject]@{ case = "same_document_membership_shares"; status = if ($contractSuitePassed) { "passed" } else { "failed" }; evidence = "shared-document membership fixture" },
    [pscustomobject]@{ case = "same_name_different_source_does_not_fold"; status = if ($contractSuitePassed) { "passed" } else { "failed" }; evidence = "source-system identity fixture" },
    [pscustomobject]@{ case = "equal_chinese_label_is_inferred_only"; status = if ($contractSuitePassed) { "passed" } else { "failed" }; evidence = "label-similarity fixture" },
    [pscustomobject]@{ case = "explicit_fk_can_be_confirmed"; status = if ($contractSuitePassed -and $relationSuitePassed) { "passed" } else { "failed" }; evidence = "explicit FK fixture" },
    [pscustomobject]@{ case = "inferred_never_upgrades_confirmed"; status = if ($contractSuitePassed -and $relationSuitePassed) { "passed" } else { "failed" }; evidence = "evidence-cap fixture" }
)

$permissionCases = @(
    [pscustomobject]@{ case = "anonymous_public"; expected = "visible" },
    [pscustomobject]@{ case = "owner_private"; expected = "owner-only" },
    [pscustomobject]@{ case = "secret_binding"; expected = "binding-only" },
    [pscustomobject]@{ case = "local_thread"; expected = "matching-thread-only" },
    [pscustomobject]@{ case = "cross_tenant"; expected = "masked-404" },
    [pscustomobject]@{ case = "mixed_visible_hidden_list"; expected = "whole-request-masked-404" }
) | ForEach-Object {
    [pscustomobject]@{
        case = $_.case
        expected = $_.expected
        status = if ($apiSuitePassed -and $visibilitySuitePassed) { "passed" } else { "failed" }
    }
}

$forbiddenMarkers = Read-ForbiddenMarkers
$responseLeakage = Measure-CapturedArtifactLeakage -Kind "response" -Paths $ResponsePath -ForbiddenMarkers $forbiddenMarkers
$logLeakage = Measure-CapturedArtifactLeakage -Kind "log" -Paths $LogPath -ForbiddenMarkers $forbiddenMarkers
$capturedArtifacts = @($responseLeakage, $logLeakage)
$capturedArtifactFailure = @($capturedArtifacts | Where-Object { $_.status -eq "failed" }).Count -gt 0
$capturedArtifactSkip = @($capturedArtifacts | Where-Object { $_.status -eq "skipped" }).Count -gt 0
$markerConfigurationMissing = [string]::IsNullOrWhiteSpace($ForbiddenMarkerPath) -or @($forbiddenMarkers).Count -eq 0
$liveArtifactIncomplete = $capturedArtifactSkip -or $markerConfigurationMissing
$suiteFailure = @($suites | Where-Object { $_.status -ne "passed" }).Count -gt 0
$matrixCases = @($crossSemanticCases) + @($permissionCases)
$matrixFailure = @($matrixCases | Where-Object { $_.status -ne "passed" }).Count -gt 0
$requiredArtifactFailure = $RequireCapturedArtifacts -and $liveArtifactIncomplete
$passed = -not ($suiteFailure -or $matrixFailure -or $capturedArtifactFailure -or $requiredArtifactFailure)
$fixtureLeakCount = if ($contractSuitePassed -and $apiSuitePassed) { 0 } else { $null }

$report = [pscustomobject]@{
    smoke = "dataset-cross-graph-security"
    mode = "offline-fixture-no-credentials"
    status = if ($passed) { "passed" } else { "failed" }
    suites = $suites
    cross_semantic_cases = $crossSemanticCases
    permission_cases = $permissionCases
    sanitized_contract = [pscustomobject]@{
        status = if ($contractSuitePassed -and $apiSuitePassed) { "passed" } else { "failed" }
        hidden_dataset_title_hits = $fixtureLeakCount
        hidden_contribution_count_hits = $fixtureLeakCount
        raw_hash_hmac_value_hits = $fixtureLeakCount
        internal_path_hits = $fixtureLeakCount
        note = "Counts are fixture assertions from the executed Rust suites, not live-service observations."
    }
    captured_response_scan = $responseLeakage
    captured_log_scan = $logLeakage
    live_service_gate = [pscustomobject]@{
        status = if ($capturedArtifactFailure) { "failed" } elseif ($liveArtifactIncomplete) { "skipped" } else { "passed" }
        reason = if ($capturedArtifactFailure) { "captured artifact leakage count was non-zero" } elseif ($capturedArtifactSkip) { "captured live response/log artifacts were not both supplied" } elseif ($markerConfigurationMissing) { "hidden-dataset marker file was not supplied or was empty" } else { "captured artifacts scanned without printing their content" }
    }
}

if ($CompactJson) {
    $report | ConvertTo-Json -Depth 12 -Compress
} else {
    $report | ConvertTo-Json -Depth 12
}

if (-not $passed) {
    exit 1
}
