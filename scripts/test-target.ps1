# Opens a small WPF window with Persian text selected (UIA TextPattern target)
# for scripts/measure-budgets.ps1. Closes itself after -Seconds.
param([int]$Seconds = 30)

Add-Type -AssemblyName PresentationFramework
$window = New-Object Windows.Window
$window.Title = 'RightPeekTestTarget'
$window.Width = 500
$window.Height = 200
$window.Topmost = $true
$box = New-Object Windows.Controls.TextBox
$box.AcceptsReturn = $true
$box.FlowDirection = 'RightToLeft'
$box.Text = "سلام دنیا، این یک متن آزمایشی است.`r`nمی‌خواهم نیم‌فاصله حفظ شود."
$window.Content = $box
$window.Add_ContentRendered({ $window.Activate(); [void]$box.Focus(); $box.SelectAll() })
$timer = New-Object Windows.Threading.DispatcherTimer
$timer.Interval = [TimeSpan]::FromSeconds($Seconds)
$timer.Add_Tick({ $window.Close() })
$timer.Start()
[void]$window.ShowDialog()
