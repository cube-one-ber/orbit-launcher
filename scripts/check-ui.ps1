[CmdletBinding()]
param([string]$Executable = "$PSScriptRoot/../target/debug/orbit.exe")
$ErrorActionPreference = 'Stop'
$temp = Join-Path ([IO.Path]::GetTempPath()) ('orbit-ui-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $temp | Out-Null
$old = @{}
foreach ($key in 'QT_FORCE_STDERR_LOGGING','QT_QPA_PLATFORM','QT_QUICK_BACKEND','ORBIT_CONFIG_DIR') { $old[$key] = [Environment]::GetEnvironmentVariable($key) }
try {
    $env:QT_FORCE_STDERR_LOGGING = '1'; $env:QT_QPA_PLATFORM = 'offscreen'; $env:QT_QUICK_BACKEND = 'software'; $env:ORBIT_CONFIG_DIR = "$temp/config"
    $process = Start-Process -FilePath $Executable -ArgumentList '--demo','--ui-test' -PassThru -RedirectStandardOutput "$temp/stdout.log" -RedirectStandardError "$temp/stderr.log"
    if (-not $process.WaitForExit(30000)) { $process.Kill(); throw 'UI test timed out' }
    $process.WaitForExit()
    $log = (Get-Content "$temp/stdout.log","$temp/stderr.log" -Raw) -join "`n"
    Write-Host $log
    if ($process.ExitCode -ne 0 -or $log -notmatch 'ORBIT_UI_TEST_PASS' -or $log -match 'ORBIT_UI_TEST_FAIL|ReferenceError|TypeError|Binding loop|Cannot assign|Unable to assign') { throw 'UI checks failed' }
    if (Test-Path "$temp/config/settings.json") { throw 'Preview mode unexpectedly wrote settings' }
} finally {
    foreach ($key in $old.Keys) { [Environment]::SetEnvironmentVariable($key, $old[$key]) }
    Remove-Item $temp -Recurse -Force
}
