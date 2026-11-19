# Reads the accessibility tree of examples/interaction_acceptance back
# through UI Automation, as a screen reader would, and drives it: focus, set
# value, press.
#
# Run with Windows PowerShell in an interactive desktop session, after
# building the example:
#   cargo build --no-default-features --features winit --example interaction_acceptance
#   powershell -File scripts/windows_uia_tree.ps1
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$CT = [System.Windows.Automation.ControlType]
$Scope = [System.Windows.Automation.TreeScope]

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "target" }
$out = if ($env:RUI_UIA_OUT) { $env:RUI_UIA_OUT } else { "target/windows-uia" }
New-Item -ItemType Directory -Force -Path $out | Out-Null
$example = Join-Path $targetDir "debug/examples/interaction_acceptance.exe"
$script:failed = $false

function Wait-For($what, [scriptblock]$find) {
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline) {
        try { $found = & $find } catch { $found = $null }
        if ($found) { return $found }
        Start-Sleep -Milliseconds 200
    }
    throw "FAIL: timed out waiting for $what"
}

function Find-Node($root, $type, $name) {
    $condition = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $type)),
        (New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $name)))
    return $root.FindFirst($Scope::Descendants, $condition)
}

function Check($condition, $message) {
    if (-not $condition) { throw "FAIL: $message" }
    Write-Output "ok: $message"
}

function Write-Tree($node, $depth, $lines) {
    $info = $node.Current
    $lines.Add(("  " * $depth) + "$($info.ControlType.ProgrammaticName) '$($info.Name)' enabled=$($info.IsEnabled)")
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $child = $walker.GetFirstChild($node)
    while ($child) {
        Write-Tree $child ($depth + 1) $lines
        $child = $walker.GetNextSibling($child)
    }
}

$app = Start-Process -FilePath $example -PassThru
try {
    $titled = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, "RUI Interaction Acceptance")
    $window = Wait-For "the window" { $AE::RootElement.FindFirst($Scope::Children, $titled) }
    # The first tree arrives with the first frame after UI Automation asks.
    $save = Wait-For "the Save button" { Find-Node $window $CT::Button "Save" }
    $lines = New-Object System.Collections.Generic.List[string]
    Write-Tree $window 0 $lines
    $lines | Tee-Object -FilePath (Join-Path $out "tree.txt")

    Check ($null -ne (Find-Node $window $CT::Text "RUI interaction acceptance")) "the heading is text"
    Check $save.Current.IsEnabled "Save is enabled"
    $disabled = Find-Node $window $CT::Button "Disabled"
    Check ($null -ne $disabled) "the disabled button is exposed"
    Check (-not $disabled.Current.IsEnabled) "the disabled button is not enabled"
    $filter = Find-Node $window $CT::Edit "Filter"
    Check ($null -ne $filter) "the Filter field is an edit"
    Check ($null -ne (Find-Node $window $CT::Edit "Draft")) "the Draft field is an edit"

    $filter.SetFocus()
    Wait-For "the Filter field to take focus" {
        $node = Find-Node $window $CT::Edit "Filter"
        if ($node.Current.HasKeyboardFocus) { $node }
    } | Out-Null
    Check $true "focus reached the Filter field"

    $filter.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue("zzz")
    Wait-For "the filter to hide the note" { Find-Node $window $CT::Text "No matching notes" } | Out-Null
    $value = (Find-Node $window $CT::Edit "Filter").GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value
    Check ($value -eq "zzz") "set value reached the Filter field"

    $save.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-For "the confirm dialog" { Find-Node $window $CT::Window "Confirm save" } | Out-Null
    Check $true "pressing Save opened the confirm dialog"
    $cancel = Wait-For "the Cancel button" { Find-Node $window $CT::Button "Cancel" }
    $cancel.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-For "the canceled status" { Find-Node $window $CT::Text "Status: Canceled" } | Out-Null
    Check $true "pressing Cancel reached the app"
} finally {
    Stop-Process -Id $app.Id -ErrorAction SilentlyContinue
}
