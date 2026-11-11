# Native Windows prereqs for Tauri that mise cannot manage.
# Checks are fast; installs only run when something is missing.
$ErrorActionPreference = 'Stop'

function Test-WebView2 {
    $keys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    )
    foreach ($k in $keys) {
        $pv = (Get-ItemProperty -Path $k -Name pv -ErrorAction SilentlyContinue).pv
        if ($pv -and $pv -ne '0.0.0.0') { return $true }
    }
    return $false
}

function Test-MsvcBuildTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $false }
    $found = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return [bool]$found
}

if (Test-WebView2) {
    Write-Host 'WebView2 runtime: OK'
} else {
    Write-Host 'WebView2 runtime missing - installing via winget...'
    winget install --id Microsoft.EdgeWebView2Runtime --exact --silent --accept-source-agreements --accept-package-agreements
    if ($LASTEXITCODE -ne 0) { throw "WebView2 install failed (exit $LASTEXITCODE)" }
}

if (Test-MsvcBuildTools) {
    Write-Host 'MSVC C++ build tools: OK'
} else {
    Write-Host 'MSVC C++ build tools missing - installing VS 2022 Build Tools via winget (multi-GB download, may show a UAC prompt)...'
    winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --accept-source-agreements --accept-package-agreements --override '--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    if ($LASTEXITCODE -ne 0) { throw "Build Tools install failed (exit $LASTEXITCODE)" }
    Write-Host 'Build Tools installed. Open a new terminal so environment changes take effect.'
}
