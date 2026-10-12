# Phase 4 installer smoke test: silent install, launch, silent uninstall, and
# check that nothing is left behind. Runs in CI on clean runners and locally.
#
#   npm run tauri build
#   powershell -ExecutionPolicy Bypass -File scripts/installer-smoke.ps1
#
# Quit any running RightPeek first. A local run uninstalls RightPeek (settings
# in %APPDATA% are kept, as with a normal uninstall that leaves the box unticked).
param(
    [string]$Installer = (Get-ChildItem "$PSScriptRoot\..\src-tauri\target\release\bundle\nsis\*-setup.exe" |
        Sort-Object LastWriteTime | Select-Object -Last 1).FullName
)

$ErrorActionPreference = 'Stop'
$product = 'RightPeek'
$installDir = "$env:LOCALAPPDATA\$product"
$exe = "$installDir\rightpeek.exe"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$product"
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$shortcut = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\$product.lnk"
$appSwitchedKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage\AppSwitched'
$compatKey = 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Store'
$failures = [System.Collections.Generic.List[string]]::new()

function Check([bool]$ok, [string]$what) {
    if ($ok) { "  ok    $what" } else { "  FAIL  $what"; $failures.Add($what) }
}

function Wait-Until([scriptblock]$condition, [int]$seconds = 60) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while (-not (& $condition)) {
        if ((Get-Date) -gt $deadline) { return $false }
        Start-Sleep -Milliseconds 500
    }
    $true
}

if (-not $Installer -or -not (Test-Path $Installer)) { throw "Installer not found. Build it with: npm run tauri build" }
if (Get-Process rightpeek -ErrorAction SilentlyContinue) { throw "Quit RightPeek before running this test." }
"Installer: $Installer ($([math]::Round((Get-Item $Installer).Length / 1MB, 2)) MB)"

"Install"
$p = Start-Process $Installer -ArgumentList '/S' -Wait -PassThru
Check ($p.ExitCode -eq 0) "installer exit code 0 (got $($p.ExitCode))"
Check (Test-Path $exe) "exe installed to $installDir"
Check (Test-Path $uninstallKey) 'listed in Installed apps'
Check (Test-Path $shortcut) 'Start menu shortcut'

"Run"
$app = Start-Process $exe -PassThru
Start-Sleep 5
Check (-not $app.HasExited) 'app is still running after 5 s'
Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
$app.WaitForExit(10000) | Out-Null

"Uninstall"
# Simulate "Start with Windows" and Windows' own usage records so the test
# proves the uninstaller removes them (a 5 s silent run may not create them).
# A clean profile may not have these keys yet (seen on windows-2025).
foreach ($key in $runKey, $appSwitchedKey, $compatKey) {
    if (-not (Test-Path $key)) { New-Item $key -Force | Out-Null }
}
Set-ItemProperty $runKey -Name $product -Value "`"$exe`""
New-ItemProperty $appSwitchedKey -Name $exe -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty $compatKey -Name $exe -Value ([byte[]](0, 0)) -PropertyType Binary -Force | Out-Null
$uninstaller = "$installDir\uninstall.exe"
Check (Test-Path $uninstaller) 'uninstaller present'
# The NSIS uninstaller copies itself to %TEMP% and returns at once, so poll.
Start-Process $uninstaller -ArgumentList '/S' -Wait
Check (Wait-Until { -not (Test-Path $exe) }) 'exe removed'
Check (Wait-Until { -not (Test-Path $uninstallKey) }) 'removed from Installed apps'
Check (-not (Test-Path $shortcut)) 'Start menu shortcut removed'
Check (Wait-Until { $null -eq (Get-ItemProperty $runKey -Name $product -ErrorAction SilentlyContinue) } 10) 'Start with Windows entry removed'
function Has-Value($key, $name) { $null -ne (Get-ItemProperty $key -Name $name -ErrorAction SilentlyContinue) }
Check (-not (Has-Value $appSwitchedKey $exe)) 'Explorer app-switch record removed'
Check (-not (Has-Value $compatKey $exe)) 'Compatibility Assistant record removed'

if ($failures.Count) {
    "FAILED: $($failures.Count) check(s)"
    exit 1
}
'PASSED'
