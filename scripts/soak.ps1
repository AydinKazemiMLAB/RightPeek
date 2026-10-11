# Phase 3 soak test: drives the hotkey for -Minutes and checks RightPeek
# never dies and its memory stays flat. Uses the keyboard, so leave the PC alone.
#
#   npm run tauri build -- --no-bundle
#   powershell -ExecutionPolicy Bypass -File scripts/soak.ps1 -Minutes 30
#
# Each round: capture text from a test window, close the viewer with Esc,
# then capture from an empty window (the "No text selected" notice).
param(
    [string]$Exe = "$PSScriptRoot\..\src-tauri\target\release\rightpeek.exe",
    [string[]]$Modifiers = @('Ctrl', 'Shift'),
    [string]$Key = 'Space',
    [int]$Minutes = 30,
    [int]$PauseSeconds = 3
)

$ErrorActionPreference = 'Stop'
$Modifiers = @($Modifiers -split ',' | ForEach-Object Trim)

Add-Type -Namespace RightPeekSoak -Name Keys -MemberDefinition @'
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern System.IntPtr FindWindow(string cls, string title);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
'@

function Send-Keys([byte[]]$Mods, [byte]$Vk) {
    foreach ($m in $Mods) { [RightPeekSoak.Keys]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
    [RightPeekSoak.Keys]::keybd_event($Vk, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [RightPeekSoak.Keys]::keybd_event($Vk, 0, 2, [UIntPtr]::Zero)
    [array]::Reverse($Mods)
    foreach ($m in $Mods) { [RightPeekSoak.Keys]::keybd_event($m, 0, 2, [UIntPtr]::Zero) }
}

$codes = @{ Ctrl = 0x11; Alt = 0x12; Shift = 0x10; Win = 0x5B }
$hotkeyMods = [byte[]]@($Modifiers | ForEach-Object { $codes[$_] })
$hotkeyVk = if ($Key -eq 'Space') { [byte]0x20 } else { [byte][char]$Key.ToUpper() }

# The viewer is the only top-level window titled exactly "RightPeek".
function Test-ViewerVisible {
    $h = [RightPeekSoak.Keys]::FindWindow($null, 'RightPeek')
    $h -ne [IntPtr]::Zero -and [RightPeekSoak.Keys]::IsWindowVisible($h)
}

function Get-PrivateMB([int]$Id) {
    $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$Id" -ErrorAction SilentlyContinue
    if ($perf) { [math]::Round([double]$perf.WorkingSetPrivate / 1MB, 1) } else { $null }
}

function Start-Target([string]$Text) {
    Start-Process powershell -PassThru -WindowStyle Normal -ArgumentList @(
        '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', "$PSScriptRoot\test-target.ps1",
        '-Seconds', [string]($Minutes * 60 + 120), '-Text', "`"$Text`"")
}

if (Get-Process rightpeek -ErrorAction SilentlyContinue) { throw 'RightPeek is already running; quit it first.' }
$app = Start-Process $Exe -PassThru
Start-Sleep -Seconds 5
$shell = New-Object -ComObject WScript.Shell
$withText = Start-Target 'سلام دنیا (آزمایش ۱۲۳) و یک متن English با نیم‌فاصله: می‌خواهم.'
Start-Sleep -Seconds 4
$memStart = Get-PrivateMB $app.Id
$deadline = (Get-Date).AddMinutes($Minutes)
$rounds = 0; $failures = 0; $alive = $true

$locked = $false
while ((Get-Date) -lt $deadline) {
    # Keys go nowhere on the lock screen; stop instead of counting failures.
    if (Get-Process LogonUI -ErrorAction SilentlyContinue) { $locked = $true; break }
    $rounds++
    foreach ($empty in $false, $true) {
        if ($empty) { $box = Start-Target ''; Start-Sleep -Seconds 4 }
        [void]$shell.AppActivate($(if ($empty) { $box.Id } else { $withText.Id }))
        Start-Sleep -Milliseconds 400
        Send-Keys $hotkeyMods $hotkeyVk
        Start-Sleep -Seconds $PauseSeconds
        if (-not (Test-ViewerVisible)) { $failures++ } else { Send-Keys @() 0x1B }  # Esc closes the viewer
        if ($empty) { Stop-Process -Id $box.Id -ErrorAction SilentlyContinue }
        Start-Sleep -Milliseconds 500
    }
    if (-not (Get-Process -Id $app.Id -ErrorAction SilentlyContinue)) { $alive = $false; break }
    if ($rounds % 10 -eq 0) { Write-Host ("round {0}: {1} MB" -f $rounds, (Get-PrivateMB $app.Id)) }
}

$memEnd = if ($alive) { Get-PrivateMB $app.Id } else { $null }
Stop-Process -Id $withText.Id -ErrorAction SilentlyContinue
if ($alive) { Stop-Process -Id $app.Id -ErrorAction SilentlyContinue }
[pscustomobject]@{
    'Minutes'                       = $Minutes
    'Rounds (2 captures each)'      = $rounds
    'Viewer did not appear'         = $failures
    'App still alive'               = $alive
    'Stopped early: screen locked'  = $locked
    'Idle-ish RAM at start (MB)'    = $memStart
    'RAM at end (MB)'               = $memEnd
} | Format-List
