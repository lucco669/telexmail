param(
    [int]$SampleRootId,
    [ValidateRange(2, 3600)][int]$Seconds = 30,
    [string]$Label = 'idle',
    [string]$OutputFile
)

$ErrorActionPreference = 'Stop'
if (-not ('TelexBenchmark.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
namespace TelexBenchmark {
  public static class Native {
    [StructLayout(LayoutKind.Sequential)] public struct Memory {
      public uint cb, faults;
      public UIntPtr peakWs, ws, peakPaged, paged, peakNonPaged, nonPaged, pagefile, peakPagefile, commit, privateWs;
      public ulong sharedCommit;
    }
    [DllImport("psapi.dll", SetLastError=true)] static extern bool GetProcessMemoryInfo(IntPtr process, ref Memory memory, uint size);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [StructLayout(LayoutKind.Sequential)] public struct Bounds { public int left, top, right, bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Bounds bounds);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window, StringBuilder text, int size);
    [DllImport("user32.dll")] static extern int GetWindowTextLength(IntPtr window);
    delegate bool EnumWindow(IntPtr window, IntPtr unused);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow callback, IntPtr unused);
    public static IntPtr Window(uint process) {
      IntPtr found = IntPtr.Zero;
      EnumWindows((window, unused) => {
        uint owner; GetWindowThreadProcessId(window, out owner);
        if (owner == process && IsWindowVisible(window)) { found = window; return false; }
        return true;
      }, IntPtr.Zero);
      return found;
    }
    public static string Title(uint process) {
      var window = Window(process);
      var text = new StringBuilder(GetWindowTextLength(window) + 1);
      GetWindowText(window, text, text.Capacity);
      return text.ToString();
    }
    public static Memory Read(IntPtr process) {
      var m = new Memory(); m.cb = (uint)Marshal.SizeOf(m);
      if (!GetProcessMemoryInfo(process, ref m, m.cb)) throw new System.ComponentModel.Win32Exception();
      return m;
    }
  }
}
'@
}

function Get-BenchmarkTree([int]$RootId) {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, CreationDate, Name)
    $root = $all | Where-Object ProcessId -eq $RootId | Select-Object -First 1
    if (-not $root) { throw "Benchmark root $RootId exited" }
    $selected = @{$RootId = $root}
    do {
        $added = $false
        foreach ($process in $all) {
            $parent = $selected[[int]$process.ParentProcessId]
            if ($parent -and $process.CreationDate -ge $parent.CreationDate -and -not $selected.ContainsKey([int]$process.ProcessId)) {
                $selected[[int]$process.ProcessId] = $process
                $added = $true
            }
        }
    } while ($added)
    return @($selected.Values)
}

function Measure-BenchmarkTree([int]$RootId, [int]$Seconds = 30, [string]$Label = 'idle') {
    $logicalProcessors = (Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
    $samples = [System.Collections.Generic.List[object]]::new()
    $previousCpu = @{}
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $previousTime = 0.0
    do {
        $rows = [System.Collections.Generic.List[object]]::new()
        $errors = [System.Collections.Generic.List[string]]::new()
        $cpuDelta = 0.0
        foreach ($info in Get-BenchmarkTree $RootId) {
            try {
                $process = Get-Process -Id $info.ProcessId
                $memory = [TelexBenchmark.Native]::Read($process.Handle)
                $cpu = $process.TotalProcessorTime.TotalSeconds
                $key = "$($info.ProcessId):$($info.CreationDate.Ticks)"
                if ($previousCpu.ContainsKey($key)) { $cpuDelta += [math]::Max(0, $cpu - $previousCpu[$key]) }
                elseif ($samples.Count -gt 0) { $cpuDelta += $cpu }
                $previousCpu[$key] = $cpu
                $rows.Add([pscustomobject]@{id=$info.ProcessId; name=$info.Name; privateRamBytes=$memory.privateWs.ToUInt64(); privateCommitBytes=$memory.commit.ToUInt64()})
            } catch { $errors.Add("Couldn't read process $($info.ProcessId): $($_.Exception.Message)") }
        }
        $now = $clock.Elapsed.TotalSeconds
        $interval = $now - $previousTime
        $rootWindow = [TelexBenchmark.Native]::Window($RootId)
        $windowBounds = [TelexBenchmark.Native+Bounds]::new()
        [void][TelexBenchmark.Native]::GetWindowRect($rootWindow, [ref]$windowBounds)
        [uint32]$foregroundId = 0
        [void][TelexBenchmark.Native]::GetWindowThreadProcessId([TelexBenchmark.Native]::GetForegroundWindow(), [ref]$foregroundId)
        $samples.Add([pscustomobject]@{
            elapsedSeconds=$now
            privateRamBytes=($rows | Measure-Object privateRamBytes -Sum).Sum
            privateCommitBytes=($rows | Measure-Object privateCommitBytes -Sum).Sum
            cpuSeconds=if ($samples.Count -gt 0) { $cpuDelta } else { $null }
            cpuPercent=if ($samples.Count -gt 0) { 100 * $cpuDelta / $interval / $logicalProcessors } else { $null }
            foreground=($foregroundId -eq $RootId)
            visible=[TelexBenchmark.Native]::IsWindowVisible($rootWindow)
            minimized=[TelexBenchmark.Native]::IsIconic($rootWindow)
            windowWidth=$windowBounds.right - $windowBounds.left
            windowHeight=$windowBounds.bottom - $windowBounds.top
            processes=@($rows)
            errors=@($errors)
        })
        $previousTime = $now
        if ($now -lt $Seconds) { Start-Sleep -Milliseconds 1000 }
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    if ($samples | Where-Object { $_.errors.Count -gt 0 }) { throw 'Incomplete process memory reads; refusing to produce a benchmark result.' }
    $os = Get-CimInstance Win32_OperatingSystem
    $processor = Get-CimInstance Win32_Processor | Select-Object -First 1
    return [pscustomobject]@{
        schema=1; measuredAt=[DateTimeOffset]::UtcNow.ToString('o'); label=$Label
        durationSeconds=$clock.Elapsed.TotalSeconds; logicalProcessors=$logicalProcessors
        hardware=[pscustomobject]@{os=$os.Caption; build=$os.BuildNumber; cpu=$processor.Name; ramBytes=[uint64]$os.TotalVisibleMemorySize * 1024}
        memoryMetric='sum of private working set, including descendant processes'
        cpuMetric='CPU time delta divided by elapsed time and logical processors; first sample excluded'
        samples=@($samples)
    }
}

if ($SampleRootId) {
    $result = Measure-BenchmarkTree -RootId $SampleRootId -Seconds $Seconds -Label $Label
    $json = $result | ConvertTo-Json -Depth 8
    if ($OutputFile) { $json | Set-Content -LiteralPath $OutputFile -Encoding utf8 }
    else { $json }
}
