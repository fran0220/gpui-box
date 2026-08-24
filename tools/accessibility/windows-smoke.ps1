param(
    [Parameter(Mandatory = $true)]
    [int]$TargetProcessId,
    [Parameter(Mandatory = $true)]
    [ValidateSet("editable", "form", "menu")]
    [string]$Mode
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

# The Windows PowerShell UIAutomationTypes assembly does not expose a named
# identifier for newer UIA properties. LookupById is the documented bridge for
# properties whose numerical identifier is known; FullDescription is 30159.
$FullDescriptionProperty =
    [System.Windows.Automation.AutomationProperty]::LookupById(30159)

function Find-All {
    param(
        [System.Windows.Automation.ControlType]$ControlType,
        [string]$Name
    )

    $condition = [System.Windows.Automation.AndCondition]::new(
        [System.Windows.Automation.PropertyCondition]::new(
            [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
            $TargetProcessId
        ),
        [System.Windows.Automation.PropertyCondition]::new(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            $ControlType
        ),
        [System.Windows.Automation.PropertyCondition]::new(
            [System.Windows.Automation.AutomationElement]::NameProperty,
            $Name
        )
    )
    return @(
        [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
            [System.Windows.Automation.TreeScope]::Descendants,
            $condition
        )
    )
}

function Find-Unique {
    param(
        [System.Windows.Automation.ControlType]$ControlType,
        [string]$Name
    )

    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $matches = @(Find-All -ControlType $ControlType -Name $Name)
        if ($matches.Count -eq 1) {
            return $matches[0]
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "expected one $ControlType named '$Name', found $($matches.Count)"
}

function Wait-Until {
    param(
        [scriptblock]$Predicate,
        [string]$Failure
    )

    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        if (& $Predicate) {
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw $Failure
}

function Pattern {
    param(
        [System.Windows.Automation.AutomationElement]$Element,
        [System.Windows.Automation.AutomationPattern]$Pattern
    )

    $value = $null
    if (-not $Element.TryGetCurrentPattern($Pattern, [ref]$value)) {
        throw "element '$($Element.Current.Name)' does not expose pattern $($Pattern.ProgrammaticName)"
    }
    return $value
}

function One-CharacterRange {
    param(
        $Document,
        [int]$Offset
    )

    $range = $Document.Clone()
    $null = $range.MoveEndpointByRange(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
        $range,
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start
    )
    $null = $range.MoveEndpointByUnit(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
        [System.Windows.Automation.Text.TextUnit]::Character,
        $Offset
    )
    $null = $range.MoveEndpointByRange(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
        $range,
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start
    )
    $null = $range.MoveEndpointByUnit(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
        [System.Windows.Automation.Text.TextUnit]::Character,
        1
    )
    return $range
}

function First-Bounds {
    param($Range)

    $rectangles = @($Range.GetBoundingRectangles())
    if ($rectangles.Count -eq 0) {
        return $null
    }
    return $rectangles[0]
}

switch ($Mode) {
    "editable" {
        $email = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::Edit) -Name "Email"
        $email.SetFocus()
        $value = [System.Windows.Automation.ValuePattern](
            Pattern -Element $email -Pattern ([System.Windows.Automation.ValuePattern]::Pattern)
        )
        $value.SetValue("edited@example.com")
        Wait-Until -Failure "UIA ValuePattern did not edit Email" -Predicate {
            $current = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::Edit) -Name "Email"
            $pattern = [System.Windows.Automation.ValuePattern](
                Pattern -Element $current -Pattern ([System.Windows.Automation.ValuePattern]::Pattern)
            )
            return $pattern.Current.Value -eq "edited@example.com" -and $current.Current.HasKeyboardFocus
        }

        $email = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::Edit) -Name "Email"
        $text = [System.Windows.Automation.TextPattern](
            Pattern -Element $email -Pattern ([System.Windows.Automation.TextPattern]::Pattern)
        )
        $document = $text.DocumentRange
        $first = First-Bounds (One-CharacterRange -Document $document -Offset 0)
        $middle = First-Bounds (One-CharacterRange -Document $document -Offset 5)
        if ($null -eq $first -or $null -eq $middle -or
            $first.Width -le 0 -or $first.Height -le 0 -or
            $middle.Width -le 0 -or $middle.Height -le 0 -or
            ($first.X -eq $middle.X -and $first.Y -eq $middle.Y)) {
            throw "Email did not expose distinct, non-empty UIA character rectangles"
        }

        $selection = @($text.GetSelection())
        if ($selection.Count -ne 1 -or
            $selection[0].CompareEndpoints(
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
                $document,
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End
            ) -ne 0 -or
            $selection[0].CompareEndpoints(
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
                $document,
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End
            ) -ne 0) {
            throw "Email did not expose its logical end caret through UIA TextPattern selection"
        }
        Write-Output "Email|Edit|edited@example.com|focused|character-bounds|end-caret"
    }

    "form" {
        $workspace = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::Edit) -Name "Workspace name"
        $retention = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::Edit) -Name "Retention"
        $workspaceHelp = [string]$workspace.GetCurrentPropertyValue(
            $FullDescriptionProperty,
            $true
        )
        $retentionHelp = [string]$retention.GetCurrentPropertyValue(
            $FullDescriptionProperty,
            $true
        )
        if ($workspaceHelp -ne "Shown wherever this workspace appears. A workspace with this name already exists.") {
            throw "Workspace name FullDescription was '$workspaceHelp'"
        }
        if ($retentionHelp -ne "How long a finished run is kept. This workspace allows at most 60 days.") {
            throw "Retention FullDescription was '$retentionHelp'"
        }
        Write-Output "Workspace name|$workspaceHelp"
        Write-Output "Retention|$retentionHelp"
    }

    "menu" {
        $menu = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::Menu) -Name "Run actions"
        $copyLink = Find-Unique -ControlType ([System.Windows.Automation.ControlType]::MenuItem) -Name "Copy link"
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        if (-not $copyLink.Current.HasKeyboardFocus -or
            $focused.Current.ProcessId -ne $TargetProcessId -or
            $focused.Current.Name -ne "Copy link") {
            throw "Copy link was not the singular focused UIA MenuItem"
        }
        $invoke = [System.Windows.Automation.InvokePattern](
            Pattern -Element $copyLink -Pattern ([System.Windows.Automation.InvokePattern]::Pattern)
        )
        $invoke.Invoke()
        Wait-Until -Failure "Run actions UIA Menu remained after invoking Copy link" -Predicate {
            return @(Find-All -ControlType ([System.Windows.Automation.ControlType]::Menu) -Name "Run actions").Count -eq 0
        }
        Write-Output "Run actions|Copy link|focused|invoked|closed"
    }
}
