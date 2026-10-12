param([ValidateRange(1, 20)][int]$Runs = 5)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$work = Join-Path $repo '.bench'
$exe = Join-Path $work 'telex-benchmark.exe'
$profile = Join-Path $env:APPDATA 'app.telexmail.benchmark'
if (-not (Test-Path -LiteralPath (Join-Path $profile 'telex.db'))) { throw 'Run the fixture export and prepare.mjs first.' }
if (-not (Test-Path -LiteralPath $exe) -or (Get-Item -LiteralPath $exe).VersionInfo.ProductName -ne 'Telex Benchmark') { throw 'Expected the isolated Telex Benchmark executable in .bench.' }
. (Join-Path $PSScriptRoot 'measure.ps1')
$reports = [System.Collections.Generic.List[object]]::new()
for ($run = 1; $run -le $Runs; $run++) {
    Write-Output "Telex desktop run $run/$Runs"
    $startedAt = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    # This is an interactive benchmark window: it must be visible for rendering and RAM measurements.
    $process = Start-Process -FilePath $exe -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(40)
        do {
            Start-Sleep -Milliseconds 50
            $title = [TelexBenchmark.Native]::Title($process.Id)
            if ($title.StartsWith('TELEX_BENCHMARK_ERROR:')) { throw $title }
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Benchmark UI failed to become ready. Check the isolated build and profile.' }
        } until ($title.StartsWith('TELEX_BENCHMARK_READY:'))
        $readyAt = [long]$title.Substring('TELEX_BENCHMARK_READY:'.Length)
        Start-Sleep -Seconds 5
        $resources = Measure-BenchmarkTree -RootId $process.Id -Seconds 30 -Label 'offline-inbox-idle-100k'
        $deadline = [DateTime]::UtcNow.AddSeconds(90)
        do {
            Start-Sleep -Milliseconds 100
            $title = [TelexBenchmark.Native]::Title($process.Id)
            if ($title.StartsWith('TELEX_BENCHMARK_ERROR:')) { throw $title }
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Desktop action benchmark timed out.' }
        } until ($title.StartsWith('TELEX_BENCHMARK_RESULT:'))
        $desktop = $title.Substring('TELEX_BENCHMARK_RESULT:'.Length) | ConvertFrom-Json
        $reports.Add([pscustomobject]@{run=$run; startupMs=$readyAt - $startedAt; desktop=$desktop; resources=$resources})
        Write-Output "Inbox ready in $($readyAt - $startedAt) ms; $($desktop.metrics.Count) action measurements saved."
    } finally {
        # Only the process this script created, with a distinct benchmark app identity.
        if (-not $process.HasExited) {
            [void]$process.CloseMainWindow()
            if (-not $process.WaitForExit(10000)) { Stop-Process -Id $process.Id }
        }
    }
}
$report = [pscustomobject]@{
    schema=1; application='Telex'; version=(Get-Content (Join-Path $repo 'package.json') | ConvertFrom-Json).version
    fixtureId='telex-scale-100k-v1'; messages=100000; scenario='offline-inbox-idle-100k'
    build='release; isolated app identity; UI timing hooks; network checks disabled'
    startup='First run recorded separately; remaining runs are warm launches. No cold-boot claim.'
    runs=@($reports)
}
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $work 'telex-desktop.json') -Encoding utf8
Write-Output 'Saved .bench/telex-desktop.json'
