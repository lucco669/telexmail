$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$securityTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$exe = Join-Path $securityTarget 'debug/telex.exe'
if ((Get-Item -LiteralPath $exe).VersionInfo.ProductName -ne 'Telex Security Check') { throw 'Build the isolated security webview executable first.' }
if (-not ('TelexSecurity.Native' -as [type])) {
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
namespace TelexSecurity {
  public static class Native {
    delegate bool Callback(IntPtr window, IntPtr unused);
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback cb, IntPtr unused);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    public static string Title(uint pid) {
      string found = "";
      EnumWindows((window,unused) => {
        uint owner; GetWindowThreadProcessId(window,out owner);
        if (owner == pid) {
          var text = new StringBuilder(32768); GetWindowText(window,text,text.Capacity);
          if (text.ToString().StartsWith("TELEX_SECURITY_") || text.ToString() == "Telex Security Check") {found=text.ToString(); return false;}
        }
        return true;
      },IntPtr.Zero);
      return found;
    }
  }
}
'@
}
$process = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    do {
        if ($process.HasExited) { throw 'Security webview exited without a result.' }
        $title = [TelexSecurity.Native]::Title($process.Id)
        if ($title.StartsWith('TELEX_SECURITY_ERROR:')) { throw $title }
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Security webview timed out.' }
        Start-Sleep -Milliseconds 100
    } until ($title.StartsWith('TELEX_SECURITY_RESULT:'))
    $result = $title.Substring('TELEX_SECURITY_RESULT:'.Length) | ConvertFrom-Json
    if ($result.status -ne 'passed') { throw 'Security webview returned a failed result.' }
    $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repo '.bench/security/webview-win32.json') -Encoding utf8
    Write-Output "WebView2: $($result.cases) hostile-email render cases and sandbox execution regression passed."
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id }
}
