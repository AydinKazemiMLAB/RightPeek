# Measures the docs/PLAN.md §2 budgets against a release build.
#
#   npm run tauri build -- --no-bundle
#   powershell -ExecutionPolicy Bypass -File scripts/measure-budgets.ps1 -Modifiers Ctrl,Alt -Key Q
#
# -Modifiers and -Key describe the configured hotkey (default Ctrl+Shift+Space).
# The script starts the app, measures idle RAM and CPU, presses the hotkey on a
# test window (cold, then warm), measures RAM with the viewer open, and quits.
param(
    [string]$Exe = "$PSScriptRoot\..\src-tauri\target\release\rightpeek.exe",
    [string[]]$Modifiers = @('Ctrl', 'Shift'),
    [string]$Key = 'Space',
    [int]$IdleSeconds = 20
)

$ErrorActionPreference = 'Stop'
# `-File` passes "Ctrl,Alt" as one string; split it into modifier names.
$Modifiers = @($Modifiers -split ',' | ForEach-Object Trim)
$timingLog = Join-Path $env:TEMP 'rightpeek-timing.log'
Remove-Item $timingLog -ErrorAction SilentlyContinue

Add-Type -Namespace RightPeekBench -Name Keys -MemberDefinition @'
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
'@

function Send-Hotkey([string]$KeyName) {
    $vk = if ($KeyName -eq 'Space') { 0x20 } else { [byte][char]$KeyName.ToUpper() }
    $codes = @{ Ctrl = 0x11; Alt = 0x12; Shift = 0x10; Win = 0x5B }
    $mods = @($Modifiers | ForEach-Object { [byte]$codes[$_] })
    $up = 2
    foreach ($m in $mods) { [RightPeekBench.Keys]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
    [RightPeekBench.Keys]::keybd_event($vk, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [RightPeekBench.Keys]::keybd_event($vk, 0, $up, [UIntPtr]::Zero)
    [array]::Reverse($mods)
    foreach ($m in $mods) { [RightPeekBench.Keys]::keybd_event($m, 0, $up, [UIntPtr]::Zero) }
}

# Private working set (MB) of a process tree, via the non-localized WMI class.
function Get-PrivateWorkingSetMB([int]$RootId, [switch]$IncludeChildren) {
    $ids = @($RootId)
    if ($IncludeChildren) {
        $all = Get-CimInstance Win32_Process
        $frontier = @($RootId)
        while ($frontier.Count) {
            $children = @($all | Where-Object { $frontier -contains $_.ParentProcessId } | ForEach-Object ProcessId)
            $ids += $children
            $frontier = $children
        }
    }
    $sum = 0
    foreach ($id in $ids) {
        $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$id" -ErrorAction SilentlyContinue
        if ($perf) { $sum += [double]$perf.WorkingSetPrivate }
    }
    [math]::Round($sum / 1MB, 1)
}

function Read-Timings { @(Get-Content $timingLog -ErrorAction SilentlyContinue | ForEach-Object { [int]($_ -replace '\D', '') }) }

if (Get-Process rightpeek -ErrorAction SilentlyContinue) { throw 'RightPeek is already running; quit it first.' }
$env:RIGHTPEEK_TIMING_LOG = $timingLog
$app = Start-Process $Exe -PassThru
Start-Sleep -Seconds 5

$idleRam = Get-PrivateWorkingSetMB $app.Id
$app.Refresh(); $cpuStart = $app.TotalProcessorTime
Start-Sleep -Seconds $IdleSeconds
$app.Refresh(); $cpuUsed = ($app.TotalProcessorTime - $cpuStart).TotalMilliseconds
$idleCpu = [math]::Round(100 * $cpuUsed / ($IdleSeconds * 1000 * [Environment]::ProcessorCount), 3)

$shell = New-Object -ComObject WScript.Shell
$target = Start-Process powershell -PassThru -WindowStyle Normal -ArgumentList @(
    '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', "$PSScriptRoot\test-target.ps1", '-Seconds', '60')
Start-Sleep -Seconds 4

function Measure-Capture {
    [void]$shell.AppActivate('RightPeekTestTarget'); Start-Sleep -Milliseconds 500
    $before = (Read-Timings).Count
    Send-Hotkey $Key
    $deadline = (Get-Date).AddSeconds(10)
    while ((Read-Timings).Count -eq $before -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
    $timings = Read-Timings
    if ($timings.Count -gt $before) { $timings[-1] } else { $null }
}

$cold = Measure-Capture
Start-Sleep -Seconds 2
$viewerRam = Get-PrivateWorkingSetMB $app.Id -IncludeChildren
$warm = Measure-Capture

Stop-Process -Id $target.Id -ErrorAction SilentlyContinue
Stop-Process -Id $app.Id -ErrorAction SilentlyContinue
Remove-Item Env:RIGHTPEEK_TIMING_LOG

$exeMB = [math]::Round((Get-Item $Exe).Length / 1MB, 2)
[pscustomobject]@{
    'Idle RAM, app only (MB, <= 20)'        = $idleRam
    'Idle CPU (% of all cores, ~0)'         = $idleCpu
    'RAM with viewer incl. WebView2 (<= 120)' = $viewerRam
    'Hotkey -> visible, cold (ms, <= 600)'  = $cold
    'Hotkey -> visible, warm (ms, <= 300)'  = $warm
    'Release exe size (MB)'                 = $exeMB
} | Format-List
