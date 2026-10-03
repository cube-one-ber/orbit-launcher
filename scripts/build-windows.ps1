# Run from an x64 MSVC / KDE Craft environment with Rust's MSVC toolchain installed.
[CmdletBinding()]
param([string]$QMake = "qmake", [switch]$Package)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) { throw "Open an x64 MSVC Developer PowerShell with KDE Craft activated first." }
    $env:QMAKE = (Get-Command $QMake -ErrorAction Stop).Source
    $qtVersion = (& $env:QMAKE -query QT_VERSION).Trim()
    if (-not $qtVersion.StartsWith("6.")) { throw "Qt 6 is required; found $qtVersion" }
    $spec = (& $env:QMAKE -query QMAKE_XSPEC).Trim()
    if ($spec -notmatch 'msvc') { throw "Use an MSVC Qt build with Rust x86_64-pc-windows-msvc, not $spec" }
    $qtBin = (& $env:QMAKE -query QT_INSTALL_BINS).Trim()
    $qtQml = (& $env:QMAKE -query QT_INSTALL_QML).Trim()
    if (-not (Test-Path "$qtQml/org/kde/kirigami/qmldir")) { throw "Kirigami 6 is missing. Install it with KDE Craft using the same Qt toolchain." }
    $env:PATH = "$qtBin;$env:PATH"
    & cargo test --locked --no-default-features --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "Rust tests failed" }
    & cargo build --locked --release --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "Build failed" }
    $exe = Join-Path (Get-Location) 'target/x86_64-pc-windows-msvc/release/orbit.exe'
    Write-Host "Built $exe"
    if ($Package) {
        # A fresh staging directory avoids shipping stale DLLs from a previous build.
        $stage = Join-Path (Get-Location) ("target/windows-package-" + [guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory $stage | Out-Null
        Copy-Item $exe "$stage/orbit.exe"
        & "$qtBin/windeployqt.exe" --release --compiler-runtime --qmldir qml --dir $stage "$stage/orbit.exe"
        if ($LASTEXITCODE -ne 0) { throw "Qt/QML deployment failed" }
        # windeployqt handles Qt, but KDE QML plugins can also depend on KF6 DLLs.
        $queue = [System.Collections.Generic.Queue[string]]::new()
        $seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        Get-ChildItem $stage -Recurse -File | Where-Object { $_.Extension -in '.exe','.dll' } | ForEach-Object { $queue.Enqueue($_.FullName) }
        while ($queue.Count -gt 0) {
            $file = $queue.Dequeue()
            if (-not $seen.Add($file)) { continue }
            $dependencies = & dumpbin.exe /DEPENDENTS $file
            if ($LASTEXITCODE -ne 0) { throw "Dependency inspection failed: $file" }
            foreach ($line in $dependencies) {
                if ($line -match '^\s+([\w.+-]+\.dll)\s*$') {
                    $name = $Matches[1]
                    if ((Test-Path "$qtBin/$name") -and -not (Test-Path "$stage/$name")) {
                        Copy-Item "$qtBin/$name" "$stage/$name"
                        $queue.Enqueue("$stage/$name")
                    }
                }
            }
        }
        "[Paths]`nPrefix=.`nPlugins=.`nQmlImports=qml`n" | Set-Content "$stage/qt.conf" -Encoding ascii
        Copy-Item LICENSE,README.md $stage
        Copy-Item docs/windows.md "$stage/WINDOWS.md"
        # Validate with development import paths removed and only system DLLs on PATH.
        $savedPath = $env:PATH
        $savedImports = $env:QML_IMPORT_PATH
        $savedImports2 = $env:QML2_IMPORT_PATH
        $savedPlugins = $env:QT_PLUGIN_PATH
        try {
            $env:PATH = "$stage;$env:SystemRoot/System32;$env:SystemRoot"
            $env:QML_IMPORT_PATH = ''; $env:QML2_IMPORT_PATH = ''; $env:QT_PLUGIN_PATH = ''
            & "$PSScriptRoot/check-ui.ps1" -Executable "$stage/orbit.exe"
        } finally {
            $env:PATH = $savedPath; $env:QML_IMPORT_PATH = $savedImports
            $env:QML2_IMPORT_PATH = $savedImports2; $env:QT_PLUGIN_PATH = $savedPlugins
        }
        Write-Host "Validated development bundle: $stage"
        Write-Host 'Complete the Windows release checklist and collect third-party license notices before distributing.'
    }
} finally { Pop-Location }
