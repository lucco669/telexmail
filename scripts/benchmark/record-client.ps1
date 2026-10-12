param(
    [Parameter(Mandatory)][int]$RootId,
    [Parameter(Mandatory)][ValidateSet('thunderbird','outlook-classic','outlook-new')][string]$Client,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Notes,
    [ValidateRange(2,20)][int]$Runs = 5
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$work = Join-Path $repo '.bench'
. (Join-Path $PSScriptRoot 'measure.ps1')
$verificationFile = Join-Path $work "$Client-verified.json"
if (-not (Test-Path -LiteralPath $verificationFile)) { throw 'Verify the imported corpus and completed indexing first; see docs/performance.md.' }
$verification = Get-Content -LiteralPath $verificationFile | ConvertFrom-Json
if ($verification.messages -ne 100000 -or $verification.fixtureId -ne 'telex-scale-100k-v1' -or -not $verification.indexingComplete) { throw 'The fixture verification is incomplete.' }
if ($verification.processId -ne $RootId) { throw 'Verify the corpus for this benchmark process before recording it.' }
$rootInfo = Get-CimInstance Win32_Process -Filter "ProcessId = $RootId"
if ($Client -eq 'thunderbird') {
    $expectedProfile = (Join-Path $work 'thunderbird').Replace('\','/')
    if ($rootInfo.Name -ne 'thunderbird.exe' -or -not $rootInfo.CommandLine.Replace('\','/').Contains($expectedProfile, [StringComparison]::OrdinalIgnoreCase)) { throw 'Expected the isolated Thunderbird benchmark profile.' }
}
$names = @{thunderbird='Thunderbird'; 'outlook-classic'='Outlook (classic)'; 'outlook-new'='Outlook (new)'}
$results = [System.Collections.Generic.List[object]]::new()
for ($run = 1; $run -le $Runs; $run++) {
    Write-Output "$($names[$Client]) idle sample $run/$Runs"
    Start-Sleep -Seconds 5
    $results.Add((Measure-BenchmarkTree -RootId $RootId -Seconds 30 -Label 'offline-inbox-idle-100k'))
    $report = [pscustomobject]@{
        schema=1; application=$names[$Client]; version=$Version; notes=$Notes
        fixtureId=$verification.fixtureId; messages=$verification.messages; scenario='offline-inbox-idle-100k'
        verification=$verification; runs=@($results)
    }
    $report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $work "$Client.json") -Encoding utf8
}
Write-Output "Saved .bench/$Client.json"
